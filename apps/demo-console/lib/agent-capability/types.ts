export interface AgentSessionProjection {
  readonly key_thumbprint: string;
  readonly subject_type: "agent";
  readonly subject_id: string;
  readonly created_at_unix_seconds: string;
}

export interface PublishedAccessSelection {
  readonly agent: AgentSessionProjection;
  readonly agentId: string;
  readonly datasetId: string;
  readonly snapshotId: string;
  readonly policyHash: string;
  readonly relationshipRoot: string;
  readonly snapshotExpiresAtUnixSeconds: string;
}

export type CapabilityMode = "live" | "recorded";

export type CapabilityJobState =
  | "PROOF_JOB_STATE_QUEUED"
  | "PROOF_JOB_STATE_EXECUTING"
  | "PROOF_JOB_STATE_PROVING"
  | "PROOF_JOB_STATE_SUCCEEDED"
  | "PROOF_JOB_STATE_FAILED"
  | "PROOF_JOB_STATE_CANCELED";

export type CapabilityContextValue =
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "int"; readonly value: string }
  | { readonly kind: "string"; readonly value: string };

export interface CapabilityContextEntry {
  readonly name: string;
  readonly value: CapabilityContextValue;
}

export interface CapabilityRequestInput {
  readonly agent_id: string;
  readonly task_id: string;
  readonly purpose: string;
  readonly action: "query";
  readonly resource_type: "dataset";
  readonly resource_scope: string;
  readonly requested_valid_for_seconds: number;
  readonly snapshot_id: string;
  readonly snapshot_expires_at_unix_seconds: string;
  readonly request_context: readonly CapabilityContextEntry[];
}

export interface CreateCapabilityInput extends CapabilityRequestInput {
  readonly submission_id: string;
}

export interface CapabilityTimestamp {
  readonly seconds: string;
  readonly nanos: number;
}

export interface CapabilityMetricsProjection {
  readonly execution_milliseconds: string;
  readonly proving_milliseconds: string;
  readonly execution_cycle_count: string;
  readonly proof_size_bytes: string;
}

export interface PublicCapabilityProjection {
  readonly capability_id: string;
  readonly agent_key_thumbprint: string;
  readonly task_id: string;
  readonly purpose: string;
  readonly action: string;
  readonly resource_type: string;
  readonly resource_scope: string;
  readonly policy_hash: string;
  readonly relationship_snapshot_root: string;
  readonly snapshot_id: string;
  readonly issued_at: CapabilityTimestamp | null;
  readonly expires_at: CapabilityTimestamp | null;
  readonly proof_system: string;
  readonly verification_key_id: string;
  readonly proof_size_bytes: string;
}

export interface CapabilityJobProjection {
  readonly job_id: string;
  readonly state: CapabilityJobState;
  readonly submitted_at: CapabilityTimestamp | null;
  readonly updated_at: CapabilityTimestamp | null;
  readonly public_error_code: string;
  readonly metrics: CapabilityMetricsProjection | null;
  readonly capability: PublicCapabilityProjection | null;
  readonly mode: CapabilityMode;
  readonly authorizes_requests: boolean;
  readonly recorded_source: string | null;
}

export interface CapabilityProcessConfig {
  readonly mode: CapabilityMode;
  readonly executable: string | null;
  readonly endpoint: string | null;
}

export class CapabilityFlowError extends Error {
  public readonly code:
    | "CAPABILITY_BUSY"
    | "CAPABILITY_CONFLICT"
    | "CAPABILITY_NOT_FOUND"
    | "CAPABILITY_UNAVAILABLE"
    | "INVALID_CAPABILITY_REQUEST";

  public constructor(code: CapabilityFlowError["code"]) {
    super(code);
    this.name = "CapabilityFlowError";
    this.code = code;
  }
}
