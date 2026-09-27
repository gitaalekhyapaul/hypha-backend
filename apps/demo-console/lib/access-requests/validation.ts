import { fromJson, type JsonValue } from "@bufbuild/protobuf";
import {
  AccessGrantInputsSchema,
  AccessIntentSchema,
  AccessRequestMutationSchema,
} from "@raziel/proto-ts";
import { AccessRequestError } from "./types.ts";

export const ACCESS_BODY_LIMIT = 16 * 1024;
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export function requestId(value: string): string {
  if (!UUID_V4.test(value)) throw invalid();
  return value;
}

export function agentId(value: string): string {
  if (!/^[0-9a-f]{32}$/.test(value)) throw invalid();
  return value;
}

export function parseIntent(bytes: Uint8Array) {
  const value = objectJson(bytes);
  keys(
    value,
    ["formatVersion", "requestId", "taskId", "purpose", "action", "datasetId", "datasetRevision"],
    ["requestContext"],
  );
  if (value.formatVersion !== 1 || typeof value.requestId !== "string") throw invalid();
  requestId(value.requestId);
  decimal(value.datasetRevision);
  if (BigInt(value.datasetRevision as string) > BigInt(Number.MAX_SAFE_INTEGER)) throw invalid();
  if (
    typeof value.datasetId !== "string" ||
    !/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(value.datasetId)
  )
    throw invalid();
  if (value.action !== "query") throw invalid();
  for (const field of [value.taskId, value.purpose]) {
    if (
      typeof field !== "string" ||
      field.trim().length === 0 ||
      Buffer.byteLength(field) > 256 ||
      [...field].some(
        (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
      )
    )
      throw invalid();
  }
  const context = value.requestContext ?? [];
  if (!Array.isArray(context) || context.length > 16) throw invalid();
  const names = new Set<string>();
  for (const entry of context) {
    const item = object(entry);
    keys(item, ["name"], ["boolValue", "intValue", "stringValue"]);
    if (
      typeof item.name !== "string" ||
      !/^[a-z][a-z0-9_]{0,255}$/.test(item.name) ||
      names.has(item.name)
    )
      throw invalid();
    names.add(item.name);
    const fields = ["boolValue", "intValue", "stringValue"].filter((key) =>
      Object.hasOwn(item, key),
    );
    if (fields.length !== 1) throw invalid();
    if (fields[0] === "boolValue" && typeof item.boolValue !== "boolean") throw invalid();
    if (
      fields[0] === "stringValue" &&
      (typeof item.stringValue !== "string" || Buffer.byteLength(item.stringValue) > 256)
    )
      throw invalid();
    if (
      fields[0] === "intValue" &&
      (typeof item.intValue !== "string" || !/^(0|-?[1-9][0-9]{0,18})$/.test(item.intValue))
    )
      throw invalid();
  }
  try {
    return fromJson(AccessIntentSchema, value as JsonValue);
  } catch {
    throw invalid();
  }
}

export function parseMutation(bytes: Uint8Array) {
  const value = objectJson(bytes);
  keys(value, ["formatVersion", "expectedGeneration"]);
  if (value.formatVersion !== 1) throw invalid();
  decimal(value.expectedGeneration);
  try {
    return fromJson(AccessRequestMutationSchema, value as JsonValue);
  } catch {
    throw invalid();
  }
}

export function parseApproval(bytes: Uint8Array) {
  const value = objectJson(bytes);
  keys(value, [
    "formatVersion",
    "expectedGeneration",
    "policyRecordId",
    "policyVersionId",
    "approvalEpoch",
    "grant",
  ]);
  if (value.formatVersion !== 1) throw invalid();
  decimal(value.expectedGeneration);
  decimal(value.approvalEpoch);
  if (typeof value.policyVersionId !== "string" || !/^[0-9a-f]{32}$/.test(value.policyVersionId))
    throw invalid();
  if (
    typeof value.policyRecordId !== "string" ||
    !/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(value.policyRecordId)
  )
    throw invalid();
  const grant = object(value.grant);
  keys(grant, [
    "formatVersion",
    "teamId",
    "organizationId",
    "approvedAmount",
    "isApproved",
    "isSuspended",
    "isOverride",
  ]);
  if (grant.formatVersion !== 1) throw invalid();
  for (const id of [grant.teamId, grant.organizationId]) {
    if (typeof id !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(id)) throw invalid();
  }
  decimal(grant.approvedAmount);
  if (BigInt(grant.approvedAmount as string) > BigInt(Number.MAX_SAFE_INTEGER)) throw invalid();
  for (const flag of [grant.isApproved, grant.isSuspended, grant.isOverride]) {
    if (typeof flag !== "boolean") throw invalid();
  }
  try {
    return {
      expectedGeneration: BigInt(value.expectedGeneration as string),
      policyRecordId: value.policyRecordId,
      policyVersionId: value.policyVersionId,
      approvalEpoch: BigInt(value.approvalEpoch as string),
      grant: fromJson(AccessGrantInputsSchema, grant as JsonValue),
    };
  } catch {
    throw invalid();
  }
}

export function listOptions(target: string, operator: boolean) {
  const url = new URL(target, "http://owner.invalid");
  const allowed = operator ? ["limit", "cursor", "agentId"] : ["limit", "cursor"];
  for (const key of url.searchParams.keys()) {
    if (!allowed.includes(key) || url.searchParams.getAll(key).length !== 1) throw invalid();
  }
  const limitText = url.searchParams.get("limit") ?? "25";
  if (!/^[1-9][0-9]?$/.test(limitText) || Number(limitText) > 50) throw invalid();
  const cursor = url.searchParams.get("cursor") ?? undefined;
  if (cursor !== undefined && !/^[A-Za-z0-9_-]{1,512}$/.test(cursor)) throw invalid();
  const owner = url.searchParams.get("agentId") ?? undefined;
  if (owner !== undefined) agentId(owner);
  return {
    limit: Number(limitText),
    ...(cursor === undefined ? {} : { cursor }),
    ...(owner === undefined ? {} : { agentId: owner }),
  };
}

export async function readAccessBody(request: Request): Promise<Uint8Array> {
  if (
    request.headers.has("content-encoding") ||
    request.headers.get("content-type")?.split(";", 1)[0]?.trim() !== "application/json"
  )
    throw invalid();
  const length = request.headers.get("content-length");
  if (length !== null && (!/^(0|[1-9][0-9]*)$/.test(length) || Number(length) > ACCESS_BODY_LIMIT))
    throw invalid();
  const reader = request.body?.getReader();
  if (!reader) throw invalid();
  let size = 0;
  const chunks: Uint8Array[] = [];
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > ACCESS_BODY_LIMIT) {
        await reader.cancel();
        throw invalid();
      }
      chunks.push(value);
    }
    return Buffer.concat(chunks);
  } catch {
    throw invalid();
  } finally {
    reader.releaseLock();
  }
}

function decimal(value: unknown) {
  if (
    typeof value !== "string" ||
    !/^[1-9][0-9]{0,19}$/.test(value) ||
    BigInt(value) > 18446744073709551615n
  )
    throw invalid();
}

function objectJson(bytes: Uint8Array): Record<string, unknown> {
  try {
    if (bytes.length > ACCESS_BODY_LIMIT) throw invalid();
    return object(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)));
  } catch {
    throw invalid();
  }
}

function object(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}

function keys(value: Record<string, unknown>, required: string[], optional: string[] = []) {
  if (
    required.some((key) => !Object.hasOwn(value, key)) ||
    Object.keys(value).some((key) => !required.includes(key) && !optional.includes(key))
  )
    throw invalid();
}

function invalid() {
  return new AccessRequestError("INVALID_REQUEST");
}
