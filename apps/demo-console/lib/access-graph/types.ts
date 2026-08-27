import type { PolicyActivationProjection } from "../policy-activation/types.ts";
import type { PolicyBuilderState } from "../policy-builder/index.ts";
import type { PolicyCandidateProjection, PublicTimestamp } from "../policy-candidate/types.ts";

export type AccessGraphStoredValue =
  | { readonly type: "bool"; readonly value: boolean }
  | { readonly type: "int"; readonly value: number }
  | { readonly type: "string"; readonly value: string };

export interface AccessGraphStoredContext {
  readonly name: string;
  readonly value: AccessGraphStoredValue;
}

export interface AccessGraphStoredCaveat {
  readonly name: string;
  readonly context: readonly AccessGraphStoredContext[];
}

export interface AccessGraphRelationship {
  readonly resource_type: string;
  readonly resource_id: string;
  readonly relation: string;
  readonly subject_type: string;
  readonly subject_id: string;
  readonly subject_relation?: string;
  readonly caveat?: AccessGraphStoredCaveat;
}

export interface PublishAccessGraphRequest {
  readonly candidate_id: string;
  readonly valid_for_seconds: number;
  readonly relationships: readonly AccessGraphRelationship[];
}

export interface AccessGraphPublicationProjection {
  readonly policy_record_id: string;
  readonly policy_hash: string;
  readonly snapshot_id: string;
  readonly relationship_root: string;
  readonly epoch: string;
  readonly issued_at: PublicTimestamp;
  readonly expires_at: PublicTimestamp;
  readonly accepted_relationship_count: number;
  readonly registry_generation: string;
  readonly status: "ACCEPTED";
}

export interface ActivePolicySelection {
  readonly activation: PolicyActivationProjection;
  readonly candidate: PolicyCandidateProjection;
  readonly builder: PolicyBuilderState;
}

export interface AccessGraphFormState {
  readonly agentId: string;
  readonly datasetId: string;
  readonly teamId: string;
  readonly organizationId: string;
  readonly validForSeconds: string;
  readonly approvedAmount: string;
  readonly isApproved: boolean;
  readonly isSuspended: boolean;
  readonly isOverride: boolean;
}

export interface AccessGraphProcessConfig {
  readonly executable: string;
  readonly socketPath: string;
}

export type AccessGraphFailureCode =
  | "INVALID_ACCESS_GRAPH_REQUEST"
  | "ACCESS_GRAPH_CONFLICT"
  | "ACCESS_GRAPH_REJECTED"
  | "ACCESS_GRAPH_UNAVAILABLE";

export class AccessGraphError extends Error {
  public readonly code: AccessGraphFailureCode;

  public constructor(code: AccessGraphFailureCode) {
    super(code);
    this.name = "AccessGraphError";
    this.code = code;
  }
}
