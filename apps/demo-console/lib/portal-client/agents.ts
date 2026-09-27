import { OPERATOR_CSRF_HEADER } from "../operator-auth/types.ts";
import { PortalRequestError } from "./browser.ts";

export interface AgentRegistration {
  readonly formatVersion: 1;
  readonly agentId: string;
  readonly displayName: string;
  readonly publicKey: string;
  readonly keyThumbprint: string;
  readonly createdAtUnixSeconds: string;
}

export interface AgentTokenMetadata {
  readonly formatVersion: 1;
  readonly tokenId: string;
  readonly agentId: string;
  readonly createdAtUnixSeconds: string;
  readonly expiresAtUnixSeconds: string;
  readonly revokedAtUnixSeconds?: string;
}

export interface CreatedAgentToken {
  readonly token: string;
  readonly metadata: AgentTokenMetadata;
}

export const DEFAULT_AGENT_TOKEN_DAYS = 30;
export const MAX_AGENT_TOKEN_DAYS = 90;

export function publicKeyHexToBase64(input: string): string {
  const hex = input.trim();
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new PortalRequestError("INVALID_PUBLIC_KEY", 400);
  }
  const bytes = Array.from({ length: 32 }, (_, index) =>
    String.fromCharCode(Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16)),
  );
  return btoa(bytes.join(""));
}

export function agentTokenExpiry(days: number, now = Date.now()): string {
  if (!Number.isInteger(days) || days < 1 || days > MAX_AGENT_TOKEN_DAYS) {
    throw new PortalRequestError("INVALID_TOKEN_LIFETIME", 400);
  }
  return String(Math.floor(now / 1_000) + days * 86_400);
}

export async function listAgentRegistrations(
  signal?: AbortSignal,
): Promise<{ readonly agents: readonly AgentRegistration[] }> {
  const result = object(await requestAgents("/api/operator/agents", {}, signal));
  if (!Array.isArray(result.agents)) throw unavailable();
  return { agents: result.agents.map(registration) };
}

export async function registerAgent(
  displayName: string,
  publicKeyHex: string,
  csrfToken: string,
): Promise<AgentRegistration> {
  const name = displayName.trim();
  if (!name) throw new PortalRequestError("INVALID_DISPLAY_NAME", 400);
  return registration(
    await mutateAgents("/api/operator/agents", csrfToken, {
      formatVersion: 1,
      displayName: name,
      publicKey: publicKeyHexToBase64(publicKeyHex),
    }),
  );
}

export async function listAgentTokens(
  agentId: string,
  signal?: AbortSignal,
): Promise<{ readonly tokens: readonly AgentTokenMetadata[] }> {
  const result = object(await requestAgents(tokenPath(agentId), {}, signal));
  if (!Array.isArray(result.tokens)) throw unavailable();
  return { tokens: result.tokens.map((value) => tokenMetadata(value, agentId)) };
}

export async function createAgentToken(
  agentId: string,
  days: number,
  csrfToken: string,
): Promise<CreatedAgentToken> {
  const result = object(
    await mutateAgents(tokenPath(agentId), csrfToken, {
      formatVersion: 1,
      expiresAtUnixSeconds: agentTokenExpiry(days),
    }),
  );
  const metadata = tokenMetadata(result.metadata, agentId);
  if (
    typeof result.token !== "string" ||
    !/^rz1\.[0-9a-f]{32}\.[A-Za-z0-9_-]{43}$/.test(result.token) ||
    result.token.split(".")[1] !== metadata.tokenId
  ) {
    throw unavailable();
  }
  return { token: result.token, metadata };
}

export async function revokeAgentToken(
  agentId: string,
  tokenId: string,
  csrfToken: string,
): Promise<AgentTokenMetadata> {
  const metadata = tokenMetadata(
    await mutateAgents(`${tokenPath(agentId)}/${encodeURIComponent(tokenId)}/revoke`, csrfToken, {
      formatVersion: 1,
    }),
    agentId,
  );
  if (metadata.tokenId !== tokenId || metadata.revokedAtUnixSeconds === undefined) {
    throw unavailable();
  }
  return metadata;
}

function tokenPath(agentId: string): string {
  return `/api/operator/agents/${encodeURIComponent(agentId)}/tokens`;
}

function mutateAgents(path: string, csrfToken: string, body: unknown): Promise<unknown> {
  return requestAgents(path, {
    method: "POST",
    headers: { "Content-Type": "application/json", [OPERATOR_CSRF_HEADER]: csrfToken },
    body: JSON.stringify(body),
  });
}

async function requestAgents(
  path: string,
  init: RequestInit = {},
  signal?: AbortSignal,
): Promise<unknown> {
  let response: Response;
  try {
    const timeout = AbortSignal.timeout(15_000);
    response = await fetch(path, {
      ...init,
      cache: "no-store",
      credentials: "same-origin",
      signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
    });
  } catch {
    throw unavailable();
  }
  // Session expiry must survive an empty or non-JSON response from an intermediary.
  if (response.status === 401) throw new PortalRequestError("OPERATOR_UNAUTHORIZED", 401);
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new PortalRequestError("PORTAL_UNAVAILABLE", response.ok ? 503 : response.status);
  }
  if (!response.ok) {
    const error = typeof body === "object" && body !== null ? Reflect.get(body, "error") : null;
    const code = typeof error === "string" ? error : error?.code;
    throw new PortalRequestError(
      typeof code === "string" ? code : "PORTAL_UNAVAILABLE",
      response.status,
    );
  }
  return body;
}

function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw unavailable();
  return value as Record<string, unknown>;
}

function textField(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw unavailable();
  return value;
}

function timestamp(value: unknown): string {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,11})$/.test(value)) throw unavailable();
  if (Number(value) > 253_402_300_799) throw unavailable();
  return value;
}

function base64Key(value: unknown): string {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]{43}=$/.test(value)) throw unavailable();
  if (btoa(atob(value)) !== value) throw unavailable();
  return value;
}

function registration(value: unknown): AgentRegistration {
  const data = object(value);
  if (data.formatVersion !== 1) throw unavailable();
  return {
    formatVersion: 1,
    agentId: textField(data.agentId),
    displayName: textField(data.displayName),
    publicKey: base64Key(data.publicKey),
    keyThumbprint: base64Key(data.keyThumbprint),
    createdAtUnixSeconds: timestamp(data.createdAtUnixSeconds),
  };
}

function tokenMetadata(value: unknown, agentId: string): AgentTokenMetadata {
  const data = object(value);
  if (data.formatVersion !== 1 || data.agentId !== agentId) throw unavailable();
  // Project metadata explicitly so a list can never retain a bearer secret.
  return {
    formatVersion: 1,
    tokenId: textField(data.tokenId),
    agentId,
    createdAtUnixSeconds: timestamp(data.createdAtUnixSeconds),
    expiresAtUnixSeconds: timestamp(data.expiresAtUnixSeconds),
    ...(data.revokedAtUnixSeconds === undefined
      ? {}
      : { revokedAtUnixSeconds: timestamp(data.revokedAtUnixSeconds) }),
  };
}

function unavailable(): PortalRequestError {
  return new PortalRequestError("PORTAL_UNAVAILABLE", 503);
}
