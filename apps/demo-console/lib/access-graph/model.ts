import { buildRule, type PolicyBuilderState } from "../policy-builder/index.ts";
import { caveatCatalogEntry, type PolicyRule } from "../policy-draft/index.ts";
import type {
  AccessGraphFormState,
  AccessGraphRelationship,
  AccessGraphStoredCaveat,
  AccessGraphStoredContext,
} from "./types.ts";

export function defaultAccessGraphForm(datasetId = "reconciliation-demo"): AccessGraphFormState {
  return {
    agentId: "agent-reconciliation-01",
    datasetId,
    teamId: "finance-operations",
    organizationId: "nus-fintech-lab",
    validForSeconds: "7200",
    approvedAmount: "50000",
    isApproved: true,
    isSuspended: false,
    isOverride: false,
  };
}

export function buildAccessGraphRelationships(
  builder: PolicyBuilderState,
  form: AccessGraphFormState,
  agentSubjectId: string,
): readonly AccessGraphRelationship[] {
  const resourceType = identifier(builder.resourceType, "Resource type");
  const resourceId = normalizeAccessGraphDatasetId(form.datasetId);
  const agentId = digestIdentifier(agentSubjectId, "Agent key subject");
  const rule = buildRule(builder);
  const boundResourceScope = resourceScopeBinding(rule);
  if (boundResourceScope !== null && resourceId !== boundResourceScope) {
    throw new Error("Dataset ID must match the policy-bound resource scope");
  }
  const relationships: AccessGraphRelationship[] = [];
  visitRule(rule, (leaf) => {
    switch (leaf.kind) {
      case "direct":
        relationships.push({
          resource_type: resourceType,
          resource_id: resourceId,
          relation: leaf.relation,
          subject_type: "agent",
          subject_id: agentId,
          ...(leaf.caveat ? { caveat: storedCaveat(leaf.caveat, form) } : {}),
        });
        break;
      case "subject_set": {
        const teamId = identifier(form.teamId, "Team ID");
        relationships.push(
          {
            resource_type: resourceType,
            resource_id: resourceId,
            relation: leaf.relation,
            subject_type: "team",
            subject_id: teamId,
            subject_relation: "member",
          },
          {
            resource_type: "team",
            resource_id: teamId,
            relation: "member",
            subject_type: "agent",
            subject_id: agentId,
          },
        );
        break;
      }
      case "arrow": {
        const organizationId = identifier(form.organizationId, "Organization ID");
        relationships.push(
          {
            resource_type: resourceType,
            resource_id: resourceId,
            relation: leaf.relation,
            subject_type: "organization",
            subject_id: organizationId,
          },
          {
            resource_type: "organization",
            resource_id: organizationId,
            relation: "auditor",
            subject_type: "agent",
            subject_id: agentId,
          },
        );
        break;
      }
    }
  });
  const deduplicated = new Map(
    relationships.map((relationship) => [identity(relationship), relationship] as const),
  );
  return [...deduplicated.values()].sort((left, right) =>
    identity(left).localeCompare(identity(right)),
  );
}

export function normalizeAccessGraphDatasetId(input: string): string {
  return identifier(input, "Dataset ID");
}

export function policyBoundResourceScope(builder: PolicyBuilderState): string | null {
  return resourceScopeBinding(buildRule(builder));
}

export function graphValiditySeconds(input: string): number {
  if (!/^[0-9]+$/.test(input)) {
    throw new Error("Snapshot lifetime must be a whole number");
  }
  const value = Number(input);
  if (!Number.isSafeInteger(value) || value < 900 || value > 86_400) {
    throw new Error("Snapshot lifetime must be between 900 and 86400 seconds");
  }
  return value;
}

function storedCaveat(
  selection: Extract<PolicyRule, { readonly kind: "direct" }>["caveat"],
  form: AccessGraphFormState,
): AccessGraphStoredCaveat {
  if (!selection) {
    throw new Error("Caveat selection is missing");
  }
  const catalog = caveatCatalogEntry(selection);
  const context: AccessGraphStoredContext[] = catalog.parameters
    .filter((parameter) => parameter.source === "RELATIONSHIP_CONTEXT")
    .map((parameter): AccessGraphStoredContext => {
      switch (parameter.name) {
        case "approved_amount":
          return {
            name: parameter.name,
            value: { type: "int", value: positiveInteger(form.approvedAmount, "Approved amount") },
          };
        case "is_approved":
          return { name: parameter.name, value: { type: "bool", value: form.isApproved } };
        case "is_override":
          return { name: parameter.name, value: { type: "bool", value: form.isOverride } };
        case "is_suspended":
          return { name: parameter.name, value: { type: "bool", value: form.isSuspended } };
        default:
          throw new Error(`Unsupported relationship context ${parameter.name}`);
      }
    })
    .sort((left, right) => left.name.localeCompare(right.name));
  return { name: catalog.schemaName, context };
}

function visitRule(
  rule: PolicyRule,
  visitor: (rule: Exclude<PolicyRule, { readonly kind: "all" | "any" }>) => void,
): void {
  if (rule.kind === "all" || rule.kind === "any") {
    for (const child of rule.children) {
      visitRule(child, visitor);
    }
    return;
  }
  visitor(rule);
}

function resourceScopeBinding(rule: PolicyRule): string | null {
  if (rule.kind !== "direct" || !rule.caveat || !("resourceScope" in rule.caveat)) {
    return null;
  }
  return identifier(rule.caveat.resourceScope, "Policy resource scope");
}

function identifier(input: string, label: string): string {
  const value = input.trim();
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(value)) {
    throw new Error(`${label} contains unsupported characters`);
  }
  return value;
}

function positiveInteger(input: string, label: string): number {
  if (!/^[0-9]+$/.test(input)) {
    throw new Error(`${label} must be a whole number`);
  }
  const value = Number(input);
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} is outside the supported range`);
  }
  return value;
}

function digestIdentifier(input: string, label: string): string {
  const value = input.trim();
  if (!/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest`);
  }
  return value;
}

function identity(relationship: AccessGraphRelationship): string {
  return [
    relationship.resource_type,
    relationship.resource_id,
    relationship.relation,
    relationship.subject_type,
    relationship.subject_id,
    relationship.subject_relation ?? "",
  ].join("\u0000");
}
