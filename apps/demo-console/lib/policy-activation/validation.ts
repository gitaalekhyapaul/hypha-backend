import { isAbsolute, normalize } from "node:path";

import { validateCandidateReference } from "../policy-candidate/index.ts";
import {
  type PolicyActivationConfig,
  PolicyActivationError,
  type PolicyActivationProjection,
} from "./types.ts";

const PUBLIC_IDENTIFIER = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const HEX_DIGEST = /^[0-9a-f]{64}$/;
const POSITIVE_INTEGER = /^[1-9][0-9]*$/;

export function loadPolicyActivationConfig(
  environment: NodeJS.ProcessEnv = process.env,
): PolicyActivationConfig {
  const config = {
    executable: environment.RAZIEL_POLICY_ACTIVATION_EXECUTABLE ?? "",
    socketPath: environment.RAZIEL_POLICY_ACTIVATION_SOCKET_PATH ?? "",
  };
  if (!safeAbsolutePath(config.executable) || !safeAbsolutePath(config.socketPath)) {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  return config;
}

export function validateActivationIdentity(
  candidateId: unknown,
  approvedBy: unknown,
): {
  readonly candidateId: string;
  readonly approvedBy: string;
} {
  let validatedCandidateId: string;
  try {
    validatedCandidateId = validateCandidateReference(candidateId);
  } catch {
    throw new PolicyActivationError("INVALID_POLICY_ACTIVATION_REQUEST");
  }
  if (typeof approvedBy !== "string" || !PUBLIC_IDENTIFIER.test(approvedBy)) {
    throw new PolicyActivationError("INVALID_POLICY_ACTIVATION_REQUEST");
  }
  return { candidateId: validatedCandidateId, approvedBy };
}

export function validateActivationProjection(input: unknown): PolicyActivationProjection {
  const value = exactObject(input, [
    "record_id",
    "policy_name",
    "policy_version",
    "policy_hash",
    "approval_epoch",
    "status",
    "registered_at",
    "valid_from",
    "valid_until",
    "registry_generation",
  ]);
  if (
    typeof value.record_id !== "string" ||
    !PUBLIC_IDENTIFIER.test(value.record_id) ||
    typeof value.policy_name !== "string" ||
    !PUBLIC_IDENTIFIER.test(value.policy_name) ||
    typeof value.policy_version !== "string" ||
    !POSITIVE_INTEGER.test(value.policy_version) ||
    typeof value.policy_hash !== "string" ||
    !HEX_DIGEST.test(value.policy_hash) ||
    typeof value.approval_epoch !== "string" ||
    !POSITIVE_INTEGER.test(value.approval_epoch) ||
    value.status !== "APPROVED" ||
    typeof value.registry_generation !== "string" ||
    !POSITIVE_INTEGER.test(value.registry_generation)
  ) {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  return {
    record_id: value.record_id,
    policy_name: value.policy_name,
    policy_version: value.policy_version,
    policy_hash: value.policy_hash,
    approval_epoch: value.approval_epoch,
    status: value.status,
    registered_at: publicTimestamp(value.registered_at),
    valid_from: publicTimestamp(value.valid_from),
    valid_until: publicTimestamp(value.valid_until),
    registry_generation: value.registry_generation,
  };
}

function publicTimestamp(input: unknown): { readonly seconds: string; readonly nanos: 0 } {
  const value = exactObject(input, ["seconds", "nanos"]);
  if (
    typeof value.seconds !== "string" ||
    !POSITIVE_INTEGER.test(value.seconds) ||
    value.nanos !== 0
  ) {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  return { seconds: value.seconds, nanos: 0 };
}

function exactObject(input: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  const value = input as Record<string, unknown>;
  if (
    Object.keys(value).length !== keys.length ||
    keys.some((key) => !(key in value)) ||
    Object.keys(value).some((key) => !keys.includes(key))
  ) {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  return value;
}

function safeAbsolutePath(value: string): boolean {
  return (
    isAbsolute(value) &&
    normalize(value) === value &&
    value !== "/" &&
    !value.includes("\0") &&
    !value.endsWith("/.") &&
    !value.endsWith("/..")
  );
}
