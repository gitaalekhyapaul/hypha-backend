import type { CapabilityJobProjection } from "./types.ts";
import { CapabilityFlowError } from "./types.ts";

export interface LiveCapabilityBinding {
  readonly action: string;
  readonly agentKeyThumbprint: string;
  readonly operatorId: string;
  readonly purpose: string;
  readonly resourceScope: string;
  readonly resourceType: string;
  readonly snapshotId: string;
  readonly taskId: string;
}

interface LiveJobRecord {
  capabilityProtoHex: string | null;
  readonly binding: LiveCapabilityBinding;
}

export interface LiveSubmissionAttempt {
  readonly binding: LiveCapabilityBinding;
  readonly command: unknown;
  readonly expectedJobId: string;
}

interface LiveSubmissionRecord {
  readonly attempt: Promise<LiveSubmissionAttempt>;
  readonly createdAtMilliseconds: number;
  readonly fingerprint: string;
}

interface LiveJobGlobal {
  __razielM5LiveJobs?: Map<string, LiveJobRecord>;
  __razielM5LiveSubmissions?: Map<string, LiveSubmissionRecord>;
}

const processState = globalThis as typeof globalThis & LiveJobGlobal;
if (!processState.__razielM5LiveJobs) {
  processState.__razielM5LiveJobs = new Map<string, LiveJobRecord>();
}
const jobs = processState.__razielM5LiveJobs;
if (!processState.__razielM5LiveSubmissions) {
  processState.__razielM5LiveSubmissions = new Map<string, LiveSubmissionRecord>();
}
const submissions = processState.__razielM5LiveSubmissions;
const SUBMISSION_RETENTION_MILLISECONDS = 2 * 60 * 60 * 1_000;
const MAX_RETAINED_SUBMISSIONS = 64;

export function liveSubmissionAttempt(
  operatorId: string,
  submissionId: string,
  fingerprint: string,
  create: () => Promise<LiveSubmissionAttempt>,
  nowMilliseconds = Date.now(),
): { readonly attempt: Promise<LiveSubmissionAttempt>; readonly isNew: boolean } {
  pruneSubmissions(nowMilliseconds);
  const key = submissionKey(operatorId, submissionId);
  const current = submissions.get(key);
  if (current) {
    if (current.fingerprint !== fingerprint) {
      throw new CapabilityFlowError("CAPABILITY_CONFLICT");
    }
    return { attempt: current.attempt, isNew: false };
  }
  if (submissions.size >= MAX_RETAINED_SUBMISSIONS) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  let attempt: Promise<LiveSubmissionAttempt>;
  attempt = create().catch((error: unknown) => {
    if (submissions.get(key)?.attempt === attempt) {
      submissions.delete(key);
    }
    throw error;
  });
  submissions.set(key, {
    attempt,
    createdAtMilliseconds: nowMilliseconds,
    fingerprint,
  });
  return { attempt, isNew: true };
}

export function forgetLiveSubmission(operatorId: string, submissionId: string): void {
  submissions.delete(submissionKey(operatorId, submissionId));
}

export function registerLiveJob(jobId: string, binding: LiveCapabilityBinding): void {
  const current = jobs.get(jobId);
  if (current) {
    if (current.binding.operatorId !== binding.operatorId) {
      throw new CapabilityFlowError("CAPABILITY_CONFLICT");
    }
    return;
  }
  jobs.set(jobId, { binding, capabilityProtoHex: null });
}

export function liveJobBinding(jobId: string, operatorId: string): LiveCapabilityBinding {
  const record = jobs.get(jobId);
  if (!record || record.binding.operatorId !== operatorId) {
    throw new CapabilityFlowError("CAPABILITY_NOT_FOUND");
  }
  return record.binding;
}

export function retainLiveCapability(
  job: CapabilityJobProjection,
  capabilityProtoHex: string | null,
  binding: LiveCapabilityBinding,
): CapabilityJobProjection {
  if (!job.capability) {
    if (capabilityProtoHex !== null) {
      throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
    }
    return job;
  }
  const capability = job.capability;
  if (
    capabilityProtoHex === null ||
    capability.capability_id !== job.job_id ||
    capability.agent_key_thumbprint !== binding.agentKeyThumbprint ||
    capability.task_id !== binding.taskId ||
    capability.purpose !== binding.purpose ||
    capability.action !== binding.action ||
    capability.resource_type !== binding.resourceType ||
    capability.resource_scope !== binding.resourceScope ||
    capability.snapshot_id !== binding.snapshotId
  ) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  const record = jobs.get(job.job_id);
  if (!record) {
    throw new CapabilityFlowError("CAPABILITY_NOT_FOUND");
  }
  const authorizesRequests = job.mode === "live" && job.state === "PROOF_JOB_STATE_SUCCEEDED";
  record.capabilityProtoHex = authorizesRequests ? capabilityProtoHex : null;
  return { ...job, authorizes_requests: authorizesRequests };
}

export function retainedCapabilityProtoHex(jobId: string, operatorId: string): string {
  const record = jobs.get(jobId);
  if (!record || record.binding.operatorId !== operatorId || !record.capabilityProtoHex) {
    throw new CapabilityFlowError("CAPABILITY_NOT_FOUND");
  }
  return record.capabilityProtoHex;
}

function pruneSubmissions(nowMilliseconds: number): void {
  for (const [key, record] of submissions) {
    if (nowMilliseconds - record.createdAtMilliseconds >= SUBMISSION_RETENTION_MILLISECONDS) {
      submissions.delete(key);
    }
  }
}

function submissionKey(operatorId: string, submissionId: string): string {
  return `${operatorId.length}:${operatorId}${submissionId}`;
}
