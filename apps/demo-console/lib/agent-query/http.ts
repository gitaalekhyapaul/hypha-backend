import { fromBinary } from "@bufbuild/protobuf";
import { ProofCapabilitySchema } from "@raziel/proto-ts";
import { projectGatewayResult } from "../../app/api/reconciliation/query/contract.ts";
import { RAW_TARGET_HEADER } from "../../server/agent-transport.ts";
import { publicAgentAuthResponse } from "../agent-auth/http.ts";
import { AgentAuthError, type AgentRegistration } from "../agent-auth/types.ts";
import { jsonResponse } from "../operator-auth/index.ts";

const ENVELOPE_LIMIT = 6 * 1024 * 1024;
const RESPONSE_LIMIT = 1024 * 1024;
const FIELDS = [
  "format_version",
  "capability_proto_hex",
  "agent_public_key",
  "request_method",
  "canonical_request_target",
  "request_body_base64",
  "request_body_hash",
  "request_nonce",
  "request_timestamp_unix_seconds",
  "agent_request_signature",
];

interface Dependencies {
  authenticate(request: Request, body: Uint8Array): Promise<AgentRegistration>;
  gatewayUrl(): string;
  fetch?: typeof fetch;
}

class Rejected extends Error {}

export function createAgentQueryHandler(deps: Dependencies) {
  return async (request: Request, datasetId: string): Promise<Response> => {
    try {
      if (!/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(datasetId)) throw new Rejected();
      const target = `/api/datasets/${encodeURIComponent(datasetId)}/protected-query`;
      const rawTarget = request.headers.get(RAW_TARGET_HEADER);
      if (rawTarget === null) throw new AgentAuthError("SERVICE_UNAVAILABLE");
      if (request.method !== "POST" || rawTarget !== target) throw new Rejected();
      if (
        request.headers.has("content-encoding") ||
        request.headers.get("content-type")?.split(";", 1)[0]?.trim() !== "application/json"
      ) {
        throw new Rejected();
      }
      let bytes: Buffer;
      try {
        bytes = await readBounded(request, ENVELOPE_LIMIT);
      } catch {
        throw new Rejected();
      }
      const agent = await deps.authenticate(request, bytes);
      const expected = inspectEnvelope(bytes, agent, target, datasetId);
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 15_000);
      try {
        // Only this fixed gateway route is reachable. Tokens and caller headers
        // never cross the owner-network hop, and neither signed body is rewritten.
        const response = await (deps.fetch ?? fetch)(new URL(target, deps.gatewayUrl()), {
          method: "POST",
          body: new Uint8Array(bytes),
          redirect: "error",
          cache: "no-store",
          headers: { "content-type": "application/json" },
          signal: controller.signal,
        });
        if (response.status === 403) {
          await response.body?.cancel();
          throw new Rejected();
        }
        if (
          response.status !== 200 ||
          response.headers.get("content-type")?.split(";", 1)[0]?.trim() !== "application/json"
        ) {
          await response.body?.cancel();
          throw new Error();
        }
        const result = JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(
            await readBounded(response, RESPONSE_LIMIT),
          ),
        );
        return jsonResponse(projectGatewayResult(result, expected));
      } finally {
        controller.abort();
        clearTimeout(timeout);
      }
    } catch (error) {
      if (error instanceof Rejected) return jsonResponse({ error: "AUTHORIZATION_REJECTED" }, 403);
      if (error instanceof AgentAuthError) return publicAgentAuthResponse(error);
      return jsonResponse({ error: "PROTECTED_QUERY_UNAVAILABLE" }, 503);
    }
  };
}

function inspectEnvelope(
  bytes: Buffer,
  agent: AgentRegistration,
  target: string,
  datasetId: string,
) {
  try {
    const value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
    if (
      value === null ||
      typeof value !== "object" ||
      Array.isArray(value) ||
      Object.keys(value).length !== FIELDS.length ||
      FIELDS.some((key) => !Object.hasOwn(value, key)) ||
      value.format_version !== 1 ||
      value.request_method !== "POST" ||
      value.canonical_request_target !== target ||
      value.agent_public_key !== Buffer.from(agent.publicKey, "base64").toString("hex") ||
      typeof value.capability_proto_hex !== "string" ||
      value.capability_proto_hex.length > 4 * 1024 * 1024 ||
      !/^(?:[0-9a-f]{2})+$/.test(value.capability_proto_hex)
    )
      throw new Rejected();
    // These fields select the public response projection, not authorization.
    // The independent gateway still validates every claim and both cold/cache paths.
    const capability = fromBinary(
      ProofCapabilitySchema,
      Buffer.from(value.capability_proto_hex, "hex"),
    );
    if (
      !/^[0-9a-f]{64}$/.test(capability.capabilityId) ||
      !/^(dataset|dataset_[0-9a-f]{32})$/.test(capability.resourceType) ||
      capability.resourceScope !== datasetId
    )
      throw new Rejected();
    return {
      capabilityId: capability.capabilityId,
      datasetId,
      resourceType: capability.resourceType,
    };
  } catch {
    throw new Rejected();
  }
}

async function readBounded(source: Request | Response, limit: number): Promise<Buffer> {
  const length = source.headers.get("content-length");
  if (length !== null && (!/^[1-9][0-9]*$/.test(length) || Number(length) > limit))
    throw new Error();
  const reader = source.body?.getReader();
  if (!reader) throw new Error();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) {
        await reader.cancel();
        throw new Error();
      }
      chunks.push(value);
    }
    if (size === 0) throw new Error();
    return Buffer.concat(chunks);
  } finally {
    reader.releaseLock();
  }
}
