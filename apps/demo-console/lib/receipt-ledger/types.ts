export const RECEIPT_FILTER_FIELDS = [
  "task_id",
  "capability_id",
  "proof_job_id",
  "policy_record_id",
  "snapshot_id",
  "request_id",
] as const;

export type ReceiptFilterField = (typeof RECEIPT_FILTER_FIELDS)[number];
export type ReceiptExportFormat = "json" | "jsonl";

export interface ReceiptLedgerQuery {
  readonly format: ReceiptExportFormat;
  readonly filter?: {
    readonly field: ReceiptFilterField;
    readonly value: string;
  };
}

export interface PublicAuditReceipt {
  readonly format_version: 1;
  readonly receipt_id: string;
  readonly receipt_type:
    | "AUDIT_RECEIPT_TYPE_CAPABILITY_VERIFICATION"
    | "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST";
  readonly occurred_at: string;
  readonly request_id: string;
  readonly task_id: string;
  readonly capability_id: string;
  readonly proof_job_id: string;
  readonly policy_record_id: string;
  readonly policy_hash: string;
  readonly snapshot_id: string;
  readonly relationship_snapshot_root: string;
  readonly public_claims_digest: string;
  readonly proof_digest: string;
  readonly action: string;
  readonly resource_type: string;
  readonly resource_scope: string;
  readonly outcome: "AUDIT_OUTCOME_ACCEPTED" | "AUDIT_OUTCOME_REJECTED" | "AUDIT_OUTCOME_ERROR";
  readonly reason_code:
    | "PUBLIC_ERROR_CODE_UNSPECIFIED"
    | "PUBLIC_ERROR_CODE_INVALID_REQUEST"
    | "PUBLIC_ERROR_CODE_NOT_AUTHORIZED"
    | "PUBLIC_ERROR_CODE_CAPABILITY_EXPIRED"
    | "PUBLIC_ERROR_CODE_CAPABILITY_INVALID"
    | "PUBLIC_ERROR_CODE_REQUEST_REPLAYED"
    | "PUBLIC_ERROR_CODE_SERVICE_UNAVAILABLE";
}

export interface PublicReceiptLedger {
  readonly classification: "PUBLIC_AUDIT";
  readonly total_count: number;
  readonly filtered_count: number;
  readonly receipts: readonly PublicAuditReceipt[];
}

export type ReceiptLedgerFailureCode = "INVALID_RECEIPT_QUERY" | "RECEIPT_LEDGER_UNAVAILABLE";

export class ReceiptLedgerError extends Error {
  public readonly code: ReceiptLedgerFailureCode;
  public readonly status: number;

  public constructor(code: ReceiptLedgerFailureCode, status: number) {
    super(code);
    this.name = "ReceiptLedgerError";
    this.code = code;
    this.status = status;
  }
}
