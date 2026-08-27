import { describe, expect, it } from "vitest";

import {
  buildPolicyCandidateRequest,
  defaultPolicyBuilderState,
  type PolicyBuilderState,
} from "../lib/policy-builder/index.ts";

describe("M5 visual policy builder", () => {
  it("maps direct assignment to one direct relation", () => {
    expect(request({ mode: "direct", directRelation: "owner" }).draft.rule).toEqual({
      kind: "direct",
      relation: "owner",
    });
  });

  it("maps team membership to the supported subject set", () => {
    expect(request({ mode: "team" }).draft.rule).toEqual({
      kind: "subject_set",
      relation: "team_viewer",
    });
  });

  it("maps alternative paths to ANY and retains organization delegation", () => {
    expect(
      request({
        mode: "alternative",
        alternativeLeft: "organization_audit",
        alternativeRight: "viewer",
      }).draft.rule,
    ).toEqual({
      kind: "any",
      children: [
        { kind: "arrow", relation: "organization", targetPermission: "audit" },
        { kind: "direct", relation: "viewer" },
      ],
    });
  });

  it("maps dual control to ALL of two distinct direct relations", () => {
    expect(
      request({ mode: "dual_control", dualLeft: "approver", dualRight: "auditor" }).draft.rule,
    ).toEqual({
      kind: "all",
      children: [
        { kind: "direct", relation: "approver" },
        { kind: "direct", relation: "auditor" },
      ],
    });
  });

  it("maps claim-bound caveats without accepting runtime caveat values", () => {
    expect(
      request({
        mode: "caveat",
        caveatRelation: "viewer",
        caveatTemplate: "reconciliation_scope",
        taskId: "task-007",
        purpose: "payment-reconciliation",
        resourceScope: "dataset:august",
      }).draft.rule,
    ).toEqual({
      kind: "direct",
      relation: "viewer",
      caveat: {
        template: "reconciliation_scope",
        taskId: "task-007",
        purpose: "payment-reconciliation",
        resourceScope: "dataset:august",
      },
    });
  });

  it("rejects duplicate alternative and dual-control leaves", () => {
    expect(() =>
      request({ mode: "alternative", alternativeLeft: "viewer", alternativeRight: "viewer" }),
    ).toThrow("duplicate rule leaf is unsupported");
    expect(() =>
      request({ mode: "dual_control", dualLeft: "approver", dualRight: "approver" }),
    ).toThrow("duplicate rule leaf is unsupported");
  });
});

function request(overrides: Partial<PolicyBuilderState>) {
  return buildPolicyCandidateRequest(
    { ...defaultPolicyBuilderState(), ...overrides },
    "candidate-policy-builder-test",
  );
}
