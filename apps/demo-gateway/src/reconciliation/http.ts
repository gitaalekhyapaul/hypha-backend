import { NextResponse } from "next/server";

import {
  type DatasetDraftInput,
  type ReconciliationQuery,
  type ReconciliationStatus,
  ReconciliationValidationError,
  type SyntheticTransactionInput,
} from "./model";
import { DatasetConflictError, DatasetNotFoundError } from "./store";

const MAX_OPERATOR_BODY_BYTES = 1_048_576;

export async function readOperatorJson(request: Request): Promise<unknown> {
  const raw = await request.text();
  if (new TextEncoder().encode(raw).byteLength > MAX_OPERATOR_BODY_BYTES) {
    throw new ReconciliationValidationError("operator request is too large");
  }
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    throw new ReconciliationValidationError("operator request is not valid JSON");
  }
}

export function parseDatasetDraft(value: unknown): DatasetDraftInput {
  const object = record(value, "dataset draft");
  const rows = array(object.rows, "dataset rows").map(parseTransaction);
  return {
    datasetId: string(object.datasetId, "dataset identifier"),
    sourceId: string(object.sourceId, "source identifier"),
    sourceLabel: string(object.sourceLabel, "source label"),
    rows,
  };
}

export function parseCloneRequest(value: unknown): { readonly targetDatasetId: string } {
  const object = record(value, "clone request");
  return { targetDatasetId: string(object.targetDatasetId, "target dataset identifier") };
}

export function parseReconciliationQuery(value: unknown): ReconciliationQuery {
  const object = record(value, "reconciliation query");
  return {
    fromOccurredAt: string(object.fromOccurredAt, "query start"),
    toOccurredAt: string(object.toOccurredAt, "query end"),
    ...(object.transactionId !== undefined
      ? { transactionId: string(object.transactionId, "transaction identifier") }
      : {}),
    ...(object.merchantReference !== undefined
      ? { merchantReference: string(object.merchantReference, "merchant reference") }
      : {}),
    ...(object.limit !== undefined ? { limit: number(object.limit, "query limit") } : {}),
  };
}

export function reconciliationErrorResponse(error: unknown): NextResponse {
  if (error instanceof DatasetNotFoundError) {
    return NextResponse.json({ error: "DATASET_NOT_FOUND" }, { status: 404 });
  }
  if (error instanceof DatasetConflictError) {
    return NextResponse.json(
      { error: "DATASET_CONFLICT", message: error.message },
      { status: 409 },
    );
  }
  if (error instanceof ReconciliationValidationError) {
    return NextResponse.json(
      { error: "INVALID_SYNTHETIC_DATASET", message: error.message },
      { status: 400 },
    );
  }
  return NextResponse.json({ error: "DATASET_UNAVAILABLE" }, { status: 503 });
}

function parseTransaction(value: unknown): SyntheticTransactionInput {
  const object = record(value, "transaction row");
  return {
    transactionId: string(object.transactionId, "transaction identifier"),
    occurredAt: string(object.occurredAt, "transaction time"),
    amountMinor: string(object.amountMinor, "amount_minor"),
    currency: string(object.currency, "currency"),
    ...(object.merchantReference !== undefined
      ? { merchantReference: string(object.merchantReference, "merchant reference") }
      : {}),
    ...(object.accountReference !== undefined
      ? { accountReference: string(object.accountReference, "account reference") }
      : {}),
    ...(object.transactionType !== undefined
      ? { transactionType: string(object.transactionType, "transaction type") }
      : {}),
    reconciliationStatus: status(object.reconciliationStatus),
  };
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ReconciliationValidationError(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new ReconciliationValidationError(`${label} must be an array`);
  }
  return value;
}

function string(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new ReconciliationValidationError(`${label} must be a string`);
  }
  return value;
}

function number(value: unknown, label: string): number {
  if (typeof value !== "number") {
    throw new ReconciliationValidationError(`${label} must be a number`);
  }
  return value;
}

function status(value: unknown): ReconciliationStatus {
  if (value === "UNMATCHED" || value === "MATCHED" || value === "REVIEW") {
    return value;
  }
  throw new ReconciliationValidationError("reconciliation status is unsupported");
}
