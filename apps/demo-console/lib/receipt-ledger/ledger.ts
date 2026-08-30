import { readFile, stat } from "node:fs/promises";
import path from "node:path";

import {
  type PublicAuditReceipt,
  type PublicReceiptLedger,
  RECEIPT_FILTER_FIELDS,
  ReceiptLedgerError,
  type ReceiptLedgerQuery,
} from "./types.ts";

const MAX_RECEIPT_FILE_BYTES = 8 * 1024 * 1024;
const MAX_RECEIPT_LINE_BYTES = 4_096;
const MAX_RECEIPTS = 4_096;
const MAX_FILTER_VALUE_BYTES = 256;
const LOWER_HEX_DIGEST = /^[0-9a-f]{64}$/;
const WHOLE_SECOND_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;
const RECEIPT_FIELDS = [
  "format_version",
  "receipt_id",
  "receipt_type",
  "occurred_at",
  "request_id",
  "task_id",
  "capability_id",
  "proof_job_id",
  "policy_record_id",
  "policy_hash",
  "snapshot_id",
  "relationship_snapshot_root",
  "public_claims_digest",
  "proof_digest",
  "action",
  "resource_type",
  "resource_scope",
  "outcome",
  "reason_code",
] as const;

export function receiptStorePath(environment: NodeJS.ProcessEnv = process.env): string {
  const configured = environment.RAZIEL_RECEIPT_STORE_PATH ?? ".raziel/m5/receipts.jsonl";
  if (configured.length === 0 || Buffer.byteLength(configured, "utf8") > 4_096) {
    throw unavailable();
  }
  return path.resolve(/* turbopackIgnore: true */ configured);
}

export function readReceiptLedgerQuery(requestUrl: string): ReceiptLedgerQuery {
  let url: URL;
  try {
    url = new URL(requestUrl);
  } catch {
    throw invalidQuery();
  }
  const allowed = new Set(["field", "value", "format"]);
  for (const key of url.searchParams.keys()) {
    if (!allowed.has(key) || url.searchParams.getAll(key).length !== 1) {
      throw invalidQuery();
    }
  }
  const format = url.searchParams.get("format") ?? "json";
  if (format !== "json" && format !== "jsonl") {
    throw invalidQuery();
  }
  const field = url.searchParams.get("field");
  const value = url.searchParams.get("value");
  if ((field === null) !== (value === null)) {
    throw invalidQuery();
  }
  if (field === null || value === null) {
    return { format };
  }
  if (
    !RECEIPT_FILTER_FIELDS.includes(field as (typeof RECEIPT_FILTER_FIELDS)[number]) ||
    !isBoundedPublicText(value, MAX_FILTER_VALUE_BYTES)
  ) {
    throw invalidQuery();
  }
  return {
    format,
    filter: {
      field: field as (typeof RECEIPT_FILTER_FIELDS)[number],
      value,
    },
  };
}

export async function readPublicReceiptLedger(
  filePath: string,
  query: ReceiptLedgerQuery,
): Promise<PublicReceiptLedger> {
  const receipts = await readPublicReceipts(filePath);
  const filter = query.filter;
  const filtered = filter
    ? receipts.filter((receipt) => receipt[filter.field] === filter.value)
    : receipts;
  return {
    classification: "PUBLIC_AUDIT",
    total_count: receipts.length,
    filtered_count: filtered.length,
    receipts: filtered,
  };
}

export function serializePublicReceiptJsonl(receipts: readonly PublicAuditReceipt[]): string {
  return (
    receipts.map((receipt) => JSON.stringify(receipt)).join("\n") + (receipts.length ? "\n" : "")
  );
}

async function readPublicReceipts(filePath: string): Promise<readonly PublicAuditReceipt[]> {
  let metadata: Awaited<ReturnType<typeof stat>>;
  try {
    metadata = await stat(filePath);
  } catch (error) {
    if (isMissingFile(error)) {
      return [];
    }
    throw unavailable();
  }
  if (!metadata.isFile() || metadata.size > MAX_RECEIPT_FILE_BYTES) {
    throw unavailable();
  }

  let bytes: Buffer;
  try {
    bytes = await readFile(filePath);
  } catch {
    throw unavailable();
  }
  if (bytes.byteLength > MAX_RECEIPT_FILE_BYTES) {
    throw unavailable();
  }
  if (bytes.byteLength === 0) {
    return [];
  }

  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw unavailable();
  }
  const lines = text.endsWith("\n") ? text.slice(0, -1).split("\n") : text.split("\n");
  if (lines.length > MAX_RECEIPTS || lines.some((line) => line.length === 0)) {
    throw unavailable();
  }
  return lines.map(parsePublicReceiptLine);
}

