import type { SyntheticTransaction } from "../../../../lib/dataset-client/types.ts";
import type {
  ProtectedReconciliationQueryInput,
  ProtectedReconciliationQueryResult,
  PublicReconciliationReceipt,
} from "../../../../lib/portal-client/reconciliation.ts";

const MAX_BROWSER_BODY_BYTES = 8 * 1024;
const MAX_GATEWAY_RESPONSE_BYTES = 1024 * 1024;
const MAX_QUERY_RANGE_MILLISECONDS = 31 * 24 * 60 * 60 * 1_000;
const MAX_QUERY_ROWS = 100;
const LOWER_HEX_DIGEST = /^[0-9a-f]{64}$/;
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const WHOLE_SECOND_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;

export type ReconciliationConsoleFailureCode =
  | "INVALID_RECONCILIATION_QUERY"
  | "RECONCILIATION_AUTHORIZATION_FAILED"
  | "RECONCILIATION_REQUEST_REPLAYED"
  | "RECONCILIATION_REQUEST_EXPIRED"
  | "RECONCILIATION_REQUEST_ALTERED"
  | "RECONCILIATION_REQUEST_UNSIGNED"
  | "RECONCILIATION_UNAVAILABLE";

export class ReconciliationConsoleError extends Error {
  public readonly code: ReconciliationConsoleFailureCode;
  public readonly status: number;

  public constructor(code: ReconciliationConsoleFailureCode, status: number) {
    super(code);
    this.name = "ReconciliationConsoleError";
    this.code = code;
    this.status = status;
  }
}

export interface ProtectedGatewayEnvelope {
  readonly format_version: 1;
  readonly capability_proto_hex: string;
  readonly agent_public_key: string;
  readonly request_method: "POST";
  readonly canonical_request_target: string;
  readonly request_body_base64: string;
  readonly request_body_hash: string;
  readonly request_nonce: string;
  readonly request_timestamp_unix_seconds: string;
  readonly agent_request_signature: string;
}

export async function readReconciliationQuery(
  request: Request,
): Promise<ProtectedReconciliationQueryInput> {
  const declaredLength = request.headers.get("content-length");
  if (declaredLength !== null) {
    const parsed = Number(declaredLength);
    if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > MAX_BROWSER_BODY_BYTES) {
      throw invalidQuery();
    }
  }
  const raw = await request.text();
  if (Buffer.byteLength(raw, "utf8") > MAX_BROWSER_BODY_BYTES) {
    throw invalidQuery();
  }
  let input: unknown;
  try {
    input = JSON.parse(raw) as unknown;
  } catch {
    throw invalidQuery();
  }
  try {
    const object = exactObject(input, [
      "job_id",
      "dataset_id",
      "request_mode",
      "from_occurred_at",
      "to_occurred_at",
      "limit",
    ]);
    const jobId = stringValue(object.job_id);
    const datasetId = stringValue(object.dataset_id);
    const requestMode = object.request_mode;
    const fromOccurredAt = utcSecond(object.from_occurred_at);
    const toOccurredAt = utcSecond(object.to_occurred_at);
    const limit = integerValue(object.limit);
    if (
      !LOWER_HEX_DIGEST.test(jobId) ||
      !SAFE_ID.test(datasetId) ||
      (requestMode !== "IN_SCOPE" &&
        requestMode !== "OUT_OF_SCOPE" &&
        requestMode !== "REPLAY" &&
        requestMode !== "EXPIRED" &&
        requestMode !== "ALTERED" &&
        requestMode !== "UNSIGNED") ||
      fromOccurredAt.milliseconds > toOccurredAt.milliseconds ||
      toOccurredAt.milliseconds - fromOccurredAt.milliseconds > MAX_QUERY_RANGE_MILLISECONDS ||
      limit < 1 ||
      limit > MAX_QUERY_ROWS
    ) {
      throw invalidQuery();
    }
    return {
      job_id: jobId,
      dataset_id: datasetId,
      request_mode: requestMode,
      from_occurred_at: fromOccurredAt.value,
      to_occurred_at: toOccurredAt.value,
      limit,
    };
  } catch {
    throw invalidQuery();
  }
}

