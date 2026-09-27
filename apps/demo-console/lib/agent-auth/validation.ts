import { AgentAuthError } from "./types.ts";

export const MAX_UNIX_SECONDS = 253402300799;
export const MAX_TOKEN_LIFETIME_SECONDS = 90 * 86400;
export const AGENT_ID = /^[0-9a-f]{32}$/;

export function wholeSeconds(value: unknown): number {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,11})$/.test(value)) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  const seconds = Number(value);
  if (!Number.isSafeInteger(seconds) || seconds > MAX_UNIX_SECONDS) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  return seconds;
}

export function publicKeyBytes(value: unknown): Buffer {
  if (typeof value !== "string" || value.length !== 44) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.length !== 32 || bytes.toString("base64") !== value) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  return bytes;
}

export function exactObject(value: unknown, fields: readonly string[]): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  const object = value as Record<string, unknown>;
  if (Object.keys(object).length !== fields.length || fields.some((field) => !(field in object))) {
    throw new AgentAuthError("INVALID_REQUEST");
  }
  return object;
}

export function tokenParts(value: string): { tokenId: string; secret: string } {
  const match = /^rz1\.([0-9a-f]{32})\.([A-Za-z0-9_-]{43})$/.exec(value);
  const tokenId = match?.[1];
  const secret = match?.[2];
  if (!tokenId || !secret || Buffer.from(secret, "base64url").toString("base64url") !== secret) {
    throw new AgentAuthError("UNAUTHENTICATED");
  }
  return { tokenId, secret };
}

export function assertId(value: string): void {
  if (!AGENT_ID.test(value)) throw new AgentAuthError("INVALID_REQUEST");
}
