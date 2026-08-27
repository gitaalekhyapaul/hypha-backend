import type { PublicTimestamp } from "../policy-candidate/types.ts";

export type PolicyActivationFailureCode =
  | "INVALID_POLICY_ACTIVATION_REQUEST"
  | "POLICY_ACTIVATION_CONFLICT"
  | "POLICY_ACTIVATION_REJECTED"
  | "POLICY_ACTIVATION_UNAVAILABLE";

export interface PolicyActivationProjection {
  readonly record_id: string;
  readonly policy_name: string;
  readonly policy_version: string;
  readonly policy_hash: string;
  readonly approval_epoch: string;
  readonly status: "APPROVED";
  readonly registered_at: PublicTimestamp;
  readonly valid_from: PublicTimestamp;
  readonly valid_until: PublicTimestamp;
  readonly registry_generation: string;
}

export interface PolicyActivationConfig {
  readonly executable: string;
  readonly socketPath: string;
}

export class PolicyActivationError extends Error {
  public readonly code: PolicyActivationFailureCode;

  public constructor(code: PolicyActivationFailureCode) {
    super(code);
    this.name = "PolicyActivationError";
    this.code = code;
  }
}
