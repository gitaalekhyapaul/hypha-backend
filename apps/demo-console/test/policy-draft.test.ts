import { describe, expect, it } from "vitest";

import {
  M5_COMPILER_LIMITS,
  PolicyDraftValidationError,
  renderPolicySource,
  validatePolicyDraft,
} from "../lib/policy-draft/index.ts";

describe("M5 policy draft", () => {
  it.each([
    ["direct", { kind: "direct", relation: "viewer" }],
    ["team", { kind: "subject_set", relation: "team_viewer" }],
    [
      "union",
      {
        kind: "any",
        children: [
          { kind: "direct", relation: "viewer" },
          { kind: "direct", relation: "auditor" },
        ],
      },
    ],
    [
      "intersection",
      {
        kind: "all",
        children: [
          { kind: "direct", relation: "viewer" },
          { kind: "direct", relation: "approver" },
        ],
      },
    ],
    ["delegation", { kind: "arrow", relation: "organization", targetPermission: "audit" }],
  ])("renders the accepted %s rule shape", (_name, rule) => {
    const rendered = renderPolicySource(draft(rule));
    expect(rendered.schema).toContain("definition dataset");
    expect(rendered.schema).toContain("permission query =");
    expect(JSON.parse(rendered.compilerConfig)).toEqual({
      config_version: 1,
      limits: M5_COMPILER_LIMITS,
      caveats: {},
    });
  });

  it("renders a stable caveated policy with fixed parameter provenance", () => {
    const input = draft({
      kind: "direct",
      relation: "viewer",
      caveat: {
        template: "reconciliation_scope",
        taskId: "task-august",
        purpose: "payment reconciliation",
        resourceScope: "dataset:synthetic-august",
      },
    });
    const first = renderPolicySource(input);
    const second = renderPolicySource(structuredClone(input));

    expect(second).toEqual(first);
    expect(first.schema).toContain(
      "caveat m5_reconciliation_scope(task_id string, purpose string, resource_scope string, approved_amount int, requested_amount int, is_approved bool)",
    );
    expect(first.schema).toContain('task_id == "task-august"');
    expect(first.schema).toContain("relation viewer: agent with m5_reconciliation_scope");
    expect(JSON.parse(first.compilerConfig).caveats).toEqual({
      m5_reconciliation_scope: {
        task_id: "CAPABILITY_CLAIM",
        purpose: "CAPABILITY_CLAIM",
        resource_scope: "CAPABILITY_CLAIM",
        approved_amount: "RELATIONSHIP_CONTEXT",
        requested_amount: "REQUEST_CONTEXT",
        is_approved: "RELATIONSHIP_CONTEXT",
      },
    });
  });

  it("normalizes commutative child order before rendering", () => {
    const left = renderPolicySource(
      draft({
        kind: "any",
        children: [
          { kind: "direct", relation: "viewer" },
          { kind: "direct", relation: "auditor" },
        ],
      }),
    );
    const right = renderPolicySource(
      draft({
        kind: "any",
        children: [
          { kind: "direct", relation: "auditor" },
          { kind: "direct", relation: "viewer" },
        ],
      }),
    );
    expect(right.schema).toBe(left.schema);
    expect(right.ruleSummary).toBe(left.ruleSummary);
  });

  it.each([
    ["unknown field", { ...draft({ kind: "direct", relation: "viewer" }), rawZed: "x" }],
    ["wildcard", draft({ kind: "direct", relation: "viewer", subject: "agent:*" })],
    ["exclusion", draft({ kind: "exclude", left: "viewer", right: "blocked" })],
    ["recursion", draft({ kind: "recursive", permission: "query" })],
    [
      "duplicate branch",
      draft({
        kind: "any",
        children: [
          { kind: "direct", relation: "viewer" },
          { kind: "direct", relation: "viewer" },
        ],
      }),
    ],
    [
      "two arrows",
      draft({
        kind: "any",
        children: [
          { kind: "arrow", relation: "organization", targetPermission: "audit" },
          { kind: "arrow", relation: "organization", targetPermission: "audit" },
        ],
      }),
    ],
    [
      "arbitrary caveat",
      draft({
        kind: "direct",
        relation: "viewer",
        caveat: { template: "raw_cel", expression: "true" },
      }),
    ],
  ])("rejects the unsupported %s construct", (_name, input) => {
    expect(() => validatePolicyDraft(input)).toThrow(PolicyDraftValidationError);
  });

  it("rejects reserved resource definitions and caller-selected limits", () => {
    expect(() =>
      validatePolicyDraft({
        ...draft({ kind: "direct", relation: "viewer" }),
        resourceType: "team",
      }),
    ).toThrow("reserved");
    expect(() =>
      validatePolicyDraft({
        ...draft({ kind: "direct", relation: "viewer" }),
        limits: { max_expression_nodes: 1 },
      }),
    ).toThrow("unsupported");
  });

  it("rejects invalid identifiers, versions, caveat literals, and deep trees", () => {
    expect(() =>
      validatePolicyDraft({
        ...draft({ kind: "direct", relation: "viewer" }),
        resourceType: "Dataset",
      }),
    ).toThrow("lowercase SpiceDB identifier");
    expect(() =>
      validatePolicyDraft({
        ...draft({ kind: "direct", relation: "viewer" }),
        policyVersion: 0,
      }),
    ).toThrow("between 1");
    expect(() =>
      validatePolicyDraft(
        draft({
          kind: "direct",
          relation: "viewer",
          caveat: {
            template: "task_purpose_scope",
            taskId: "",
            purpose: "reconciliation",
            resourceScope: "dataset:august",
          },
        }),
      ),
    ).toThrow("bounded task");
    expect(() => validatePolicyDraft(draft(deepRule()))).toThrow("depth exceeds");
  });
});

function draft(rule: unknown): Record<string, unknown> {
  return {
    formatVersion: 1,
    candidateId: "candidate-m5-001",
    policyName: "reconciliation-query",
    policyVersion: 1,
    resourceType: "dataset",
    permission: "query",
    rule,
  };
}

function deepRule(): unknown {
  return {
    kind: "any",
    children: [
      { kind: "direct", relation: "viewer" },
      {
        kind: "all",
        children: [
          { kind: "direct", relation: "auditor" },
          {
            kind: "any",
            children: [
              { kind: "direct", relation: "owner" },
              {
                kind: "all",
                children: [
                  { kind: "direct", relation: "approver" },
                  { kind: "subject_set", relation: "team_viewer" },
                ],
              },
            ],
          },
        ],
      },
    ],
  };
}
