import { describe, expect, it } from "vitest";
import { reconcileLiveSubmission } from "../lib/agent-capability/lifecycle.ts";
import {
  type LiveSubmissionAttempt,
  liveSubmissionAttempt,
} from "../lib/agent-capability/live-registry.ts";
import type {
  CapabilityJobProjection,
  CapabilityProcessConfig,
} from "../lib/agent-capability/types.ts";
import { CapabilityFlowError } from "../lib/agent-capability/types.ts";

const CONFIG: CapabilityProcessConfig = {
  mode: "live",
  executable: "/test/client",
  endpoint: "http://razield:50051",
};

describe("M5 capability submission lifecycle", () => {
  it("reconciles an unavailable create through the expected job ID", async () => {
    const commands: unknown[] = [];
    const result = await reconcileLiveSubmission(
      attempt(),
      CONFIG,
      true,
      async (_config, command) => {
        commands.push(command);
        if (commands.length === 1) {
          throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
        }
        return { capabilityProtoHex: null, projection: queuedJob() };
      },
    );

    expect(result.projection.job_id).toBe(JOB_ID);
    expect(commands).toEqual([
      { operation: "create", request: { issuance_nonce: "original" } },
      { operation: "get", job_id: JOB_ID },
    ]);
  });

  it("checks the retained job before resending an uncertain retry", async () => {
    const commands: unknown[] = [];
    await reconcileLiveSubmission(attempt(), CONFIG, false, async (_config, command) => {
      commands.push(command);
      return { capabilityProtoHex: null, projection: queuedJob() };
    });

    expect(commands).toEqual([{ operation: "get", job_id: JOB_ID }]);
  });

  it("resends the exact signed request after reconciliation finds no job", async () => {
    const commands: unknown[] = [];
    const result = await reconcileLiveSubmission(
      attempt(),
      CONFIG,
      false,
      async (_config, command) => {
        commands.push(command);
        if (commands.length === 1) {
          throw new CapabilityFlowError("CAPABILITY_NOT_FOUND");
        }
        return { capabilityProtoHex: null, projection: queuedJob() };
      },
    );

    expect(result.projection.job_id).toBe(JOB_ID);
    expect(commands).toEqual([
      { operation: "get", job_id: JOB_ID },
      { operation: "create", request: { issuance_nonce: "original" } },
    ]);
  });

  it("shares one signed attempt and rejects changed input for the same submission", async () => {
    let creations = 0;
    const create = async () => {
      creations += 1;
      return attempt();
    };
    const first = liveSubmissionAttempt("operator-test", SUBMISSION_ID, "fingerprint-a", create);
    const concurrent = liveSubmissionAttempt(
      "operator-test",
      SUBMISSION_ID,
      "fingerprint-a",
      create,
    );

    await expect(first.attempt).resolves.toEqual(await concurrent.attempt);
    expect(creations).toBe(1);
    expect(() =>
      liveSubmissionAttempt("operator-test", SUBMISSION_ID, "fingerprint-b", create),
    ).toThrowError(new CapabilityFlowError("CAPABILITY_CONFLICT"));
  });
});

function attempt(): LiveSubmissionAttempt {
  return {
    expectedJobId: JOB_ID,
    binding: {
      action: "query",
      agentKeyThumbprint: "22".repeat(32),
      operatorId: "operator-test",
      purpose: "reconcile-payments",
      resourceScope: "august-close",
      resourceType: "dataset",
      snapshotId: `snapshot-v1:${"33".repeat(32)}`,
      taskId: "payment-reconciliation",
    },
    command: { operation: "create", request: { issuance_nonce: "original" } },
  };
}

function queuedJob(): CapabilityJobProjection {
  return {
    job_id: JOB_ID,
    state: "PROOF_JOB_STATE_QUEUED",
    submitted_at: { seconds: "1788182441", nanos: 0 },
    updated_at: { seconds: "1788182441", nanos: 0 },
    public_error_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
    metrics: null,
    capability: null,
    mode: "live",
    authorizes_requests: false,
    recorded_source: null,
  };
}

const JOB_ID = "11".repeat(32);
const SUBMISSION_ID = "123e4567-e89b-42d3-a456-426614174010";
