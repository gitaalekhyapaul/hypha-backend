import type { ReconciliationQuery, SyntheticTransaction } from "../reconciliation/model";

export const PROTECTED_REQUEST_FORMAT_VERSION = 1;
export const PROTECTED_QUERY_MAX_BODY_BYTES = 1_048_576;
export const PROTECTED_QUERY_MAX_ENVELOPE_BYTES = 6 * 1_048_576;
export const MAX_REQUEST_AGE_SECONDS = 300n;
export const MAX_REQUEST_FUTURE_SKEW_SECONDS = 30n;
// M5 accepts capabilities with validity windows up to 24 hours.
export const MAX_M5_CAPABILITY_VALIDITY_SECONDS = 24 * 60 * 60;

export interface ProtectedRequestEnvelope {
  readonly formatVersion: typeof PROTECTED_REQUEST_FORMAT_VERSION;
  readonly capabilityProtoHex: string;
  readonly agentPublicKeyHex: string;
  readonly requestMethod: "POST";
  readonly canonicalRequestTarget: string;
  readonly requestBody: Uint8Array;
  readonly requestBodyHashHex: string;
  readonly requestNonceHex: string;
  readonly requestTimestampUnixSeconds: bigint;
  readonly agentRequestSignatureHex: string;
  readonly query: ReconciliationQuery;
}

export interface VerifiedCapabilityClaims {
  readonly formatVersion: number;
  readonly capabilityId: string;
  readonly agentKeyThumbprintHex: string;
  readonly taskId: string;
  readonly purpose: string;
  readonly action: string;
  readonly resourceType: string;
  readonly resourceScope: string;
  readonly policyHashHex: string;
  readonly relationshipSnapshotRootHex: string;
  readonly snapshotId: string;
  readonly issuedAtUnixSeconds: bigint;
  readonly expiresAtUnixSeconds: bigint;
}

export interface InspectedCapability {
  readonly capabilityId: string;
  readonly proofDigestHex: string;
}

export type VerifierResult =
  | { readonly outcome: "rejected" }
  | {
      readonly outcome: "verified";
      readonly claims: VerifiedCapabilityClaims;
      readonly policyRecordId: string;
      readonly proofDigestHex: string;
    };

export interface ProtectedRequestVerifier {
  inspectCapability(envelope: ProtectedRequestEnvelope): Promise<InspectedCapability>;
  verify(envelope: ProtectedRequestEnvelope): Promise<VerifierResult>;
}

export type VerifiedCapability = Extract<VerifierResult, { readonly outcome: "verified" }>;

export interface VerificationCache {
  get(capabilityId: string, proofDigestHex: string): Promise<VerifiedCapability | undefined>;
  put(verification: VerifiedCapability, ttlSeconds: number): Promise<void>;
}

export interface ProtectedRequestSignatureVerifier {
  verify(capabilityId: string, envelope: ProtectedRequestEnvelope): Promise<boolean>;
}

export interface NonceStore {
  claim(capabilityId: string, requestNonceHex: string, ttlSeconds: number): Promise<boolean>;
}

export interface PublicReceipt {
  readonly format_version: 1;
  readonly receipt_id: string;
  readonly receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST";
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
  readonly outcome: "AUDIT_OUTCOME_ACCEPTED" | "AUDIT_OUTCOME_REJECTED";
  readonly reason_code:
    | "PUBLIC_ERROR_CODE_UNSPECIFIED"
    | "PUBLIC_ERROR_CODE_NOT_AUTHORIZED"
    | "PUBLIC_ERROR_CODE_REQUEST_REPLAYED";
}

export interface ReceiptStore {
  append(receipt: PublicReceipt): Promise<void>;
}

export interface ProtectedQueryResult {
  readonly classification: "SYNTHETIC";
  readonly dataset_id: string;
  readonly verification_path: "VERIFIER" | "CACHE";
  readonly row_count: number;
  readonly rows: readonly SyntheticTransaction[];
  readonly receipt: PublicReceipt;
}

export interface ProtectedQueryDependencies {
  readonly verifier: ProtectedRequestVerifier;
  readonly verificationCache: VerificationCache;
  readonly signatureVerifier: ProtectedRequestSignatureVerifier;
  readonly nonceStore: NonceStore;
  readonly receiptStore: ReceiptStore;
  readonly queryDataset: (
    datasetId: string,
    query: ReconciliationQuery,
  ) => Promise<readonly SyntheticTransaction[]>;
  readonly nowUnixSeconds?: () => bigint;
}
