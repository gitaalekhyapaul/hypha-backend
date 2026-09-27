import type { PreparePolicyVersionRequest } from "../policy-version/types.ts";
import { PortalRequestError } from "../portal-client/browser.ts";
import { buildPolicyCandidateRequest, type PolicyBuilderState } from "./model.ts";

export function buildPolicyVersionRequest(
  state: PolicyBuilderState,
  candidateId: string,
  familyId?: string,
): PreparePolicyVersionRequest {
  const family = familyId?.trim();
  if (family !== undefined && !/^[0-9a-f]{32}$/.test(family)) {
    throw new Error("Enter a policy family ID containing 32 lowercase hexadecimal characters.");
  }
  // These compatibility fields are replaced by the owner allocator, never UI selections.
  const request = buildPolicyCandidateRequest(
    {
      ...state,
      policyName: "reconciliation-query",
      policyVersion: "1",
      resourceType: "dataset",
      expectedApprovalEpoch: "1",
    },
    candidateId,
  );
  return { ...request, ...(family === undefined ? {} : { familyId: family }) };
}

export function isUncertainPolicyVersionFailure(cause: unknown): boolean {
  return (
    cause instanceof PortalRequestError &&
    (cause.status >= 500 ||
      cause.status === 408 ||
      cause.status === 401 ||
      cause.code === "CANDIDATE_NOT_READY")
  );
}

export function policyVersionFailureMessage(cause: unknown): string {
  if (!(cause instanceof PortalRequestError)) {
    return cause instanceof Error ? cause.message : "Policy draft is invalid.";
  }
  switch (cause.code) {
    case "INVALID_CANDIDATE_REQUEST":
      return "The policy draft contains an invalid or unsupported value.";
    case "CANDIDATE_CONFLICT":
      return "This candidate conflicts with retained server state. Review it before starting another draft.";
    case "CANDIDATE_NOT_FOUND":
      return "No compiled candidate was found. Retry the same compile request.";
    case "CANDIDATE_NOT_READY":
      return "Compilation is not confirmed. Retry or refresh this candidate.";
    case "CANDIDATE_EXPIRED":
      return "This candidate expired. Start a new draft.";
    case "COMPILER_FAILED":
      return "The compiler rejected this policy. Edit the draft and compile again.";
    case "INSPECTION_FAILED":
      return "The compiled candidate failed independent inspection.";
    case "POLICY_ACTIVATION_CONFLICT":
      return "Publication conflicts with trusted registry or schema state. The version was not confirmed published.";
    case "POLICY_ACTIVATION_REJECTED":
      return "The owner publication checks rejected this version.";
    case "INVALID_POLICY_ACTIVATION_REQUEST":
      return "This candidate cannot be published.";
    default:
      return "The operation was not confirmed. Retry the same candidate.";
  }
}
