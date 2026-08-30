import { describe, expect, it } from "vitest";

import {
  buildAccessGraphRelationships,
  defaultAccessGraphForm,
} from "../lib/access-graph/model.ts";
import { defaultPolicyBuilderState, type PolicyBuilderState } from "../lib/policy-builder/index.ts";

describe("M5 access graph model", () => {
  it("maps a direct policy block to one agent relationship", () => {
    expect(relationships({ mode: "direct", directRelation: "viewer" })).toEqual([
      {
        resource_type: "dataset",
        resource_id: "reconciliation-demo",
        relation: "viewer",
        subject_type: "agent",
        subject_id: AGENT_SUBJECT_ID,
      },
    ]);
  });

  it("maps team access to the resource and membership relationships", () => {
    expect(relationships({ mode: "team" })).toEqual([
      {
        resource_type: "dataset",
        resource_id: "reconciliation-demo",
        relation: "team_viewer",
        subject_type: "team",
        subject_id: "finance-operations",
        subject_relation: "member",
      },
      {
        resource_type: "team",
        resource_id: "finance-operations",
        relation: "member",
        subject_type: "agent",
        subject_id: AGENT_SUBJECT_ID,
      },
    ]);
  });

  it("stores only relationship-scoped values for caveated access", () => {
    expect(relationships({ mode: "caveat", caveatTemplate: "amount_limit" })).toEqual([
      {
        resource_type: "dataset",
        resource_id: "reconciliation-demo",
        relation: "viewer",
        subject_type: "agent",
        subject_id: AGENT_SUBJECT_ID,
        caveat: {
          name: "m5_amount_limit",
          context: [
            {
              name: "approved_amount",
              value: { type: "int", value: 50_000 },
            },
          ],
        },
      },
    ]);
  });

  it("uses the finalized demo dataset as the default capability scope", () => {
    const builder = defaultPolicyBuilderState();
    const form = defaultAccessGraphForm();

    expect(form.datasetId).toBe("reconciliation-demo");
    expect(builder.resourceScope).toBe(form.datasetId);
  });
});

function relationships(overrides: Partial<PolicyBuilderState>) {
  return buildAccessGraphRelationships(
    { ...defaultPolicyBuilderState(), ...overrides },
    defaultAccessGraphForm(),
    AGENT_SUBJECT_ID,
  );
}

const AGENT_SUBJECT_ID = "71".repeat(32);
