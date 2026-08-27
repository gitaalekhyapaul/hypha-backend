import { createHmac, timingSafeEqual } from "node:crypto";

import {
  OPERATOR_SESSION_FORMAT_VERSION,
  OPERATOR_SESSION_LIFETIME_SECONDS,
  OperatorAuthError,
  type OperatorSecret,
  type OperatorSession,
} from "./types.ts";

const MAX_SESSION_TOKEN_BYTES = 2 * 1024;
const SESSION_ID_BYTES = 16;
const CSRF_TOKEN_BYTES = 32;
const SESSION_SIGNATURE_BYTES = 32;
const BASE64URL = /^[A-Za-z0-9_-]+$/;

interface OperatorSessionWire {
  readonly format_version: typeof OPERATOR_SESSION_FORMAT_VERSION;
  readonly operator_id: string;
  readonly session_id: string;
  readonly csrf_token: string;
  readonly issued_at_unix_seconds: number;
  readonly expires_at_unix_seconds: number;
}

export function issueSessionToken(
  secret: OperatorSecret,
  nowUnixSeconds: number,
  randomBytes: (size: number) => Buffer,
): { readonly token: string; readonly session: OperatorSession } {
  if (!safePositiveInteger(nowUnixSeconds)) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  const expiresAtUnixSeconds = nowUnixSeconds + OPERATOR_SESSION_LIFETIME_SECONDS;
  if (!Number.isSafeInteger(expiresAtUnixSeconds)) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  const sessionId = exactRandom(randomBytes, SESSION_ID_BYTES).toString("base64url");
  const csrfToken = exactRandom(randomBytes, CSRF_TOKEN_BYTES).toString("base64url");
  const wire: OperatorSessionWire = {
    format_version: OPERATOR_SESSION_FORMAT_VERSION,
    operator_id: secret.operatorId,
    session_id: sessionId,
    csrf_token: csrfToken,
    issued_at_unix_seconds: nowUnixSeconds,
    expires_at_unix_seconds: expiresAtUnixSeconds,
  };
  const payload = Buffer.from(JSON.stringify(wire), "utf8").toString("base64url");
  const signature = sign(payload, secret.sessionHmacKey).toString("base64url");
  return {
    token: `${payload}.${signature}`,
    session: sessionFromWire(wire),
  };
}

export function verifySessionToken(
  token: string,
  secret: OperatorSecret,
  nowUnixSeconds: number,
): OperatorSession {
  if (Buffer.byteLength(token) > MAX_SESSION_TOKEN_BYTES || !safePositiveInteger(nowUnixSeconds)) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  const parts = token.split(".");
  if (parts.length !== 2) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  const [payload, encodedSignature] = parts;
  if (
    !payload ||
    !encodedSignature ||
    !BASE64URL.test(payload) ||
    !BASE64URL.test(encodedSignature)
  ) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  const suppliedSignature = Buffer.from(encodedSignature, "base64url");
  const expectedSignature = sign(payload, secret.sessionHmacKey);
  if (
    suppliedSignature.byteLength !== SESSION_SIGNATURE_BYTES ||
    suppliedSignature.toString("base64url") !== encodedSignature ||
    !timingSafeEqual(suppliedSignature, expectedSignature)
  ) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }

  let input: unknown;
  try {
    const bytes = Buffer.from(payload, "base64url");
    if (bytes.toString("base64url") !== payload) {
      throw new Error("non-canonical payload");
    }
    input = JSON.parse(bytes.toString("utf8")) as unknown;
  } catch {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  const wire = strictSessionWire(input);
  if (
    wire.operator_id !== secret.operatorId ||
    wire.issued_at_unix_seconds > nowUnixSeconds ||
    wire.expires_at_unix_seconds <= nowUnixSeconds ||
    wire.expires_at_unix_seconds - wire.issued_at_unix_seconds !== OPERATOR_SESSION_LIFETIME_SECONDS
  ) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  return sessionFromWire(wire);
}

function strictSessionWire(input: unknown): OperatorSessionWire {
  const keys = [
    "format_version",
    "operator_id",
    "session_id",
    "csrf_token",
    "issued_at_unix_seconds",
    "expires_at_unix_seconds",
  ];
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  const wire = input as Record<string, unknown>;
  if (
    Object.keys(wire).length !== keys.length ||
    keys.some((key) => !(key in wire)) ||
    wire.format_version !== OPERATOR_SESSION_FORMAT_VERSION ||
    typeof wire.operator_id !== "string" ||
    typeof wire.session_id !== "string" ||
    typeof wire.csrf_token !== "string" ||
    !canonicalBase64Url(wire.session_id, SESSION_ID_BYTES) ||
    !canonicalBase64Url(wire.csrf_token, CSRF_TOKEN_BYTES) ||
    !safePositiveInteger(wire.issued_at_unix_seconds) ||
    !safePositiveInteger(wire.expires_at_unix_seconds)
  ) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  return wire as unknown as OperatorSessionWire;
}

function sessionFromWire(wire: OperatorSessionWire): OperatorSession {
  return {
    operatorId: wire.operator_id,
    sessionId: wire.session_id,
    csrfToken: wire.csrf_token,
    issuedAtUnixSeconds: wire.issued_at_unix_seconds,
    expiresAtUnixSeconds: wire.expires_at_unix_seconds,
  };
}

function sign(payload: string, key: Buffer): Buffer {
  return createHmac("sha256", key).update("raziel:operator-session:v1\0").update(payload).digest();
}

function exactRandom(randomBytes: (size: number) => Buffer, size: number): Buffer {
  const value = randomBytes(size);
  if (!Buffer.isBuffer(value) || value.byteLength !== size) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  return value;
}

function canonicalBase64Url(value: string, expectedBytes: number): boolean {
  if (!BASE64URL.test(value)) {
    return false;
  }
  const decoded = Buffer.from(value, "base64url");
  return decoded.byteLength === expectedBytes && decoded.toString("base64url") === value;
}

function safePositiveInteger(input: unknown): input is number {
  return Number.isSafeInteger(input) && (input as number) > 0;
}
