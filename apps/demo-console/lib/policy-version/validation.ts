import { isAbsolute, normalize } from "node:path";
import { PolicyCandidateError } from "../policy-candidate/types.ts";
import {
  loadPolicyCandidateConfig,
  validateCandidateConfig,
  validateCandidateReference,
  validateCandidateRequest,
} from "../policy-candidate/validation.ts";
import { validatePolicyVersionIdentity } from "../policy-draft/validation.ts";
import type {
  PolicyVersionAllocation,
  PolicyVersionConfig,
  PreparePolicyVersionRequest,
} from "./types.ts";

const ID = /^[0-9a-f]{32}$/;
const DIGEST = /^[0-9a-f]{64}$/;

export function validateVersionRequest(input: unknown): PreparePolicyVersionRequest {
  const value = object(input, ["draft", "expectedApprovalEpoch", "validForSeconds", "familyId"]);
  const request = validateCandidateRequest({
    draft: value.draft,
    expectedApprovalEpoch: value.expectedApprovalEpoch,
    validForSeconds: value.validForSeconds,
  });
  if (
    value.familyId !== undefined &&
    (typeof value.familyId !== "string" || !ID.test(value.familyId))
  ) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  // Reapproval is separate from allocating a new immutable version.
  if (request.expectedApprovalEpoch !== 1) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return {
    ...request,
    ...(value.familyId === undefined ? {} : { familyId: value.familyId as string }),
  };
}

export function validateVersionConfig(config: PolicyVersionConfig): PolicyVersionConfig {
  validateCandidateConfig(config);
  if (
    typeof config.versionRoot !== "string" ||
    !isAbsolute(config.versionRoot) ||
    normalize(config.versionRoot) !== config.versionRoot ||
    config.versionRoot === "/" ||
    config.versionRoot.includes("\0") ||
    config.versionRoot === config.candidateRoot ||
    config.versionRoot.startsWith(`${config.candidateRoot}/`) ||
    config.candidateRoot.startsWith(`${config.versionRoot}/`)
  ) {
    throw new PolicyCandidateError("INVALID_OWNER_CONFIGURATION");
  }
  return config;
}

export function loadPolicyVersionConfig(
  environment: NodeJS.ProcessEnv = process.env,
): PolicyVersionConfig {
  return validateVersionConfig({
    ...loadPolicyCandidateConfig(environment),
    versionRoot: environment.RAZIEL_POLICY_VERSION_ROOT ?? "",
  });
}

export function validateAllocation(input: unknown): PolicyVersionAllocation {
  const value = object(input, [
    "format_version",
    "candidate_id",
    "request_digest",
    "requested_family_id",
    "family_id",
    "version_id",
    "version",
  ]);
  if (
    value.format_version !== 1 ||
    typeof value.request_digest !== "string" ||
    !DIGEST.test(value.request_digest)
  ) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  validateCandidateReference(value.candidate_id);
  try {
    validatePolicyVersionIdentity({
      familyId: value.family_id,
      versionId: value.version_id,
      version: value.version,
    });
  } catch {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  if (value.requested_family_id !== null && value.requested_family_id !== value.family_id) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  if (value.requested_family_id === null && value.version !== 1) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  return value as unknown as PolicyVersionAllocation;
}

function object(input: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  const prototype = Object.getPrototypeOf(input);
  if (
    (prototype !== Object.prototype && prototype !== null) ||
    Object.keys(input).some((key) => !keys.includes(key))
  ) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return input as Record<string, unknown>;
}
