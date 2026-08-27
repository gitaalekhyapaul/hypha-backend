import {
  DatasetClientError,
  type DatasetFinalization,
  type DatasetGatewayConfig,
  type ReconciliationStatus,
  SYNTHETIC_DATASET_CLASSIFICATION,
  SYNTHETIC_DATASET_FORMAT_VERSION,
  type SyntheticDataset,
  type SyntheticDatasetCollection,
  type SyntheticDatasetDraft,
  type SyntheticTransaction,
  type SyntheticTransactionDraft,
} from "./types.ts";

const MAX_OPERATOR_BODY_BYTES = 1024 * 1024;
const MAX_DATASET_ROWS = 500;
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const UTC_SECOND = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;
const HEX_DIGEST = /^[0-9a-f]{64}$/;

export function loadDatasetGatewayConfig(
  environment: NodeJS.ProcessEnv = process.env,
): DatasetGatewayConfig {
  const raw = environment.RAZIEL_GATEWAY_INTERNAL_URL ?? "";
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new DatasetClientError("GATEWAY_CONFIGURATION_INVALID", 503);
  }
  if (
    (url.protocol !== "http:" && url.protocol !== "https:") ||
    url.username !== "" ||
    url.password !== "" ||
    url.pathname !== "/" ||
    url.search !== "" ||
    url.hash !== ""
  ) {
    throw new DatasetClientError("GATEWAY_CONFIGURATION_INVALID", 503);
  }
  return { baseUrl: url.origin };
}

export async function readDatasetJson(request: Request): Promise<unknown> {
  const contentLength = request.headers.get("content-length");
  if (contentLength !== null) {
    const parsed = Number(contentLength);
    if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > MAX_OPERATOR_BODY_BYTES) {
      throw new DatasetClientError("INVALID_DATASET_REQUEST", 400);
    }
  }
  const raw = await request.text();
  if (Buffer.byteLength(raw, "utf8") > MAX_OPERATOR_BODY_BYTES) {
    throw new DatasetClientError("INVALID_DATASET_REQUEST", 400);
  }
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    throw new DatasetClientError("INVALID_DATASET_REQUEST", 400);
  }
}

export function parseDatasetDraft(input: unknown): SyntheticDatasetDraft {
  const object = exactObject(input, ["datasetId", "sourceId", "sourceLabel", "rows"]);
  const rows = arrayValue(object.rows);
  if (rows.length > MAX_DATASET_ROWS) {
    throw new DatasetClientError("INVALID_DATASET_REQUEST", 400);
  }
  return {
    datasetId: validateDatasetReference(object.datasetId),
    sourceId: validateDatasetReference(object.sourceId),
    sourceLabel: boundedString(object.sourceLabel, 128),
    rows: rows.map(parseDraftRow),
  };
}

export function parseCloneRequest(input: unknown): { readonly targetDatasetId: string } {
  const object = exactObject(input, ["targetDatasetId"]);
  return { targetDatasetId: validateDatasetReference(object.targetDatasetId) };
}

export function validateDatasetReference(input: unknown): string {
  if (typeof input !== "string" || !SAFE_ID.test(input)) {
    throw new DatasetClientError("INVALID_DATASET_REQUEST", 400);
  }
  return input;
}

export function decodeDatasetCollection(input: unknown): SyntheticDatasetCollection {
  const object = exactObject(input, ["classification", "datasets"]);
  if (object.classification !== SYNTHETIC_DATASET_CLASSIFICATION) {
    throw unavailable();
  }
  const datasets = arrayValue(object.datasets);
  if (datasets.length > 1000) {
    throw unavailable();
  }
  return {
    classification: SYNTHETIC_DATASET_CLASSIFICATION,
    datasets: datasets.map(decodeDataset),
  };
}