export function exactGatewayQueryBody(input: ProtectedReconciliationQueryInput): Uint8Array {
  return new TextEncoder().encode(
    JSON.stringify({
      fromOccurredAt: input.from_occurred_at,
      toOccurredAt: input.to_occurred_at,
      limit: input.limit,
    }),
  );
}

export function protectedGatewayTarget(datasetId: string): string {
  return `/api/datasets/${encodeURIComponent(datasetId)}/protected-query`;
}

export async function readGatewayAuthorizationRejection(response: Response): Promise<void> {
  if (response.status !== 403) {
    throw unavailable();
  }
  const declaredLength = response.headers.get("content-length");
  if (declaredLength !== null) {
    const parsed = Number(declaredLength);
    if (!Number.isSafeInteger(parsed) || parsed <= 0 || parsed > MAX_GATEWAY_RESPONSE_BYTES) {
      throw unavailable();
    }
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.byteLength === 0 || bytes.byteLength > MAX_GATEWAY_RESPONSE_BYTES) {
    throw unavailable();
  }
  try {
    const object = exactObject(JSON.parse(bytes.toString("utf8")) as unknown, ["error"]);
    if (object.error !== "AUTHORIZATION_REJECTED") {
      throw unavailable();
    }
  } catch {
    throw unavailable();
  }
}

export async function readGatewayResult(
  response: Response,
  expected: { readonly capabilityId: string; readonly datasetId: string },
): Promise<ProtectedReconciliationQueryResult> {
  if (!response.ok) {
    if (response.status === 401 || response.status === 403 || response.status === 409) {
      throw new ReconciliationConsoleError("RECONCILIATION_AUTHORIZATION_FAILED", 403);
    }
    throw unavailable();
  }
  const declaredLength = response.headers.get("content-length");
  if (declaredLength !== null) {
    const parsed = Number(declaredLength);
    if (!Number.isSafeInteger(parsed) || parsed <= 0 || parsed > MAX_GATEWAY_RESPONSE_BYTES) {
      throw unavailable();
    }
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.byteLength === 0 || bytes.byteLength > MAX_GATEWAY_RESPONSE_BYTES) {
    throw unavailable();
  }
  let input: unknown;
  try {
    input = JSON.parse(bytes.toString("utf8")) as unknown;
  } catch {
    throw unavailable();
  }
  return projectGatewayResult(input, expected);
}

export function projectGatewayResult(
  input: unknown,
  expected: { readonly capabilityId: string; readonly datasetId: string },
): ProtectedReconciliationQueryResult {
  const object = objectValue(input);
  const rows = arrayValue(object.rows);
  const rowCount = integerValue(object.row_count);
  const verificationPath = object.verification_path;
  if (
    object.classification !== "SYNTHETIC" ||
    object.dataset_id !== expected.datasetId ||
    (verificationPath !== "VERIFIER" && verificationPath !== "CACHE") ||
    rowCount !== rows.length ||
    rowCount < 0 ||
    rowCount > MAX_QUERY_ROWS
  ) {
    throw unavailable();
  }
  return {
    classification: "SYNTHETIC",
    dataset_id: expected.datasetId,
    verification_path: verificationPath,
    row_count: rowCount,
    rows: rows.map(projectTransaction),
    receipt: projectReceipt(object.receipt, expected),
  };
}

function projectReceipt(
  input: unknown,
  expected: { readonly capabilityId: string; readonly datasetId: string },
): PublicReconciliationReceipt {
  const object = objectValue(input);
  const receiptId = safeId(object.receipt_id);
  const requestId = safeId(object.request_id);
  const taskId = safeId(object.task_id);
  const proofJobId = safeId(object.proof_job_id);
  const policyRecordId = safeId(object.policy_record_id);
  const policyHash = lowercaseDigest(object.policy_hash);
  const snapshotId = safeId(object.snapshot_id);
  const relationshipSnapshotRoot = lowercaseDigest(object.relationship_snapshot_root);
  const publicClaimsDigest = lowercaseDigest(object.public_claims_digest);
  const proofDigest = lowercaseDigest(object.proof_digest);
  const occurredAt = utcSecond(object.occurred_at).value;
  if (
    object.format_version !== 1 ||
    object.receipt_type !== "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST" ||
    object.capability_id !== expected.capabilityId ||
    proofJobId !== expected.capabilityId ||
    object.action !== "query" ||
    object.resource_type !== "dataset" ||
    object.resource_scope !== expected.datasetId ||
    object.outcome !== "AUDIT_OUTCOME_ACCEPTED" ||
    object.reason_code !== "PUBLIC_ERROR_CODE_UNSPECIFIED"
  ) {
    throw unavailable();
  }
  return {
    format_version: 1,
    receipt_id: receiptId,
    receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
    request_id: requestId,
    occurred_at: occurredAt,
    task_id: taskId,
    capability_id: expected.capabilityId,
    proof_job_id: proofJobId,
    policy_record_id: policyRecordId,
    policy_hash: policyHash,
    snapshot_id: snapshotId,
    relationship_snapshot_root: relationshipSnapshotRoot,
    public_claims_digest: publicClaimsDigest,
    proof_digest: proofDigest,
    action: "query",
    resource_type: "dataset",
    resource_scope: expected.datasetId,
    outcome: "AUDIT_OUTCOME_ACCEPTED",
    reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
  };
}

