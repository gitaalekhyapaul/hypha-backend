import { createHash, timingSafeEqual } from "node:crypto";

import { parseReconciliationQuery } from "../reconciliation/http";
import { AuthorizationRejectedError } from "./errors";
import {
  MAX_REQUEST_AGE_SECONDS,
  MAX_REQUEST_FUTURE_SKEW_SECONDS,
  PROTECTED_QUERY_MAX_BODY_BYTES,
  PROTECTED_REQUEST_FORMAT_VERSION,
  type ProtectedRequestEnvelope,
} from "./types";

const ENVELOPE_FIELDS = [
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
] as const;
const QUERY_FIELDS = [
  "fromOccurredAt",
  "toOccurredAt",
  "transactionId",
  "merchantReference",
  "limit",
] as const;
const SAFE_DATASET_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const CANONICAL_UNSIGNED_SECONDS = /^(0|[1-9][0-9]*)$/;
const CANONICAL_BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const LOWERCASE_HEX = /^[0-9a-f]+$/;

export function parseProtectedRequestEnvelope(
  value: unknown,
  datasetId: string,
): ProtectedRequestEnvelope {
  if (!SAFE_DATASET_ID.test(datasetId)) {
    throw new AuthorizationRejectedError();
  }
  const object = strictRecord(value, ENVELOPE_FIELDS);
  if (object.format_version !== PROTECTED_REQUEST_FORMAT_VERSION) {
    throw new AuthorizationRejectedError();
  }

  const requestMethod = exactString(object.request_method);
  const expectedTarget = `/api/datasets/${encodeURIComponent(datasetId)}/protected-query`;
  const canonicalRequestTarget = boundedString(object.canonical_request_target, 2_048);
  if (requestMethod !== "POST" || canonicalRequestTarget !== expectedTarget) {
    throw new AuthorizationRejectedError();
  }

  const requestBodyBase64 = boundedString(object.request_body_base64, 1_398_104);
  const requestBody = decodeCanonicalBase64(requestBodyBase64);
  if (requestBody.length === 0 || requestBody.length > PROTECTED_QUERY_MAX_BODY_BYTES) {
    throw new AuthorizationRejectedError();
  }
  const requestBodyHashHex = canonicalHex(object.request_body_hash, 32);
  const calculatedHash = createHash("sha256").update(requestBody).digest();
  if (!timingSafeEqual(calculatedHash, Buffer.from(requestBodyHashHex, "hex"))) {
    throw new AuthorizationRejectedError();
  }

  const queryValue = parseRequestBody(requestBody);
  assertAllowedKeys(queryValue, QUERY_FIELDS);
  let query: ReturnType<typeof parseReconciliationQuery>;
  try {
    query = parseReconciliationQuery(queryValue);
  } catch {
    throw new AuthorizationRejectedError();
  }

  return {
    formatVersion: PROTECTED_REQUEST_FORMAT_VERSION,
    capabilityProtoHex: canonicalVariableHex(object.capability_proto_hex, 2 * 1_048_576),
    agentPublicKeyHex: canonicalHex(object.agent_public_key, 32),
    requestMethod: "POST",
    canonicalRequestTarget,
    requestBody,
    requestBodyHashHex,
    requestNonceHex: canonicalHex(object.request_nonce, 32),
    requestTimestampUnixSeconds: canonicalUnixSeconds(object.request_timestamp_unix_seconds),
    agentRequestSignatureHex: canonicalHex(object.agent_request_signature, 64),
    query,
  };
}

export function validateRequestFreshness(requestTimestamp: bigint, nowUnixSeconds: bigint): void {
  if (
    requestTimestamp < nowUnixSeconds - MAX_REQUEST_AGE_SECONDS ||
    requestTimestamp > nowUnixSeconds + MAX_REQUEST_FUTURE_SKEW_SECONDS
  ) {
    throw new AuthorizationRejectedError();
  }
}

export function canonicalUnixSeconds(value: unknown): bigint {
  if (typeof value !== "string" || !CANONICAL_UNSIGNED_SECONDS.test(value)) {
    throw new AuthorizationRejectedError();
  }
  try {
    return BigInt(value);
  } catch {
    throw new AuthorizationRejectedError();
  }
}

export function canonicalHex(value: unknown, bytes: number): string {
  const text = exactString(value);
  if (text.length !== bytes * 2 || !LOWERCASE_HEX.test(text)) {
    throw new AuthorizationRejectedError();
  }
  return text;
}

export function boundedIdentifier(value: unknown, maximum = 256): string {
  const text = exactString(value);
  if (
    text.length === 0 ||
    text.length > maximum ||
    [...text].some((character) => character.charCodeAt(0) < 0x20 || character.charCodeAt(0) > 0x7e)
  ) {
    throw new AuthorizationRejectedError();
  }
  return text;
}

export function strictRecord(value: unknown, fields: readonly string[]): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new AuthorizationRejectedError();
  }
  const object = value as Record<string, unknown>;
  const actual = Object.keys(object).sort();
  const expected = [...fields].sort();
  if (
    actual.length !== expected.length ||
    actual.some((field, index) => field !== expected[index])
  ) {
    throw new AuthorizationRejectedError();
  }
  return object;
}

function parseRequestBody(bytes: Uint8Array): unknown {
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return JSON.parse(text) as unknown;
  } catch {
    throw new AuthorizationRejectedError();
  }
}

function assertAllowedKeys(value: unknown, allowed: readonly string[]): void {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new AuthorizationRejectedError();
  }
  const allowedFields = new Set(allowed);
  if (Object.keys(value).some((field) => !allowedFields.has(field))) {
    throw new AuthorizationRejectedError();
  }
}

function decodeCanonicalBase64(value: string): Uint8Array {
  if (!CANONICAL_BASE64.test(value)) {
    throw new AuthorizationRejectedError();
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) {
    throw new AuthorizationRejectedError();
  }
  return decoded;
}

function canonicalVariableHex(value: unknown, maximumBytes: number): string {
  const text = exactString(value);
  if (
    text.length === 0 ||
    text.length > maximumBytes * 2 ||
    text.length % 2 !== 0 ||
    !LOWERCASE_HEX.test(text)
  ) {
    throw new AuthorizationRejectedError();
  }
  return text;
}

function exactString(value: unknown): string {
  if (typeof value !== "string") {
    throw new AuthorizationRejectedError();
  }
  return value;
}

function boundedString(value: unknown, maximum: number): string {
  const text = exactString(value);
  if (text.length === 0 || text.length > maximum) {
    throw new AuthorizationRejectedError();
  }
  return text;
}
