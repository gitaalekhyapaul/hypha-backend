import type { CaveatCatalogEntry, CaveatParameterDefinition, CaveatSelection } from "./types.ts";

export const M5_COMPILER_VERSION = "1.54.0";

export const M5_COMPILER_LIMITS = {
  max_expression_nodes: 64,
  max_caveat_nodes: 64,
  max_path_depth: 4,
  max_relationship_witnesses: 32,
  max_merkle_depth: 32,
  max_string_bytes: 256,
  max_context_entries: 16,
  max_total_private_input_bytes: 4096,
} as const;

const TASK_PURPOSE_SCOPE_PARAMETERS = [
  parameter("task_id", "string", "CAPABILITY_CLAIM"),
  parameter("purpose", "string", "CAPABILITY_CLAIM"),
  parameter("resource_scope", "string", "CAPABILITY_CLAIM"),
] as const;

const AMOUNT_LIMIT_PARAMETERS = [
  parameter("approved_amount", "int", "RELATIONSHIP_CONTEXT"),
  parameter("requested_amount", "int", "REQUEST_CONTEXT"),
] as const;

const APPROVAL_GATE_PARAMETERS = [
  parameter("is_approved", "bool", "RELATIONSHIP_CONTEXT"),
  parameter("is_suspended", "bool", "RELATIONSHIP_CONTEXT"),
] as const;

const AMOUNT_OR_OVERRIDE_PARAMETERS = [
  parameter("approved_amount", "int", "RELATIONSHIP_CONTEXT"),
  parameter("is_override", "bool", "RELATIONSHIP_CONTEXT"),
  parameter("requested_amount", "int", "REQUEST_CONTEXT"),
] as const;

const RECONCILIATION_SCOPE_PARAMETERS = [
  ...TASK_PURPOSE_SCOPE_PARAMETERS,
  ...AMOUNT_LIMIT_PARAMETERS,
  parameter("is_approved", "bool", "RELATIONSHIP_CONTEXT"),
] as const;

export const CAVEAT_CATALOG: readonly CaveatCatalogEntry[] = [
  {
    template: "amount_limit",
    schemaName: "m5_amount_limit",
    label: "Amount limit",
    description: "Require the requested amount to stay within a committed approval limit.",
    parameters: AMOUNT_LIMIT_PARAMETERS,
  },
  {
    template: "amount_or_override",
    schemaName: "m5_amount_or_override",
    label: "Amount limit or override",
    description: "Allow an owner-approved override or enforce the committed amount limit.",
    parameters: AMOUNT_OR_OVERRIDE_PARAMETERS,
  },
  {
    template: "approval_gate",
    schemaName: "m5_approval_gate",
    label: "Approval gate",
    description: "Require committed approval and reject a suspended relationship.",
    parameters: APPROVAL_GATE_PARAMETERS,
  },
  {
    template: "reconciliation_scope",
    schemaName: "m5_reconciliation_scope",
    label: "Reconciliation scope",
    description: "Bind task, purpose, scope, amount, and approval in one caveat.",
    parameters: RECONCILIATION_SCOPE_PARAMETERS,
  },
  {
    template: "task_purpose_scope",
    schemaName: "m5_task_purpose_scope",
    label: "Task, purpose, and scope",
    description: "Match the capability task, purpose, and resource scope.",
    parameters: TASK_PURPOSE_SCOPE_PARAMETERS,
  },
] as const;

const CAVEATS_BY_TEMPLATE = new Map(
  CAVEAT_CATALOG.map((entry) => [entry.template, entry] as const),
);

export function caveatCatalogEntry(selection: CaveatSelection): CaveatCatalogEntry {
  const entry = CAVEATS_BY_TEMPLATE.get(selection.template);
  if (!entry) {
    throw new Error("validated caveat template is missing from the server catalog");
  }
  return entry;
}

export function caveatExpression(selection: CaveatSelection): string {
  switch (selection.template) {
    case "amount_limit":
      return "requested_amount <= approved_amount";
    case "amount_or_override":
      return "is_override || requested_amount <= approved_amount";
    case "approval_gate":
      return "is_approved && !is_suspended";
    case "task_purpose_scope":
      return [
        `task_id == ${celString(selection.taskId)}`,
        `purpose == ${celString(selection.purpose)}`,
        `resource_scope == ${celString(selection.resourceScope)}`,
      ].join(" &&\n  ");
    case "reconciliation_scope":
      return [
        `task_id == ${celString(selection.taskId)}`,
        `purpose == ${celString(selection.purpose)}`,
        `resource_scope == ${celString(selection.resourceScope)}`,
        "requested_amount <= approved_amount",
        "is_approved",
      ].join(" &&\n  ");
  }
}

function parameter(
  name: string,
  valueType: CaveatParameterDefinition["valueType"],
  source: CaveatParameterDefinition["source"],
): CaveatParameterDefinition {
  return { name, valueType, source };
}

function celString(value: string): string {
  return JSON.stringify(value);
}
