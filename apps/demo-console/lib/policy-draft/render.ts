import {
  caveatCatalogEntry,
  caveatExpression,
  M5_COMPILER_LIMITS,
  M5_COMPILER_VERSION,
} from "./catalog.ts";
import type { CaveatSelection, PolicyDraftV1, PolicyRule, RenderedPolicySource } from "./types.ts";
import { validatePolicyDraft } from "./validation.ts";

export function renderPolicySource(input: unknown): RenderedPolicySource {
  const draft = validatePolicyDraft(input);
  return {
    draft,
    compilerVersion: M5_COMPILER_VERSION,
    schema: renderSchema(draft),
    compilerConfig: renderCompilerConfig(draft),
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

function renderSchema(draft: PolicyDraftV1): string {
  const caveat = findCaveat(draft.rule);
  const sections: string[] = [];
  if (caveat) {
    sections.push(renderCaveat(caveat));
  }
  sections.push("definition agent {}");
  if (containsKind(draft.rule, "subject_set")) {
    sections.push(["definition team {", "  relation member: agent", "}"].join("\n"));
  }
  if (containsKind(draft.rule, "arrow")) {
    sections.push(
      [
        "definition organization {",
        "  relation auditor: agent",
        "  permission audit = auditor",
        "}",
      ].join("\n"),
    );
  }

  const relations = collectRelationDeclarations(draft.rule);
  sections.push(
    [
      `definition ${draft.resourceType} {`,
      ...relations.map((relation) => `  ${relation}`),
      "",
      `  permission ${draft.permission} = ${renderRuleExpression(draft.rule)}`,
      "}",
    ].join("\n"),
  );
  return `${sections.join("\n\n")}\n`;
}

function renderCompilerConfig(draft: PolicyDraftV1): string {
  const caveat = findCaveat(draft.rule);
  const caveats: Record<string, Record<string, string>> = {};
  if (caveat) {
    const entry = caveatCatalogEntry(caveat);
    caveats[entry.schemaName] = Object.fromEntries(
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

function renderCaveat(selection: CaveatSelection): string {
  const entry = caveatCatalogEntry(selection);
  const parameters = entry.parameters
    .map((parameter) => `${parameter.name} ${parameter.valueType}`)
    .join(", ");
  return [
    `caveat ${entry.schemaName}(${parameters}) {`,
    `  ${caveatExpression(selection)}`,
    "}",
  ].join("\n");
}

function collectRelationDeclarations(rule: PolicyRule): string[] {
  const declarations = new Map<string, string>();
  visitRules(rule, (leaf) => {
    switch (leaf.kind) {
      case "direct": {
        const caveat = leaf.caveat ? ` with ${caveatCatalogEntry(leaf.caveat).schemaName}` : "";
        declarations.set(leaf.relation, `relation ${leaf.relation}: agent${caveat}`);
        break;
      }
      case "subject_set":
        declarations.set(leaf.relation, `relation ${leaf.relation}: team#member`);
        break;
      case "arrow":
        declarations.set(leaf.relation, `relation ${leaf.relation}: organization`);
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
