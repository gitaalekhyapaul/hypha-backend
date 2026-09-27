import { createHash } from "node:crypto";
import {
  forgetLiveSubmission,
  type LiveSubmissionAttempt,
  liveJobBinding,
  liveSubmissionAttempt,
  registerLiveJob,
  retainLiveCapability,
} from "./live-registry.ts";
import { type CapabilityClientResult, runCapabilityClient } from "./process.ts";
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
  const submission = liveSubmissionAttempt(
    operatorId,
    request.submission_id,
    submissionFingerprint(request),
    () => signLiveSubmission(request, operatorId, nowUnixSeconds),
  );
  const attempt = await submission.attempt;
  if (config.mode === "recorded") {
    return createRecordedJob(attempt.expectedJobId, operatorId);
  }
  try {
    const result = await reconcileLiveSubmission(attempt, config, submission.isNew);
    registerLiveJob(attempt.expectedJobId, attempt.binding);
    return retainLiveCapability(result.projection, result.capabilityProtoHex, attempt.binding);
  } catch (error) {
    if (error instanceof CapabilityFlowError && error.code === "CAPABILITY_BUSY") {
      forgetLiveSubmission(operatorId, request.submission_id);
    }
    throw error;
  }
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

async function signLiveSubmission(
  request: CreateCapabilityInput,
  operatorId: string,
  nowUnixSeconds: number,
): Promise<LiveSubmissionAttempt> {
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
  return {
    expectedJobId,
    binding: {
      action: signed.action,
      agentKeyThumbprint: encodeHex(signed.agentKeyThumbprint),
      operatorId,
      purpose: signed.purpose,
      resourceScope: signed.resourceScope,
      resourceType: signed.resourceType,
      snapshotId: signed.snapshotId,
      taskId: signed.taskId,
    },
    command: {
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
    },
  };
}

export async function reconcileLiveSubmission(
  attempt: LiveSubmissionAttempt,
  config: CapabilityProcessConfig,
  publishFirst: boolean,
  runClient: (
    config: CapabilityProcessConfig,
    command: unknown,
  ) => Promise<CapabilityClientResult> = runCapabilityClient,
): Promise<CapabilityClientResult> {
  const getCommand = { operation: "get", job_id: attempt.expectedJobId };
  const commands = publishFirst
    ? [attempt.command, getCommand, attempt.command, getCommand]
    : [getCommand, attempt.command, getCommand];
  for (const command of commands) {
    try {
      const result = await runClient(config, command);
      if (result.projection.job_id !== attempt.expectedJobId) {
        throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
      }
      return result;
    } catch (error) {
      const retryableNotFound =
        error instanceof CapabilityFlowError &&
        error.code === "CAPABILITY_NOT_FOUND" &&
        command === getCommand;
      if (
        !(error instanceof CapabilityFlowError) ||
        (error.code !== "CAPABILITY_UNAVAILABLE" && !retryableNotFound)
      ) {
        throw error;
      }
    }
  }
  throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
}

function submissionFingerprint(request: CreateCapabilityInput): string {
  const { submission_id: _submissionId, ...boundRequest } = request;
  return createHash("sha256").update(JSON.stringify(boundRequest)).digest("hex");
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
