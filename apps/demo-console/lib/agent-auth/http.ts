import { create, toJson } from "@bufbuild/protobuf";
import { AgentIdentityResponseSchema } from "@raziel/proto-ts";
import { RAW_TARGET_HEADER } from "../../server/agent-transport.ts";
import {
  jsonResponse,
  OperatorAuthError,
  operatorAuthResponse,
  operatorAuthService,
} from "../operator-auth/index.ts";
import { agentAuthService } from "./runtime.ts";
import { type AgentAuthCode, AgentAuthError } from "./types.ts";

const STATUS: Record<AgentAuthCode, number> = {
  INVALID_REQUEST: 400,
  UNAUTHENTICATED: 401,
  REQUEST_REPLAYED: 409,
  SERVICE_UNAVAILABLE: 503,
  AGENT_ALREADY_REGISTERED: 409,
  NOT_FOUND: 404,
};

export async function operatorAgentRequest(
  request: Request,
  operation: () => Promise<unknown>,
  status = 200,
): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    if (request.method === "GET") await auth.verify(request);
    else await auth.authorizeMutation(request);
    return jsonResponse(await operation(), status);
  } catch (error) {
    if (error instanceof OperatorAuthError) return operatorAuthResponse(error);
    return agentAuthResponse(error);
  }
}

export async function agentMe(request: Request): Promise<Response> {
  try {
    const rawTarget = request.headers.get(RAW_TARGET_HEADER);
    if (rawTarget === null) throw new AgentAuthError("SERVICE_UNAVAILABLE");
    if (rawTarget !== "/api/agent/v1/me") throw new AgentAuthError("INVALID_REQUEST");
    const { agent, token, audience } = await agentAuthService().authenticate(request);
    return jsonResponse(
      toJson(
        AgentIdentityResponseSchema,
        create(AgentIdentityResponseSchema, {
          formatVersion: 1,
          agentId: agent.agentId,
          publicKey: Buffer.from(agent.publicKey, "base64"),
          keyThumbprint: Buffer.from(agent.keyThumbprint, "base64"),
          audience,
          tokenExpiresAtUnixSeconds: BigInt(token.expiresAtUnixSeconds),
        }),
      ),
    );
  } catch (error) {
    return publicAgentAuthResponse(error);
  }
}

export function publicAgentAuthResponse(error: unknown): Response {
  const internal = error instanceof AgentAuthError ? error.code : "SERVICE_UNAVAILABLE";
  const code =
    internal === "UNAUTHENTICATED"
      ? "NOT_AUTHORIZED"
      : internal === "AGENT_ALREADY_REGISTERED"
        ? "CONFLICT"
        : internal;
  return jsonResponse(
    { error: { code, retryable: code === "SERVICE_UNAVAILABLE" || code === "REQUEST_REPLAYED" } },
    STATUS[internal],
  );
}

export function agentAuthResponse(error: unknown): Response {
  const code = error instanceof AgentAuthError ? error.code : "SERVICE_UNAVAILABLE";
  return jsonResponse({ error: code }, STATUS[code]);
}

export async function readAgentOperatorBody(request: Request): Promise<unknown> {
  if (
    request.headers.has("content-encoding") ||
    request.headers.get("content-type")?.split(";")[0]?.trim() !== "application/json"
  ) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  const reader = request.body?.getReader();
  if (!reader) throw new AgentAuthError("INVALID_REQUEST");
  const chunks: Uint8Array[] = [];
  let bytes = 0;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) break;
      bytes += chunk.value.length;
      if (bytes > 4096) {
        await reader.cancel();
        throw new AgentAuthError("INVALID_REQUEST");
      }
      chunks.push(chunk.value);
    }
    return JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks)),
    ) as unknown;
  } catch {
    throw new AgentAuthError("INVALID_REQUEST");
  } finally {
    reader.releaseLock();
  }
}
