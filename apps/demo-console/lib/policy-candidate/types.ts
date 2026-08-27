import type { PolicyDraftV1 } from "../policy-draft/index.ts";

export const POLICY_CANDIDATE_STATE_FORMAT_VERSION = 1 as const;

export type PolicyCandidateFailureCode =
  | "CANDIDATE_CONFLICT"
  | "CANDIDATE_EXPIRED"
  | "CANDIDATE_NOT_FOUND"
  | "CANDIDATE_NOT_READY"
  | "CANDIDATE_STORE_UNAVAILABLE"
  | "COMPILER_FAILED"
  | "INSPECTION_FAILED"
  | "INVALID_CANDIDATE_REQUEST"
  | "INVALID_OWNER_CONFIGURATION";

export interface PreparePolicyCandidateRequest {
  readonly draft: PolicyDraftV1;
  readonly expectedApprovalEpoch: number;
  readonly validForSeconds: number;
}

export interface PublicTimestamp {
  readonly seconds: string;
  readonly nanos: 0;
}

export interface PolicyCandidateProjection {
  readonly candidate_id: string;
  readonly compiler_version: string;
  readonly record_id: string;
  readonly policy_name: string;
  readonly policy_version: string;
  readonly resource_type: string;
  readonly permission: "query";
  readonly rule_summary: string;
  readonly source_schema_digest: string;
  readonly policy_hash: string;
  readonly expected_approval_epoch: string;
  readonly valid_from: PublicTimestamp;
  readonly valid_until: PublicTimestamp;
  readonly status: "COMPILED";
  readonly created_at: PublicTimestamp;
  readonly expires_at: PublicTimestamp;
}

export interface PolicyCandidateConfig {
  readonly candidateRoot: string;
  readonly compilerExecutable: string;
  readonly adminExecutable: string;
}

export interface ProcessInvocation {
  readonly executable: string;
  readonly args: readonly string[];
  readonly cwd: string;
  readonly timeoutMs: number;
  readonly maxOutputBytes: number;
}

export type ProcessRunner = (invocation: ProcessInvocation) => Promise<void>;

export interface PolicyCandidateDependencies {
  readonly nowUnixSeconds?: () => number;
  readonly runProcess?: ProcessRunner;
}

export class PolicyCandidateError extends Error {
  public readonly code: PolicyCandidateFailureCode;

  public constructor(code: PolicyCandidateFailureCode) {
    super(code);
    this.name = "PolicyCandidateError";
    this.code = code;
  }
}
