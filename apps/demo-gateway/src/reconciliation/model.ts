import { createHash } from "node:crypto";

export const DATASET_FORMAT_VERSION = 1;
export const DATASET_CLASSIFICATION = "SYNTHETIC";
export const MAX_DATASET_ROWS = 500;
export const MAX_QUERY_ROWS = 100;
export const MAX_QUERY_RANGE_SECONDS = 31 * 24 * 60 * 60;

const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const WHOLE_SECOND_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;
const CANONICAL_INTEGER = /^-?(0|[1-9][0-9]{0,18})$/;
const IDENTIFIER = /^[a-z][a-z0-9_]*$/;
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/;
const SCHEMA_DESCRIPTOR = [
  "raziel/synthetic-reconciliation-dataset/v1",
  "transaction_id:string",
  "occurred_at:utc-second",
  "amount_minor:i64-decimal",
  "currency:iso-4217-alpha",
  "merchant_reference:string?",
  "account_reference:string?",
  "transaction_type:identifier?",
  "reconciliation_status:UNMATCHED|MATCHED|REVIEW",
  "source_record_digest:sha256",
].join("\n");

export type ReconciliationStatus = "UNMATCHED" | "MATCHED" | "REVIEW";
export type DatasetStatus = "DRAFT" | "FINALIZED";

export interface SyntheticTransactionInput {
  readonly transactionId: string;
  readonly occurredAt: string;
  readonly amountMinor: string;
  readonly currency: string;
  readonly merchantReference?: string;
  readonly accountReference?: string;
  readonly transactionType?: string;
  readonly reconciliationStatus: ReconciliationStatus;
}

export interface SyntheticTransaction extends SyntheticTransactionInput {
  readonly sourceRecordDigest: string;
}

export interface DatasetSource {
  readonly classification: typeof DATASET_CLASSIFICATION;
  readonly sourceId: string;
  readonly sourceLabel: string;
}

export interface DatasetFinalization {
  readonly finalizedAt: string;
  readonly checksum: string;
  readonly schemaFingerprint: string;
  readonly rowCount: number;
}

export interface SyntheticDataset {
  readonly formatVersion: typeof DATASET_FORMAT_VERSION;
  readonly datasetId: string;
  readonly revision: number;
  readonly derivedFromDatasetId?: string;
  readonly source: DatasetSource;
  readonly status: DatasetStatus;
  readonly rows: readonly SyntheticTransaction[];
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly finalization?: DatasetFinalization;
}

export interface DatasetDraftInput {
  readonly datasetId: string;
  readonly sourceId: string;
  readonly sourceLabel: string;
  readonly rows: readonly SyntheticTransactionInput[];
}

export interface ReconciliationQuery {
  readonly fromOccurredAt: string;
  readonly toOccurredAt: string;
  readonly transactionId?: string;
  readonly merchantReference?: string;
  readonly limit?: number;
}

export class ReconciliationValidationError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "ReconciliationValidationError";
  }
}