export function decodeDataset(input: unknown): SyntheticDataset {
  const object = objectValue(input);
  const required = [
    "formatVersion",
    "datasetId",
    "revision",
    "source",
    "status",
    "rows",
    "createdAt",
    "updatedAt",
  ];
  const optional = ["derivedFromDatasetId", "finalization"];
  exactKeys(object, required, optional);
  if (object.formatVersion !== SYNTHETIC_DATASET_FORMAT_VERSION) {
    throw unavailable();
  }
  const source = exactObject(object.source, ["classification", "sourceId", "sourceLabel"]);
  if (source.classification !== SYNTHETIC_DATASET_CLASSIFICATION) {
    throw unavailable();
  }
  const rows = arrayValue(object.rows);
  if (rows.length > MAX_DATASET_ROWS) {
    throw unavailable();
  }
  const status = datasetStatus(object.status);
  const finalization =
    object.finalization === undefined ? undefined : decodeFinalization(object.finalization);
  if ((status === "FINALIZED") !== (finalization !== undefined)) {
    throw unavailable();
  }
  return {
    formatVersion: SYNTHETIC_DATASET_FORMAT_VERSION,
    datasetId: strictId(object.datasetId),
    revision: positiveInteger(object.revision),
    ...(object.derivedFromDatasetId === undefined
      ? {}
      : { derivedFromDatasetId: strictId(object.derivedFromDatasetId) }),
    source: {
      classification: SYNTHETIC_DATASET_CLASSIFICATION,
      sourceId: strictId(source.sourceId),
      sourceLabel: strictString(source.sourceLabel, 128),
    },
    status,
    rows: rows.map(decodeTransaction),
    createdAt: utcSecond(object.createdAt),
    updatedAt: utcSecond(object.updatedAt),
    ...(finalization === undefined ? {} : { finalization }),
  };
}

export function decodeGatewayError(input: unknown): DatasetClientError {
  const object = objectValue(input);
  exactKeys(object, ["error"], ["message"]);
  const message = object.message === undefined ? undefined : strictString(object.message, 256);
  switch (object.error) {
    case "DATASET_NOT_FOUND":
      return new DatasetClientError("DATASET_NOT_FOUND", 404);
    case "DATASET_CONFLICT":
      return new DatasetClientError("DATASET_CONFLICT", 409, message);
    case "INVALID_SYNTHETIC_DATASET":
      return new DatasetClientError("INVALID_SYNTHETIC_DATASET", 400, message);
    case "DATASET_UNAVAILABLE":
      return unavailable();
    default:
      return unavailable();
  }
}

function parseDraftRow(input: unknown): SyntheticTransactionDraft {
  const object = objectValue(input);
  exactKeys(
    object,
    ["transactionId", "occurredAt", "amountMinor", "currency", "reconciliationStatus"],
    ["merchantReference", "accountReference", "transactionType"],
  );
  return {
    transactionId: boundedString(object.transactionId, 128),
    occurredAt: boundedString(object.occurredAt, 32),
    amountMinor: boundedString(object.amountMinor, 32),
    currency: boundedString(object.currency, 3),
    ...optionalString(object, "merchantReference", 128),
    ...optionalString(object, "accountReference", 128),
    ...optionalString(object, "transactionType", 128),
    reconciliationStatus: reconciliationStatus(object.reconciliationStatus),
  };
}

function decodeTransaction(input: unknown): SyntheticTransaction {
  const object = objectValue(input);
  exactKeys(
    object,
    [
      "transactionId",
      "occurredAt",
      "amountMinor",
      "currency",
      "reconciliationStatus",
      "sourceRecordDigest",
    ],
    ["merchantReference", "accountReference", "transactionType"],
  );
  const digest = strictString(object.sourceRecordDigest, 64);
  if (!HEX_DIGEST.test(digest)) {
    throw unavailable();
  }
  return {
    transactionId: strictString(object.transactionId, 128),
    occurredAt: utcSecond(object.occurredAt),
    amountMinor: strictString(object.amountMinor, 32),
    currency: strictString(object.currency, 3),
    ...optionalStrictString(object, "merchantReference", 128),
    ...optionalStrictString(object, "accountReference", 128),
    ...optionalStrictString(object, "transactionType", 128),
    reconciliationStatus: reconciliationStatus(object.reconciliationStatus),
    sourceRecordDigest: digest,
  };
}