function projectTransaction(input: unknown): SyntheticTransaction {
  const object = objectValue(input);
  const sourceRecordDigest = stringValue(object.sourceRecordDigest);
  const reconciliationStatus = object.reconciliationStatus;
  if (
    !LOWER_HEX_DIGEST.test(sourceRecordDigest) ||
    (reconciliationStatus !== "UNMATCHED" &&
      reconciliationStatus !== "MATCHED" &&
      reconciliationStatus !== "REVIEW")
  ) {
    throw unavailable();
  }
  return {
    transactionId: boundedText(object.transactionId, 128),
    occurredAt: utcSecond(object.occurredAt).value,
    amountMinor: boundedText(object.amountMinor, 32),
    currency: boundedText(object.currency, 3),
    ...optionalText(object, "merchantReference", 128),
    ...optionalText(object, "accountReference", 128),
    ...optionalText(object, "transactionType", 128),
    reconciliationStatus,
    sourceRecordDigest,
  };
}

function exactObject(input: unknown, keys: readonly string[]): Record<string, unknown> {
  const object = objectValue(input);
  if (Object.keys(object).length !== keys.length || keys.some((key) => !(key in object))) {
    throw invalidQuery();
  }
  return object;
}

function objectValue(input: unknown): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw unavailable();
  }
  return input as Record<string, unknown>;
}

function arrayValue(input: unknown): readonly unknown[] {
  if (!Array.isArray(input)) {
    throw unavailable();
  }
  return input;
}

function stringValue(input: unknown): string {
  if (typeof input !== "string") {
    throw unavailable();
  }
  return input;
}

function integerValue(input: unknown): number {
  if (typeof input !== "number" || !Number.isSafeInteger(input)) {
    throw unavailable();
  }
  return input;
}

function boundedText(input: unknown, maximumBytes: number): string {
  const value = stringValue(input);
  if (value.length === 0 || Buffer.byteLength(value, "utf8") > maximumBytes) {
    throw unavailable();
  }
  return value;
}

function optionalText(
  object: Record<string, unknown>,
  key: string,
  maximumBytes: number,
): Record<string, string> {
  return object[key] === undefined ? {} : { [key]: boundedText(object[key], maximumBytes) };
}

function safeId(input: unknown): string {
  const value = stringValue(input);
  if (!SAFE_ID.test(value)) {
    throw unavailable();
  }
  return value;
}

function lowercaseDigest(input: unknown): string {
  const value = stringValue(input);
  if (!LOWER_HEX_DIGEST.test(value)) {
    throw unavailable();
  }
  return value;
}

function utcSecond(input: unknown): { readonly milliseconds: number; readonly value: string } {
  const value = stringValue(input);
  const milliseconds = Date.parse(value);
  if (
    !WHOLE_SECOND_UTC.test(value) ||
    !Number.isFinite(milliseconds) ||
    new Date(milliseconds).toISOString().replace(".000Z", "Z") !== value
  ) {
    throw unavailable();
  }
  return { milliseconds, value };
}

function invalidQuery(): ReconciliationConsoleError {
  return new ReconciliationConsoleError("INVALID_RECONCILIATION_QUERY", 400);
}

function unavailable(): ReconciliationConsoleError {
  return new ReconciliationConsoleError("RECONCILIATION_UNAVAILABLE", 503);
}