export function createDraft(
  input: DatasetDraftInput,
  now: string,
  existing?: SyntheticDataset,
): SyntheticDataset {
  validateSafeId(input.datasetId, "dataset identifier");
  validateSafeId(input.sourceId, "source identifier");
  validateText(input.sourceLabel, "source label", 128);
  validateWholeSecondUtc(now, "current time");
  if (existing?.status === "FINALIZED") {
    throw new ReconciliationValidationError("finalized datasets are immutable");
  }
  if (existing && existing.datasetId !== input.datasetId) {
    throw new ReconciliationValidationError("dataset identifier does not match existing draft");
  }
  const rows = normalizeRows(input.rows);
  return {
    formatVersion: DATASET_FORMAT_VERSION,
    datasetId: input.datasetId,
    revision: existing?.revision ?? 1,
    ...(existing?.derivedFromDatasetId
      ? { derivedFromDatasetId: existing.derivedFromDatasetId }
      : {}),
    source: {
      classification: DATASET_CLASSIFICATION,
      sourceId: input.sourceId,
      sourceLabel: input.sourceLabel,
    },
    status: "DRAFT",
    rows,
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

export function finalizeDataset(dataset: SyntheticDataset, now: string): SyntheticDataset {
  if (dataset.status === "FINALIZED") {
    return dataset;
  }
  validateWholeSecondUtc(now, "finalization time");
  if (dataset.rows.length === 0) {
    throw new ReconciliationValidationError("an empty dataset cannot be finalized");
  }
  return {
    ...dataset,
    status: "FINALIZED",
    updatedAt: now,
    finalization: {
      finalizedAt: now,
      checksum: datasetChecksum(dataset),
      schemaFingerprint: schemaFingerprint(),
      rowCount: dataset.rows.length,
    },
  };
}

export function cloneDataset(
  source: SyntheticDataset,
  targetDatasetId: string,
  now: string,
): SyntheticDataset {
  validateSafeId(targetDatasetId, "target dataset identifier");
  validateWholeSecondUtc(now, "clone time");
  if (targetDatasetId === source.datasetId) {
    throw new ReconciliationValidationError("clone target must have a new dataset identifier");
  }
  return {
    formatVersion: DATASET_FORMAT_VERSION,
    datasetId: targetDatasetId,
    revision: source.revision + 1,
    derivedFromDatasetId: source.datasetId,
    source: { ...source.source },
    status: "DRAFT",
    rows: source.rows.map((row) => ({ ...row })),
    createdAt: now,
    updatedAt: now,
  };
}

export function queryDataset(
  dataset: SyntheticDataset,
  query: ReconciliationQuery,
): readonly SyntheticTransaction[] {
  if (dataset.status !== "FINALIZED") {
    throw new ReconciliationValidationError("only finalized datasets can be queried");
  }
  const from = validateWholeSecondUtc(query.fromOccurredAt, "query start");
  const to = validateWholeSecondUtc(query.toOccurredAt, "query end");
  if (from > to) {
    throw new ReconciliationValidationError("query start must not follow query end");
  }
  if ((to - from) / 1_000 > MAX_QUERY_RANGE_SECONDS) {
    throw new ReconciliationValidationError("query range exceeds the supported maximum");
  }
  const limit = query.limit ?? 50;
  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_QUERY_ROWS) {
    throw new ReconciliationValidationError("query limit is outside the supported range");
  }
  if (query.transactionId !== undefined) {
    validateSafeId(query.transactionId, "query transaction identifier");
  }
  if (query.merchantReference !== undefined) {
    validateText(query.merchantReference, "query merchant reference", 128);
  }

  return dataset.rows
    .filter((row) => {
      const occurredAt = Date.parse(row.occurredAt);
      return (
        occurredAt >= from &&
        occurredAt <= to &&
        (query.transactionId === undefined || row.transactionId === query.transactionId) &&
        (query.merchantReference === undefined || row.merchantReference === query.merchantReference)
      );
    })
    .sort(
      (left, right) =>
        left.occurredAt.localeCompare(right.occurredAt) ||
        left.transactionId.localeCompare(right.transactionId),
    )
    .slice(0, limit);
}

export function schemaFingerprint(): string {
  return sha256Hex(SCHEMA_DESCRIPTOR);
}

function normalizeRows(rows: readonly SyntheticTransactionInput[]): SyntheticTransaction[] {
  if (rows.length > MAX_DATASET_ROWS) {
    throw new ReconciliationValidationError("dataset exceeds the supported row count");
  }
  const seen = new Set<string>();
  return rows
    .map((input) => {
      validateSafeId(input.transactionId, "transaction identifier");
      validateWholeSecondUtc(input.occurredAt, "transaction time");
      validateAmount(input.amountMinor);
      if (!/^[A-Z]{3}$/.test(input.currency)) {
        throw new ReconciliationValidationError("currency must use three uppercase letters");
      }
      validateOptionalText(input.merchantReference, "merchant reference");
      validateOptionalText(input.accountReference, "account reference");
      if (input.transactionType !== undefined && !IDENTIFIER.test(input.transactionType)) {
        throw new ReconciliationValidationError("transaction type must be a lowercase identifier");
      }
      if (!(["UNMATCHED", "MATCHED", "REVIEW"] as const).includes(input.reconciliationStatus)) {
        throw new ReconciliationValidationError("reconciliation status is unsupported");
      }
      if (seen.has(input.transactionId)) {
        throw new ReconciliationValidationError("transaction identifiers must be unique");
      }
      seen.add(input.transactionId);
      const normalized: SyntheticTransactionInput = {
        transactionId: input.transactionId,
        occurredAt: input.occurredAt,
        amountMinor: input.amountMinor,
        currency: input.currency,
        ...(input.merchantReference !== undefined
          ? { merchantReference: input.merchantReference }
          : {}),
        ...(input.accountReference !== undefined
          ? { accountReference: input.accountReference }
          : {}),
        ...(input.transactionType !== undefined ? { transactionType: input.transactionType } : {}),
        reconciliationStatus: input.reconciliationStatus,
      };
      return {
        ...normalized,
        sourceRecordDigest: sha256Hex(canonicalRow(normalized)),
      };
    })
    .sort((left, right) => left.transactionId.localeCompare(right.transactionId));
}

function datasetChecksum(dataset: SyntheticDataset): string {
  return sha256Hex(
    JSON.stringify({
      formatVersion: dataset.formatVersion,
      datasetId: dataset.datasetId,
      revision: dataset.revision,
      source: dataset.source,
      rows: dataset.rows,
    }),
  );
}

function canonicalRow(row: SyntheticTransactionInput): string {
  return JSON.stringify({
    transactionId: row.transactionId,
    occurredAt: row.occurredAt,
    amountMinor: row.amountMinor,
    currency: row.currency,
    merchantReference: row.merchantReference ?? null,
    accountReference: row.accountReference ?? null,
    transactionType: row.transactionType ?? null,
    reconciliationStatus: row.reconciliationStatus,
  });
}

function validateAmount(value: string): void {
  if (!CANONICAL_INTEGER.test(value) || value === "-0") {
    throw new ReconciliationValidationError("amount_minor must be a canonical decimal integer");
  }
  const amount = BigInt(value);
  if (amount < I64_MIN || amount > I64_MAX) {
    throw new ReconciliationValidationError("amount_minor is outside the signed 64-bit range");
  }
}

function validateSafeId(value: string, label: string): void {
  if (value.length > 128 || !SAFE_ID.test(value)) {
    throw new ReconciliationValidationError(`${label} is invalid`);
  }
}

function validateText(value: string, label: string, maximum = 128): void {
  if (value.length === 0 || value.length > maximum || !isPrintableAscii(value)) {
    throw new ReconciliationValidationError(`${label} is invalid`);
  }
}

function validateOptionalText(value: string | undefined, label: string): void {
  if (value !== undefined) {
    validateText(value, label);
  }
}

function validateWholeSecondUtc(value: string, label: string): number {
  if (!WHOLE_SECOND_UTC.test(value)) {
    throw new ReconciliationValidationError(`${label} must use whole-second UTC`);
  }
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed) || new Date(parsed).toISOString().replace(".000Z", "Z") !== value) {
    throw new ReconciliationValidationError(`${label} is invalid`);
  }
  return parsed;
}

function isPrintableAscii(value: string): boolean {
  return [...value].every((character) => {
    const code = character.charCodeAt(0);
    return code >= 0x20 && code <= 0x7e;
  });
}

function sha256Hex(value: string): string {
  return createHash("sha256").update(value, "utf8").digest("hex");
}
