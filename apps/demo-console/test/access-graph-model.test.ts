import { describe, expect, it } from "vitest";

import {
  buildAccessGraphRelationships,
  defaultAccessGraphForm,
  normalizeAccessGraphDatasetId,
  policyBoundResourceScope,
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

  it("maps alternative paths to the dataset selected by Access Graph", () => {
    const form = { ...defaultAccessGraphForm(), datasetId: " august-close " };
    const result = buildAccessGraphRelationships(
      {
        ...defaultPolicyBuilderState(),
        mode: "alternative",
        alternativeLeft: "organization_audit",
        alternativeRight: "team_viewer",
      },
      form,
      AGENT_SUBJECT_ID,
    );

    expect(result).toHaveLength(4);
    expect(
      result
        .filter((relationship) => relationship.resource_type === "dataset")
        .map((relationship) => relationship.resource_id),
    ).toEqual(["august-close", "august-close"]);
    expect(normalizeAccessGraphDatasetId(form.datasetId)).toBe("august-close");
  });

  it("locks and validates a scope-bound caveat against the dataset", () => {
    const builder: PolicyBuilderState = {
      ...defaultPolicyBuilderState(),
      mode: "caveat",
      caveatTemplate: "task_purpose_scope",
      resourceScope: "approved-close",
    };

    expect(policyBoundResourceScope(builder)).toBe("approved-close");
    expect(defaultAccessGraphForm(policyBoundResourceScope(builder) ?? undefined).datasetId).toBe(
      "approved-close",
    );
    expect(() =>
      buildAccessGraphRelationships(
        builder,
        { ...defaultAccessGraphForm(), datasetId: "different-close" },
        AGENT_SUBJECT_ID,
      ),
    ).toThrow("Dataset ID must match the policy-bound resource scope");
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
