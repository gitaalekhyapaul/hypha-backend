import type {
  PolicyCandidateConfig,
  PolicyCandidateProjection,
  PreparePolicyCandidateRequest,
} from "../policy-candidate/types.ts";
import type { PolicyVersionIdentity } from "../policy-draft/types.ts";

export interface PreparePolicyVersionRequest extends PreparePolicyCandidateRequest {
  readonly familyId?: string;
}

export interface PolicyVersionConfig extends PolicyCandidateConfig {
  readonly versionRoot: string;
}

export interface PolicyVersionCandidate {
  readonly identity: PolicyVersionIdentity;
  readonly candidate: PolicyCandidateProjection;
}

export interface PolicyVersionAllocation {
  readonly format_version: 1;
  readonly candidate_id: string;
  readonly request_digest: string;
  readonly requested_family_id: string | null;
  readonly family_id: string;
  readonly version_id: string;
  readonly version: number;
}
