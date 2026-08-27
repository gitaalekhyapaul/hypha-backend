import type { SyntheticTransaction } from "../dataset-client/types.ts";

export interface ProtectedReconciliationQueryInput {
  readonly job_id: string;
  readonly dataset_id: string;
  readonly from_occurred_at: string;
  readonly to_occurred_at: string;
  readonly limit: number;
}

export interface PublicReconciliationReceipt {
  readonly format_version: 1;
  readonly receipt_id: string;
  readonly receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST";
  readonly request_id: string;
  readonly occurred_at: string;
  readonly task_id: string;
  readonly capability_id: string;
  readonly proof_job_id: string;
  readonly policy_record_id: string;
  readonly policy_hash: string;
  readonly snapshot_id: string;
  readonly relationship_snapshot_root: string;
  readonly public_claims_digest: string;
  readonly proof_digest: string;
  readonly action: "query";
  readonly resource_type: "dataset";
  readonly resource_scope: string;
  readonly outcome: "AUDIT_OUTCOME_ACCEPTED";
  readonly reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED";
}

export interface ProtectedReconciliationQueryResult {
  readonly classification: "SYNTHETIC";
  readonly dataset_id: string;
  readonly row_count: number;
  readonly rows: readonly SyntheticTransaction[];
  readonly receipt: PublicReconciliationReceipt;
}
