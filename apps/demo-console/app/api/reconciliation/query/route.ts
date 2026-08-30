import {
  liveJobBinding,
  retainedCapabilityProtoHex,
} from "../../../../lib/agent-capability/live-registry.ts";
import { encodeHex, signProtectedRequest } from "../../../../lib/agent-capability/sdk.ts";
import { operatorAgentIdentity } from "../../../../lib/agent-capability/session-store.ts";
import { CapabilityFlowError } from "../../../../lib/agent-capability/types.ts";
import { loadDatasetGatewayConfig } from "../../../../lib/dataset-client/index.ts";
import {
  jsonResponse,
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../lib/operator-auth/index.ts";
import {
  exactGatewayQueryBody,
  type ProtectedGatewayEnvelope,
  protectedGatewayTarget,
  ReconciliationConsoleError,
  readGatewayResult,
  readReconciliationQuery,
} from "./contract.ts";

export const dynamic = "force-dynamic";

const GATEWAY_TIMEOUT_MILLISECONDS = 15_000;
const CONTROLLED_OUT_OF_SCOPE_DATASET_ID = "reconciliation-denied-scope";

export async function POST(request: Request): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.authorizeMutation(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }

  try {
    const input = await readReconciliationQuery(request);
    const binding = liveJobBinding(input.job_id, operatorId);
    if (
      binding.action !== "query" ||
      binding.resourceType !== "dataset" ||
      binding.resourceScope !== input.dataset_id
    ) {
      throw authorizationFailure();
    }

    const capabilityProtoHex = retainedCapabilityProtoHex(input.job_id, operatorId);
    const identity = await operatorAgentIdentity(operatorId);
    if (encodeHex(identity.keyThumbprint) !== binding.agentKeyThumbprint) {
      throw authorizationFailure();
    }

    const targetDatasetId =
      input.request_mode === "OUT_OF_SCOPE" ? CONTROLLED_OUT_OF_SCOPE_DATASET_ID : input.dataset_id;
    if (input.request_mode === "OUT_OF_SCOPE" && targetDatasetId === binding.resourceScope) {
      throw authorizationFailure();
    }
    const body = exactGatewayQueryBody(input);
    const target = protectedGatewayTarget(targetDatasetId);
    const signed = await signProtectedRequest(identity, {
      capabilityId: input.job_id,
      method: "POST",
      target,
      body,
      requestTimestampUnixSeconds: BigInt(Math.floor(Date.now() / 1_000)),
    });
    if (
      signed.capabilityId !== input.job_id ||
      signed.requestMethod !== "POST" ||
      signed.canonicalRequestTarget !== target
    ) {
      throw authorizationFailure();
    }
    const envelope: ProtectedGatewayEnvelope = {
      format_version: signed.formatVersion,
      capability_proto_hex: capabilityProtoHex,
      agent_public_key: encodeHex(signed.agentPublicKey),
      request_method: "POST",
      canonical_request_target: signed.canonicalRequestTarget,
      request_body_base64: Buffer.from(body).toString("base64"),
      request_body_hash: encodeHex(signed.requestBodyHash),
      request_nonce: encodeHex(signed.requestNonce),
      request_timestamp_unix_seconds: signed.requestTimestampUnixSeconds.toString(),
      agent_request_signature: encodeHex(signed.agentRequestSignature),
    };

    const config = loadDatasetGatewayConfig();
    const response = await sendToGateway(config.baseUrl, target, envelope);
    return jsonResponse(
      await readGatewayResult(response, {
        capabilityId: signed.capabilityId,
        datasetId: targetDatasetId,
      }),
    );
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}

async function sendToGateway(
  baseUrl: string,
  target: string,
  envelope: ProtectedGatewayEnvelope,
): Promise<Response> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), GATEWAY_TIMEOUT_MILLISECONDS);
  try {
    return await fetch(new URL(target, baseUrl), {
      method: "POST",
      cache: "no-store",
      signal: controller.signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(envelope),
    });
  } catch {
    throw new ReconciliationConsoleError("RECONCILIATION_UNAVAILABLE", 503);
  } finally {
    clearTimeout(timeout);
  }
}

function reconciliationErrorResponse(error: unknown): Response {
  if (error instanceof ReconciliationConsoleError) {
    return jsonResponse({ error: error.code }, error.status);
  }
  if (error instanceof CapabilityFlowError) {
    return jsonResponse({ error: "RECONCILIATION_AUTHORIZATION_FAILED" }, 403);
  }
  return jsonResponse({ error: "RECONCILIATION_UNAVAILABLE" }, 503);
}

function authorizationFailure(): ReconciliationConsoleError {
  return new ReconciliationConsoleError("RECONCILIATION_AUTHORIZATION_FAILED", 403);
}
