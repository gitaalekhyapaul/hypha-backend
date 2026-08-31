import { describe, expect, it } from "vitest";

import { buildCapabilityRequestInput } from "../lib/agent-capability/request.ts";
import type { PublishedAccessSelection } from "../lib/agent-capability/types.ts";
import { defaultPolicyBuilderState } from "../lib/policy-builder/index.ts";

describe("M5 capability request binding", () => {
  it("uses the published dataset as the alternative-path capability scope", () => {
    const request = buildCapabilityRequestInput(
      { ...defaultPolicyBuilderState(), mode: "alternative" },
      publishedAccess("august-close"),
      5_400,
      [],
    );

    expect(request.resource_scope).toBe("august-close");
    expect(request.snapshot_id).toBe(SNAPSHOT_ID);
  });

  it("rejects a stale access selection for a scope-bound caveat", () => {
    expect(() =>
      buildCapabilityRequestInput(
        {
          ...defaultPolicyBuilderState(),
          mode: "caveat",
          caveatTemplate: "reconciliation_scope",
          resourceScope: "approved-close",
        },
        publishedAccess("different-close"),
        5_400,
        [],
      ),
    ).toThrow("Published dataset does not match the policy-bound resource scope");
  });
});

function publishedAccess(datasetId: string): PublishedAccessSelection {
  return {
    agent: {
      key_thumbprint: "71".repeat(32),
      subject_type: "agent",
      subject_id: "71".repeat(32),
      created_at_unix_seconds: "1788180000",
    },
    agentId: "agent-reconciliation-01",
    datasetId,
    snapshotId: SNAPSHOT_ID,
    policyHash: "72".repeat(32),
    relationshipRoot: "73".repeat(32),
    snapshotExpiresAtUnixSeconds: "1788187200",
  };
}

const SNAPSHOT_ID = `snapshot-v1:${"74".repeat(32)}`;