function parsePublicReceiptLine(line: string): PublicAuditReceipt {
  if (Buffer.byteLength(line, "utf8") > MAX_RECEIPT_LINE_BYTES) {
    throw unavailable();
  }
  let input: unknown;
  try {
    input = JSON.parse(line) as unknown;
  } catch {
    throw unavailable();
  }
  const object = exactObject(input, RECEIPT_FIELDS);
  const receiptType = object.receipt_type;
  const outcome = object.outcome;
  const reasonCode = object.reason_code;
  if (
    object.format_version !== 1 ||
    (receiptType !== "AUDIT_RECEIPT_TYPE_CAPABILITY_VERIFICATION" &&
      receiptType !== "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST") ||
    (outcome !== "AUDIT_OUTCOME_ACCEPTED" &&
      outcome !== "AUDIT_OUTCOME_REJECTED" &&
      outcome !== "AUDIT_OUTCOME_ERROR") ||
    (reasonCode !== "PUBLIC_ERROR_CODE_UNSPECIFIED" &&
      reasonCode !== "PUBLIC_ERROR_CODE_INVALID_REQUEST" &&
      reasonCode !== "PUBLIC_ERROR_CODE_NOT_AUTHORIZED" &&
      reasonCode !== "PUBLIC_ERROR_CODE_CAPABILITY_EXPIRED" &&
      reasonCode !== "PUBLIC_ERROR_CODE_CAPABILITY_INVALID" &&
      reasonCode !== "PUBLIC_ERROR_CODE_REQUEST_REPLAYED" &&
      reasonCode !== "PUBLIC_ERROR_CODE_SERVICE_UNAVAILABLE")
  ) {
    throw unavailable();
  }
  return {
    format_version: 1,
    receipt_id: publicText(object.receipt_id),
    receipt_type: receiptType,
    occurred_at: utcSecond(object.occurred_at),
    request_id: publicText(object.request_id),
    task_id: publicText(object.task_id),
    capability_id: publicText(object.capability_id),
    proof_job_id: publicText(object.proof_job_id),
    policy_record_id: publicText(object.policy_record_id),
    policy_hash: digest(object.policy_hash),
    snapshot_id: publicText(object.snapshot_id),
    relationship_snapshot_root: digest(object.relationship_snapshot_root),
    public_claims_digest: digest(object.public_claims_digest),
    proof_digest: digest(object.proof_digest),
    action: publicText(object.action),
    resource_type: publicText(object.resource_type),
    resource_scope: publicText(object.resource_scope),
    outcome,
    reason_code: reasonCode,
  };
}

function exactObject(input: unknown, fields: readonly string[]): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw unavailable();
  }
  const object = input as Record<string, unknown>;
  const actual = Object.keys(object).sort();
  const expected = [...fields].sort();
  if (
    actual.length !== expected.length ||
    actual.some((field, index) => field !== expected[index])
  ) {
    throw unavailable();
  }
  return object;
}

function publicText(value: unknown): string {
  if (typeof value !== "string" || !isBoundedPublicText(value, MAX_FILTER_VALUE_BYTES)) {
    throw unavailable();
  }
  return value;
}

function isBoundedPublicText(value: string, maximumBytes: number): boolean {
  return (
    Buffer.byteLength(value, "utf8") > 0 &&
    Buffer.byteLength(value, "utf8") <= maximumBytes &&
    [...value].every((character) => {
      const code = character.charCodeAt(0);
      return code >= 0x20 && code <= 0x7e;
    })
  );
}

function digest(value: unknown): string {
  if (typeof value !== "string" || !LOWER_HEX_DIGEST.test(value)) {
    throw unavailable();
  }
  return value;
}

function utcSecond(value: unknown): string {
  if (typeof value !== "string" || !WHOLE_SECOND_UTC.test(value)) {
    throw unavailable();
  }
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed) || new Date(parsed).toISOString().replace(".000Z", "Z") !== value) {
    throw unavailable();
  }
  return value;
}

function isMissingFile(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { readonly code?: unknown }).code === "ENOENT"
  );
}

function invalidQuery(): ReceiptLedgerError {
  return new ReceiptLedgerError("INVALID_RECEIPT_QUERY", 400);
}

function unavailable(): ReceiptLedgerError {
  return new ReceiptLedgerError("RECEIPT_LEDGER_UNAVAILABLE", 503);
}
