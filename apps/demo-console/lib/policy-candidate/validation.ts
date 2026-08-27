import { isAbsolute, normalize } from "node:path";

import { type PolicyDraftV1, validatePolicyDraft } from "../policy-draft/index.ts";
import {
  type PolicyCandidateConfig,
  PolicyCandidateError,
  type PreparePolicyCandidateRequest,
} from "./types.ts";

const MIN_POLICY_LIFETIME_SECONDS = 15 * 60;
const MAX_POLICY_LIFETIME_SECONDS = 31 * 24 * 60 * 60;
const PUBLIC_IDENTIFIER = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;

export function validateCandidateRequest(input: unknown): PreparePolicyCandidateRequest {
  const request = objectAt(input, ["draft", "expectedApprovalEpoch", "validForSeconds"]);
  let draft: PolicyDraftV1;
  try {
    draft = validatePolicyDraft(request.draft);
  } catch {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return {
    draft,
    expectedApprovalEpoch: positiveSafeInteger(request.expectedApprovalEpoch),
    validForSeconds: boundedInteger(
      request.validForSeconds,
      MIN_POLICY_LIFETIME_SECONDS,
      MAX_POLICY_LIFETIME_SECONDS,
    ),
  };
}

export function validateCandidateConfig(config: PolicyCandidateConfig): PolicyCandidateConfig {
  for (const value of [config.candidateRoot, config.compilerExecutable, config.adminExecutable]) {
    if (!isSafeAbsolutePath(value)) {
      throw new PolicyCandidateError("INVALID_OWNER_CONFIGURATION");
    }
  }
  return config;
}

export function validateCandidateReference(input: unknown): string {
  if (typeof input !== "string" || !PUBLIC_IDENTIFIER.test(input)) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return input;
}

export function loadPolicyCandidateConfig(
  environment: NodeJS.ProcessEnv = process.env,
): PolicyCandidateConfig {
  return validateCandidateConfig({
    candidateRoot: environment.RAZIEL_POLICY_CANDIDATE_ROOT ?? "",
    compilerExecutable: environment.RAZIEL_POLICY_COMPILER_EXECUTABLE ?? "",
    adminExecutable: environment.RAZIEL_ADMIN_EXECUTABLE ?? "",
  });
}

function objectAt(input: unknown, allowedKeys: readonly string[]): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  const prototype = Object.getPrototypeOf(input);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  const object = input as Record<string, unknown>;
  const allowed = new Set(allowedKeys);
  if (Object.keys(object).some((key) => !allowed.has(key))) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return object;
}

function positiveSafeInteger(input: unknown): number {
  if (!Number.isSafeInteger(input) || (input as number) < 1) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return input as number;
}

function boundedInteger(input: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(input) || (input as number) < minimum || (input as number) > maximum) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return input as number;
}

function isSafeAbsolutePath(value: string): boolean {
  return (
    isAbsolute(value) &&
    normalize(value) === value &&
    value !== "/" &&
    !value.includes("\0") &&
    !value.endsWith("/.") &&
    !value.endsWith("/..")
  );
}