function decodeFinalization(input: unknown): DatasetFinalization {
  const object = exactObject(input, ["finalizedAt", "checksum", "schemaFingerprint", "rowCount"]);
  const checksum = strictString(object.checksum, 64);
  const schemaFingerprint = strictString(object.schemaFingerprint, 64);
  if (!HEX_DIGEST.test(checksum) || !HEX_DIGEST.test(schemaFingerprint)) {
    throw unavailable();
  }
  return {
    finalizedAt: utcSecond(object.finalizedAt),
    checksum,
    schemaFingerprint,
    rowCount: nonNegativeInteger(object.rowCount),
  };
}

function exactObject(input: unknown, keys: readonly string[]): Record<string, unknown> {
  const object = objectValue(input);
  exactKeys(object, keys, []);
  return object;
}

function objectValue(input: unknown): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw unavailable();
  }
  const prototype = Object.getPrototypeOf(input);
  if (prototype !== Object.prototype && prototype !== null) {
    throw unavailable();
  }
  return input as Record<string, unknown>;
}

function exactKeys(
  object: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
): void {
  const allowed = new Set([...required, ...optional]);
  if (
    required.some((key) => !(key in object)) ||
    Object.keys(object).some((key) => !allowed.has(key))
  ) {
    throw unavailable();
  }
}

function arrayValue(input: unknown): readonly unknown[] {
  if (!Array.isArray(input)) {
    throw unavailable();
  }
  return input;
}

function boundedString(input: unknown, maximumBytes: number): string {
  if (
    typeof input !== "string" ||
    input.length === 0 ||
    Buffer.byteLength(input, "utf8") > maximumBytes
  ) {
    throw new DatasetClientError("INVALID_DATASET_REQUEST", 400);
  }
  return input;
}

function strictString(input: unknown, maximumBytes: number): string {
  if (
    typeof input !== "string" ||
    input.length === 0 ||
    Buffer.byteLength(input, "utf8") > maximumBytes
  ) {
    throw unavailable();
  }
  return input;
}

function strictId(input: unknown): string {
  const value = strictString(input, 128);
  if (!SAFE_ID.test(value)) {
    throw unavailable();
  }
  return value;
}

function optionalString(
  object: Record<string, unknown>,
  key: string,
  maximumBytes: number,
): Record<string, string> {
  return object[key] === undefined ? {} : { [key]: boundedString(object[key], maximumBytes) };
}

function optionalStrictString(
  object: Record<string, unknown>,
  key: string,
  maximumBytes: number,
): Record<string, string> {
  return object[key] === undefined ? {} : { [key]: strictString(object[key], maximumBytes) };
}

function reconciliationStatus(input: unknown): ReconciliationStatus {
  if (input === "UNMATCHED" || input === "MATCHED" || input === "REVIEW") {
    return input;
  }
  throw unavailable();
}

function datasetStatus(input: unknown): "DRAFT" | "FINALIZED" {
  if (input === "DRAFT" || input === "FINALIZED") {
    return input;
  }
  throw unavailable();
}

function utcSecond(input: unknown): string {
  const value = strictString(input, 32);
  if (!UTC_SECOND.test(value) || Number.isNaN(Date.parse(value))) {
    throw unavailable();
  }
  return value;
}

function positiveInteger(input: unknown): number {
  if (!Number.isSafeInteger(input) || (input as number) < 1) {
    throw unavailable();
  }
  return input as number;
}

function nonNegativeInteger(input: unknown): number {
  if (!Number.isSafeInteger(input) || (input as number) < 0) {
    throw unavailable();
  }
  return input as number;
}

function unavailable(): DatasetClientError {
  return new DatasetClientError("DATASET_UNAVAILABLE", 503);
}
