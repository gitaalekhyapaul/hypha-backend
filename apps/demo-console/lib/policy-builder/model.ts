import type { PreparePolicyCandidateRequest } from "../policy-candidate/types.ts";
import type {
  CaveatSelection,
  DirectRelation,
  PolicyDraftV1,
  PolicyRule,
} from "../policy-draft/types.ts";
import { validatePolicyDraft } from "../policy-draft/validation.ts";

export type PolicyBuilderMode = "direct" | "team" | "alternative" | "dual_control" | "caveat";

export type PolicyLeafSelection = DirectRelation | "team_viewer" | "organization_audit";

export interface PolicyBuilderState {
  readonly policyName: string;
  readonly policyVersion: string;
  readonly resourceType: string;
  readonly expectedApprovalEpoch: string;
  readonly validForSeconds: string;
  readonly mode: PolicyBuilderMode;
  readonly directRelation: DirectRelation;
  readonly alternativeLeft: PolicyLeafSelection;
  readonly alternativeRight: PolicyLeafSelection;
  readonly dualLeft: DirectRelation;
  readonly dualRight: DirectRelation;
  readonly caveatRelation: DirectRelation;
  readonly caveatTemplate: CaveatSelection["template"];
  readonly taskId: string;
  readonly purpose: string;
  readonly resourceScope: string;
}

export const POLICY_MODE_OPTIONS = [
  {
    value: "direct",
    label: "Direct assignment",
    description: "Grant query access through one direct agent relation.",
  },
  {
    value: "team",
    label: "Team membership",
    description: "Grant query access through membership of a related team.",
  },
  {
    value: "alternative",
    label: "Alternative paths",
    description: "Accept either of two direct, team, or delegated paths.",
  },
  {
    value: "dual_control",
    label: "Dual control",
    description: "Require the same agent to satisfy two direct relations.",
  },
  {
    value: "caveat",
    label: "Caveated access",
    description: "Attach one supported CEL caveat to a direct relation.",
  },
] as const satisfies readonly {
  readonly value: PolicyBuilderMode;
  readonly label: string;
  readonly description: string;
}[];

export const DIRECT_RELATION_OPTIONS = ["viewer", "approver", "auditor", "owner"] as const;

export const POLICY_LEAF_OPTIONS = [
  { value: "viewer", label: "Direct viewer" },
  { value: "approver", label: "Direct approver" },
  { value: "auditor", label: "Direct auditor" },
  { value: "owner", label: "Direct owner" },
  { value: "team_viewer", label: "Team membership" },
  { value: "organization_audit", label: "Organization audit delegation" },
] as const satisfies readonly {
  readonly value: PolicyLeafSelection;
  readonly label: string;
}[];

export function defaultPolicyBuilderState(): PolicyBuilderState {
  return {
    policyName: "reconciliation-query",
    policyVersion: "1",
    resourceType: "dataset",
    expectedApprovalEpoch: "1",
    validForSeconds: "86400",
    mode: "direct",
    directRelation: "viewer",
    alternativeLeft: "viewer",
    alternativeRight: "team_viewer",
    dualLeft: "approver",
    dualRight: "auditor",
    caveatRelation: "viewer",
    caveatTemplate: "reconciliation_scope",
    taskId: "reconcile-batch-001",
    purpose: "payment-reconciliation",
    resourceScope: "dataset:synthetic-reconciliation",
  };
}

export function buildPolicyCandidateRequest(
  state: PolicyBuilderState,
  candidateId: string,
): PreparePolicyCandidateRequest {
  const draft = validatePolicyDraft({
    formatVersion: 1,
    candidateId,
    policyName: state.policyName.trim(),
    policyVersion: positiveInteger(state.policyVersion, "Policy version"),
    resourceType: state.resourceType.trim(),
    permission: "query",
    rule: buildRule(state),
  }) as PolicyDraftV1;

  return {
    draft,
    expectedApprovalEpoch: positiveInteger(state.expectedApprovalEpoch, "Approval epoch"),
    validForSeconds: boundedInteger(
      state.validForSeconds,
      15 * 60,
      31 * 24 * 60 * 60,
      "Candidate lifetime",
    ),
  };
}

export function buildRule(state: PolicyBuilderState): PolicyRule {
  switch (state.mode) {
    case "direct":
      return { kind: "direct", relation: state.directRelation };
    case "team":
      return { kind: "subject_set", relation: "team_viewer" };
    case "alternative":
      return {
        kind: "any",
        children: [leafRule(state.alternativeLeft), leafRule(state.alternativeRight)],
      };
    case "dual_control":
      return {
        kind: "all",
        children: [
          { kind: "direct", relation: state.dualLeft },
          { kind: "direct", relation: state.dualRight },
        ],
      };
    case "caveat":
      return {
        kind: "direct",
        relation: state.caveatRelation,
        caveat: caveatSelection(state),
      };
  }
}

function leafRule(selection: PolicyLeafSelection): PolicyRule {
  if (selection === "team_viewer") {
    return { kind: "subject_set", relation: "team_viewer" };
  }
  if (selection === "organization_audit") {
    return { kind: "arrow", relation: "organization", targetPermission: "audit" };
  }
  return { kind: "direct", relation: selection };
}

function caveatSelection(state: PolicyBuilderState): CaveatSelection {
  if (
    state.caveatTemplate === "task_purpose_scope" ||
    state.caveatTemplate === "reconciliation_scope"
  ) {
    return {
      template: state.caveatTemplate,
      taskId: state.taskId.trim(),
      purpose: state.purpose.trim(),
      resourceScope: state.resourceScope.trim(),
    };
  }
  return { template: state.caveatTemplate };
}

function positiveInteger(input: string, label: string): number {
  return boundedInteger(input, 1, Number.MAX_SAFE_INTEGER, label);
}

function boundedInteger(input: string, minimum: number, maximum: number, label: string): number {
  if (!/^[0-9]+$/.test(input)) {
    throw new Error(`${label} must be a whole number`);
  }
  const value = Number(input);
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new Error(`${label} must be between ${minimum} and ${maximum}`);
  }
  return value;
}
