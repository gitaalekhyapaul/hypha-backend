import {
  type LiveCapabilityBinding,
  liveJobBinding,
  registerLiveJob,
  retainLiveCapability,
} from "./live-registry.ts";
import { runCapabilityClient } from "./process.ts";
import { cancelRecordedJob, createRecordedJob, getRecordedJob } from "./recorded.ts";
import { type AuthorizationContextEntry, encodeHex, signCapabilityRequest } from "./sdk.ts";
import { operatorAgentIdentity } from "./session-store.ts";
import type {
  CapabilityJobProjection,
  CapabilityProcessConfig,
  CreateCapabilityInput,
} from "./types.ts";
import { CapabilityFlowError } from "./types.ts";
import { validateCreateCapabilityInput } from "./validation.ts";

export async function createCapability(
  input: unknown,
  operatorId: string,
  config: CapabilityProcessConfig,
  nowUnixSeconds = Math.floor(Date.now() / 1000),
): Promise<CapabilityJobProjection> {
  const request = validateCreateCapabilityInput(input, nowUnixSeconds);
  const identity = await operatorAgentIdentity(operatorId);
  const signed = await signCapabilityRequest(identity, {
    agentId: request.agent_id,
    taskId: request.task_id,
    purpose: request.purpose,
    action: request.action,
    resourceType: request.resource_type,
    resourceScope: request.resource_scope,
    requestedExpiresAtUnixSeconds: BigInt(nowUnixSeconds + request.requested_valid_for_seconds),
    snapshotId: request.snapshot_id,
    requestContext: sdkContext(request),
  });
  const expectedJobId = encodeHex(signed.capabilityRequestCommitment);
  if (config.mode === "recorded") {
    return createRecordedJob(expectedJobId, operatorId);
  }
  const binding: LiveCapabilityBinding = {
    action: signed.action,
    agentKeyThumbprint: encodeHex(signed.agentKeyThumbprint),
    operatorId,
    purpose: signed.purpose,
    resourceScope: signed.resourceScope,
    resourceType: signed.resourceType,
    snapshotId: signed.snapshotId,
    taskId: signed.taskId,
  };
  const result = await runCapabilityClient(config, {
    operation: "create",
    request: {
      format_version: signed.formatVersion,
      agent_id: signed.agentId,
      agent_public_key: encodeHex(signed.agentPublicKey),
      task_id: signed.taskId,
      purpose: signed.purpose,
      action: signed.action,
      resource_type: signed.resourceType,
      resource_scope: signed.resourceScope,
      requested_expires_at_unix_seconds: signed.requestedExpiresAtUnixSeconds.toString(),
      issuance_nonce: encodeHex(signed.issuanceNonce),
      snapshot_id: signed.snapshotId,
      request_signature: encodeHex(signed.requestSignature),
      request_context: signed.requestContext.map(transportContext),
    },
  });
  if (result.projection.job_id !== expectedJobId) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  registerLiveJob(expectedJobId, binding);
  return retainLiveCapability(result.projection, result.capabilityProtoHex, binding);
}

export async function getCapability(
  jobId: string,
  operatorId: string,
  config: CapabilityProcessConfig,
): Promise<CapabilityJobProjection> {
  if (config.mode === "recorded") {
    return getRecordedJob(jobId, operatorId);
  }
  const binding = liveJobBinding(jobId, operatorId);
  const result = await runCapabilityClient(config, { operation: "get", job_id: jobId });
  if (result.projection.job_id !== jobId) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return retainLiveCapability(result.projection, result.capabilityProtoHex, binding);
}

export async function cancelCapability(
  jobId: string,
  operatorId: string,
  config: CapabilityProcessConfig,
): Promise<CapabilityJobProjection> {
  if (config.mode === "recorded") {
    return cancelRecordedJob(jobId, operatorId);
  }
  const binding = liveJobBinding(jobId, operatorId);
  const result = await runCapabilityClient(config, { operation: "cancel", job_id: jobId });
  if (result.projection.job_id !== jobId) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return retainLiveCapability(result.projection, result.capabilityProtoHex, binding);
}

function sdkContext(request: CreateCapabilityInput): readonly AuthorizationContextEntry[] {
  return request.request_context.map((entry) => {
    switch (entry.value.kind) {
      case "bool":
        return { name: entry.name, value: entry.value };
      case "int":
        return {
          name: entry.name,
          value: { kind: "int" as const, value: BigInt(entry.value.value) },
        };
      case "string":
        return { name: entry.name, value: entry.value };
      default:
        return assertNever(entry.value);
    }
  });
}

function assertNever(_value: never): never {
  throw new CapabilityFlowError("INVALID_CAPABILITY_REQUEST");
}

function transportContext(entry: AuthorizationContextEntry) {
  return {
    name: entry.name,
    value: {
      kind: entry.value.kind,
      value: entry.value.kind === "int" ? entry.value.value.toString() : entry.value.value,
    },
  };
}
