import timeline from "../../fixtures/m4-job-timeline.json";

import type {
  CapabilityJobProjection,
  CapabilityJobState,
  CapabilityMetricsProjection,
} from "./types.ts";
import { CapabilityFlowError } from "./types.ts";

interface RecordedJob {
  canceled: boolean;
  readonly jobId: string;
  readonly operatorId: string;
  readonly submittedAtMilliseconds: number;
}

interface RecordedJobGlobal {
  __razielM5RecordedJobs?: Map<string, RecordedJob>;
}

const processState = globalThis as typeof globalThis & RecordedJobGlobal;
if (!processState.__razielM5RecordedJobs) {
  processState.__razielM5RecordedJobs = new Map<string, RecordedJob>();
}
const jobs = processState.__razielM5RecordedJobs;

export function createRecordedJob(
  commitmentHex: string,
  operatorId: string,
  nowMilliseconds = Date.now(),
): CapabilityJobProjection {
  const jobId = `recorded:${commitmentHex}`;
  let job = jobs.get(jobId);
  if (!job) {
    job = { jobId, operatorId, submittedAtMilliseconds: nowMilliseconds, canceled: false };
    jobs.set(jobId, job);
  }
  if (job.operatorId !== operatorId) {
    throw new CapabilityFlowError("CAPABILITY_CONFLICT");
  }
  return projectRecordedJob(job, nowMilliseconds);
}

export function getRecordedJob(
  jobId: string,
  operatorId: string,
  nowMilliseconds = Date.now(),
): CapabilityJobProjection {
  const job = ownedJob(jobId, operatorId);
  return projectRecordedJob(job, nowMilliseconds);
}

export function cancelRecordedJob(
  jobId: string,
  operatorId: string,
  nowMilliseconds = Date.now(),
): CapabilityJobProjection {
  const job = ownedJob(jobId, operatorId);
  const state = recordedState(job, nowMilliseconds);
  if (!isTerminal(state)) {
    job.canceled = true;
  }
  return projectRecordedJob(job, nowMilliseconds);
}

function projectRecordedJob(job: RecordedJob, nowMilliseconds: number): CapabilityJobProjection {
  const state = recordedState(job, nowMilliseconds);
  const timestamp = (milliseconds: number) => ({
    seconds: String(Math.floor(milliseconds / 1000)),
    nanos: (milliseconds % 1000) * 1_000_000,
  });
  return {
    job_id: job.jobId,
    state,
    submitted_at: timestamp(job.submittedAtMilliseconds),
    updated_at: timestamp(nowMilliseconds),
    public_error_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
    metrics: state === "PROOF_JOB_STATE_SUCCEEDED" ? recordedMetrics() : null,
    capability: null,
    mode: "recorded",
    authorizes_requests: false,
    recorded_source: timeline.source,
  };
}

function recordedState(job: RecordedJob, nowMilliseconds: number): CapabilityJobState {
  if (job.canceled) {
    return "PROOF_JOB_STATE_CANCELED";
  }
  const elapsed = Math.max(0, nowMilliseconds - job.submittedAtMilliseconds);
  let state: CapabilityJobState = "PROOF_JOB_STATE_QUEUED";
  for (const transition of timeline.transitions) {
    if (elapsed >= transition.after_milliseconds) {
      state = transition.state as CapabilityJobState;
    }
  }
  return state;
}

function recordedMetrics(): CapabilityMetricsProjection {
  return { ...timeline.metrics };
}

function ownedJob(jobId: string, operatorId: string): RecordedJob {
  const job = jobs.get(jobId);
  if (!job || job.operatorId !== operatorId) {
    throw new CapabilityFlowError("CAPABILITY_NOT_FOUND");
  }
  return job;
}

function isTerminal(state: CapabilityJobState): boolean {
  return (
    state === "PROOF_JOB_STATE_SUCCEEDED" ||
    state === "PROOF_JOB_STATE_FAILED" ||
    state === "PROOF_JOB_STATE_CANCELED"
  );
}
