import {
  caveatCatalogEntry,
  caveatExpression,
  M5_COMPILER_LIMITS,
  M5_COMPILER_VERSION,
} from "./catalog.ts";
import type {
  CaveatSelection,
  PolicyDraftV1,
  PolicyRule,
  PolicySourceInventory,
  RenderedPolicySource,
  RenderedPolicyVersionSource,
} from "./types.ts";
import { validatePolicyDraft, validatePolicyVersionIdentity } from "./validation.ts";

interface SchemaNames {
  readonly team: string;
  readonly organization: string;
  readonly caveat: (selection: CaveatSelection) => string;
}

const LEGACY_NAMES: SchemaNames = {
  team: "team",
  organization: "organization",
  caveat: (selection) => caveatCatalogEntry(selection).schemaName,
};

export function renderPolicySource(input: unknown): RenderedPolicySource {
  const draft = validatePolicyDraft(input);
  return {
    draft,
    compilerVersion: M5_COMPILER_VERSION,
    schema: renderSchema(draft, LEGACY_NAMES).schema,
    compilerConfig: renderCompilerConfig(draft, LEGACY_NAMES),
    ruleSummary: summarizeRule(draft.rule),
  };
}

export function renderPolicyVersionSource(
  input: unknown,
  identityInput: unknown,
): RenderedPolicyVersionSource {
  const identity = validatePolicyVersionIdentity(identityInput);
  const signedPolicyName = `family:${identity.familyId}:v${identity.version}`;
  // The allocated identity owns compiler and approval metadata, not display aliases.
  const draft: PolicyDraftV1 = {
    ...validatePolicyDraft(input),
    policyName: signedPolicyName,
    policyVersion: identity.version,
    resourceType: `dataset_${identity.versionId}`,
  };
  const names: SchemaNames = {
    team: `team_${identity.versionId}`,
    organization: `organization_${identity.versionId}`,
    // Draft validation permits one caveat. Its stable index is zero.
    caveat: () => `caveat_${identity.versionId}_0`,
  };
  const { schema, inventory } = renderSchema(draft, names);
  return {
    draft,
    identity,
    signedPolicyName,
    inventory,
    compilerVersion: M5_COMPILER_VERSION,
    schema,
    compilerConfig: renderCompilerConfig(draft, names),
    ruleSummary: summarizeRule(draft.rule),
  };
}

export function summarizeRule(rule: PolicyRule): string {
  switch (rule.kind) {
    case "direct":
      return rule.caveat
        ? `${rule.relation} with ${caveatCatalogEntry(rule.caveat).label}`
        : rule.relation;
    case "subject_set":
      return "team membership";
    case "arrow":
      return "organization audit delegation";
    case "any":
      return `ANY(${rule.children.map(summarizeRule).join(", ")})`;
    case "all":
      return `ALL(${rule.children.map(summarizeRule).join(", ")})`;
  }
}

function renderSchema(
  draft: PolicyDraftV1,
  names: SchemaNames,
): { readonly schema: string; readonly inventory: PolicySourceInventory } {
  const caveat = findCaveat(draft.rule);
  const sections: string[] = [];
  const definitions = ["agent", draft.resourceType];
  const caveats: string[] = [];
  if (caveat) {
    const name = names.caveat(caveat);
    sections.push(renderCaveat(caveat, name));
    caveats.push(name);
  }
  sections.push("definition agent {}");
  if (containsKind(draft.rule, "subject_set")) {
    definitions.push(names.team);
    sections.push([`definition ${names.team} {`, "  relation member: agent", "}"].join("\n"));
  }
  if (containsKind(draft.rule, "arrow")) {
    definitions.push(names.organization);
    sections.push(
      [
        `definition ${names.organization} {`,
        "  relation auditor: agent",
        "  permission audit = auditor",
        "}",
      ].join("\n"),
    );
  }

  const relations = collectRelationDeclarations(draft.rule, names);
  sections.push(
    [
      `definition ${draft.resourceType} {`,
      ...relations.map((relation) => `  ${relation}`),
      "",
      `  permission ${draft.permission} = ${renderRuleExpression(draft.rule)}`,
      "}",
    ].join("\n"),
  );
  return {
    schema: `${sections.join("\n\n")}\n`,
    inventory: { definitions: definitions.sort(), caveats: caveats.sort() },
  };
}

function renderCompilerConfig(draft: PolicyDraftV1, names: SchemaNames): string {
  const caveat = findCaveat(draft.rule);
  const caveats: Record<string, Record<string, string>> = {};
  if (caveat) {
    const entry = caveatCatalogEntry(caveat);
    caveats[names.caveat(caveat)] = Object.fromEntries(
      entry.parameters.map((parameter) => [parameter.name, parameter.source]),
    );
  }
  return `${JSON.stringify(
    {
      config_version: 1,
      limits: M5_COMPILER_LIMITS,
      caveats,
    },
    null,
    2,
  )}\n`;
}

function renderCaveat(selection: CaveatSelection, name: string): string {
  const entry = caveatCatalogEntry(selection);
  const parameters = entry.parameters
    .map((parameter) => `${parameter.name} ${parameter.valueType}`)
    .join(", ");
  return [`caveat ${name}(${parameters}) {`, `  ${caveatExpression(selection)}`, "}"].join("\n");
}

function collectRelationDeclarations(rule: PolicyRule, names: SchemaNames): string[] {
  const declarations = new Map<string, string>();
  visitRules(rule, (leaf) => {
    switch (leaf.kind) {
      case "direct": {
        const caveat = leaf.caveat ? ` with ${names.caveat(leaf.caveat)}` : "";
        declarations.set(leaf.relation, `relation ${leaf.relation}: agent${caveat}`);
        break;
      }
      case "subject_set":
        declarations.set(leaf.relation, `relation ${leaf.relation}: ${names.team}#member`);
        break;
      case "arrow":
        declarations.set(leaf.relation, `relation ${leaf.relation}: ${names.organization}`);
        break;
    }
  });
  return [...declarations.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([, declaration]) => declaration);
}

function renderRuleExpression(rule: PolicyRule): string {
  switch (rule.kind) {
    case "direct":
    case "subject_set":
      return rule.relation;
    case "arrow":
      return `${rule.relation}->${rule.targetPermission}`;
    case "any":
      return `(${rule.children.map(renderRuleExpression).join(" + ")})`;
    case "all":
      return `(${rule.children.map(renderRuleExpression).join(" & ")})`;
  }
}

function findCaveat(rule: PolicyRule): CaveatSelection | undefined {
  if (rule.kind === "direct") {
    return rule.caveat;
  }
  if (rule.kind === "any" || rule.kind === "all") {
    for (const child of rule.children) {
      const caveat = findCaveat(child);
      if (caveat) {
        return caveat;
      }
    }
  }
  return undefined;
}

function containsKind(rule: PolicyRule, kind: "arrow" | "subject_set"): boolean {
  if (rule.kind === kind) {
    return true;
  }
  if (rule.kind === "any" || rule.kind === "all") {
    return rule.children.some((child) => containsKind(child, kind));
  }
  return false;
}

function visitRules(
  rule: PolicyRule,
  visitor: (rule: Exclude<PolicyRule, { readonly kind: "all" | "any" }>) => void,
): void {
  if (rule.kind === "any" || rule.kind === "all") {
    for (const child of rule.children) {
      visitRules(child, visitor);
    }
    return;
  }
  visitor(rule);
}
