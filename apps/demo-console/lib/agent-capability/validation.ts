import type {
  CapabilityContextEntry,
  CapabilityJobProjection,
  CapabilityJobState,
  CapabilityMetricsProjection,
  CapabilityTimestamp,
  CreateCapabilityInput,
  PublicCapabilityProjection,
} from "./types.ts";
import { CapabilityFlowError } from "./types.ts";

const MIN_VALIDITY_SECONDS = 75 * 60;
const MAX_VALIDITY_SECONDS = 24 * 60 * 60;
const JOB_STATES = new Set<CapabilityJobState>([
  "PROOF_JOB_STATE_QUEUED",
  "PROOF_JOB_STATE_EXECUTING",
  "PROOF_JOB_STATE_PROVING",
  "PROOF_JOB_STATE_SUCCEEDED",
  "PROOF_JOB_STATE_FAILED",
  "PROOF_JOB_STATE_CANCELED",
]);

export function validateCreateCapabilityInput(
  input: unknown,
  nowUnixSeconds: number,
): CreateCapabilityInput {
  const object = record(input);
  exactKeys(object, [
    "action",
    "agent_id",
    "purpose",
    "request_context",
    "requested_valid_for_seconds",
    "resource_scope",
    "resource_type",
    "snapshot_expires_at_unix_seconds",
    "snapshot_id",
    "task_id",
  ]);
  const requestedValidity = integer(object.requested_valid_for_seconds);
  if (requestedValidity < MIN_VALIDITY_SECONDS || requestedValidity > MAX_VALIDITY_SECONDS) {
    invalid();
  }
  const snapshotExpiresAt = decimalInteger(object.snapshot_expires_at_unix_seconds);
  if (BigInt(nowUnixSeconds + requestedValidity) > snapshotExpiresAt) {
    invalid();
  }
  const action = identifier(object.action);
  const resourceType = identifier(object.resource_type);
  if (action !== "query" || resourceType !== "dataset") {
    invalid();
  }
  if (!Array.isArray(object.request_context) || object.request_context.length > 16) {
    invalid();
  }
  return {
    agent_id: identifier(object.agent_id),
    task_id: identifier(object.task_id),
    purpose: identifier(object.purpose),
    action,
    resource_type: resourceType,
    resource_scope: identifier(object.resource_scope),
    requested_valid_for_seconds: requestedValidity,
    snapshot_id: snapshotIdentifier(object.snapshot_id),
    snapshot_expires_at_unix_seconds: snapshotExpiresAt.toString(),
    request_context: object.request_context.map(contextEntry),
  };
}

export function validateCapabilityJobProjection(input: unknown): CapabilityJobProjection {
  const object = record(input);
  const state = stringValue(object.state) as CapabilityJobState;
  if (!JOB_STATES.has(state)) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  const mode = object.mode;
  if (mode !== "live" && mode !== "recorded") {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  const capability = object.capability === null ? null : capabilityProjection(object.capability);
  const authorizesRequests = object.authorizes_requests;
  if (typeof authorizesRequests !== "boolean" || (mode === "recorded" && authorizesRequests)) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return {
    job_id: identifier(object.job_id),
    state,
    submitted_at: nullableTimestamp(object.submitted_at),
    updated_at: nullableTimestamp(object.updated_at),
    public_error_code: identifier(object.public_error_code),
    metrics: object.metrics === null ? null : metricsProjection(object.metrics),
    capability,
    mode,
    authorizes_requests: authorizesRequests,
    recorded_source: object.recorded_source === null ? null : identifier(object.recorded_source),
  };
}

function contextEntry(input: unknown): CapabilityContextEntry {
  const object = record(input);
  exactKeys(object, ["name", "value"]);
  const value = record(object.value);
  const kind = value.kind;
  if (kind === "bool" && typeof value.value === "boolean") {
    exactKeys(value, ["kind", "value"]);
    return { name: identifier(object.name), value: { kind, value: value.value } };
  }
  if (kind === "int") {
    exactKeys(value, ["kind", "value"]);
    return {
      name: identifier(object.name),
      value: { kind, value: signedInteger(value.value).toString() },
    };
  }
  if (kind === "string") {
    exactKeys(value, ["kind", "value"]);
    return { name: identifier(object.name), value: { kind, value: identifier(value.value) } };
  }
  invalid();
}

function capabilityProjection(input: unknown): PublicCapabilityProjection {
  const object = record(input);
  return {
    capability_id: identifier(object.capability_id),
    agent_key_thumbprint: digest(object.agent_key_thumbprint),
    task_id: identifier(object.task_id),
    purpose: identifier(object.purpose),
    action: identifier(object.action),
    resource_type: identifier(object.resource_type),
    resource_scope: identifier(object.resource_scope),
    policy_hash: digest(object.policy_hash),
    relationship_snapshot_root: digest(object.relationship_snapshot_root),
    snapshot_id: snapshotIdentifier(object.snapshot_id),
    issued_at: nullableTimestamp(object.issued_at),
    expires_at: nullableTimestamp(object.expires_at),
    proof_system: identifier(object.proof_system),
    verification_key_id: identifier(object.verification_key_id),
    proof_size_bytes: unsignedInteger(object.proof_size_bytes).toString(),
  };
}

function metricsProjection(input: unknown): CapabilityMetricsProjection {
  const object = record(input);
  return {
    execution_milliseconds: unsignedInteger(object.execution_milliseconds).toString(),
    proving_milliseconds: unsignedInteger(object.proving_milliseconds).toString(),
    execution_cycle_count: unsignedInteger(object.execution_cycle_count).toString(),
    proof_size_bytes: unsignedInteger(object.proof_size_bytes).toString(),
  };
}

function nullableTimestamp(input: unknown): CapabilityTimestamp | null {
  if (input === null) {
    return null;
  }
  const object = record(input);
  const nanos = integer(object.nanos);
  if (nanos < 0 || nanos >= 1_000_000_000) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return { seconds: decimalInteger(object.seconds).toString(), nanos };
}

function record(input: unknown): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    invalid();
  }
  return input as Record<string, unknown>;
}

function exactKeys(object: Record<string, unknown>, expected: readonly string[]): void {
  const actual = Object.keys(object).sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    invalid();
  }
}

function identifier(input: unknown): string {
  if (typeof input !== "string") {
    invalid();
  }
  const value = input.trim();
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,255}$/.test(value)) {
    invalid();
  }
  return value;
}

function snapshotIdentifier(input: unknown): string {
  const value = identifier(input);
  if (!/^snapshot-v1:[0-9a-f]{64}$/.test(value)) {
    invalid();
  }
  return value;
}

function digest(input: unknown): string {
  if (typeof input !== "string" || !/^[0-9a-f]{64}$/.test(input)) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return input;
}

function integer(input: unknown): number {
  if (typeof input !== "number" || !Number.isSafeInteger(input)) {
    invalid();
  }
  return input;
}

function signedInteger(input: unknown): bigint {
  if (typeof input !== "string" || !/^-?[0-9]+$/.test(input)) {
    invalid();
  }
  return BigInt(input);
}

function unsignedInteger(input: unknown): bigint {
  if (typeof input !== "string" || !/^[0-9]+$/.test(input)) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return BigInt(input);
}

function decimalInteger(input: unknown): bigint {
  if (typeof input !== "string" || !/^[0-9]+$/.test(input)) {
    invalid();
  }
  return BigInt(input);
}

function stringValue(input: unknown): string {
  if (typeof input !== "string") {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return input;
}

function invalid(): never {
  throw new CapabilityFlowError("INVALID_CAPABILITY_REQUEST");
}
