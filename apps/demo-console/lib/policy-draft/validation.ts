import {
  type CaveatSelection,
  type DirectRelation,
  POLICY_DRAFT_FORMAT_VERSION,
  type PolicyDraftV1,
  PolicyDraftValidationError,
  type PolicyRule,
} from "./types.ts";

const SCHEMA_IDENTIFIER = /^[a-z][a-z0-9_]{0,62}$/;
const PUBLIC_IDENTIFIER = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const CLAIM_LITERAL = /^[A-Za-z0-9][A-Za-z0-9 ._:/-]{0,127}$/;
const RESERVED_RESOURCE_TYPES = new Set(["agent", "organization", "team"]);
const DIRECT_RELATIONS = new Set<DirectRelation>(["approver", "auditor", "owner", "viewer"]);
const MAX_POLICY_VERSION = 2_147_483_647;
const MAX_RULE_DEPTH = 4;
const MAX_RULE_NODES = 16;
const MAX_COMBINATION_CHILDREN = 4;

interface RuleCounters {
  arrows: number;
  caveats: number;
  nodes: number;
  readonly leaves: Set<string>;
}

export function validatePolicyDraft(input: unknown): PolicyDraftV1 {
  const draft = objectAt(input, "$", [
    "formatVersion",
    "candidateId",
    "policyName",
    "policyVersion",
    "resourceType",
    "permission",
    "rule",
  ]);
  numberEquals(draft.formatVersion, POLICY_DRAFT_FORMAT_VERSION, "$.formatVersion");
  const candidateId = publicIdentifier(draft.candidateId, "$.candidateId");
  const policyName = publicIdentifier(draft.policyName, "$.policyName");
  const policyVersion = boundedInteger(
    draft.policyVersion,
    1,
    MAX_POLICY_VERSION,
    "$.policyVersion",
  );
  const resourceType = schemaIdentifier(draft.resourceType, "$.resourceType");
  if (RESERVED_RESOURCE_TYPES.has(resourceType)) {
    fail("$.resourceType", "resource type conflicts with a reserved policy definition");
  }
  literalEquals(draft.permission, "query", "$.permission");

  const counters: RuleCounters = { arrows: 0, caveats: 0, nodes: 0, leaves: new Set() };
  const rule = ruleAt(draft.rule, "$.rule", 1, counters);
  return {
    formatVersion: POLICY_DRAFT_FORMAT_VERSION,
    candidateId,
    policyName,
    policyVersion,
    resourceType,
    permission: "query",
    rule,
  };
}

function ruleAt(input: unknown, path: string, depth: number, counters: RuleCounters): PolicyRule {
  if (depth > MAX_RULE_DEPTH) {
    fail(path, `rule depth exceeds ${MAX_RULE_DEPTH}`);
  }
  counters.nodes += 1;
  if (counters.nodes > MAX_RULE_NODES) {
    fail(path, `rule node count exceeds ${MAX_RULE_NODES}`);
  }
  const kind = objectKind(input, path);
  switch (kind) {
    case "direct": {
      const rule = objectAt(input, path, ["kind", "relation", "caveat"]);
      const relation = directRelation(rule.relation, `${path}.relation`);
      const caveat =
        rule.caveat === undefined ? undefined : caveatAt(rule.caveat, `${path}.caveat`);
      if (caveat) {
        counters.caveats += 1;
        if (counters.caveats > 1) {
          fail(path, "a policy draft can attach only one caveat");
        }
      }
      uniqueLeaf(counters, `direct:${relation}`, path);
      return { kind: "direct", relation, ...(caveat ? { caveat } : {}) };
    }
    case "subject_set": {
      const rule = objectAt(input, path, ["kind", "relation"]);
      literalEquals(rule.relation, "team_viewer", `${path}.relation`);
      uniqueLeaf(counters, "subject_set:team_viewer", path);
      return { kind: "subject_set", relation: "team_viewer" };
    }
    case "arrow": {
      const rule = objectAt(input, path, ["kind", "relation", "targetPermission"]);
      literalEquals(rule.relation, "organization", `${path}.relation`);
      literalEquals(rule.targetPermission, "audit", `${path}.targetPermission`);
      counters.arrows += 1;
      if (counters.arrows > 1) {
        fail(path, "a policy draft can contain only one arrow delegation");
      }
      uniqueLeaf(counters, "arrow:organization:audit", path);
      return { kind: "arrow", relation: "organization", targetPermission: "audit" };
    }
    case "any":
    case "all": {
      const rule = objectAt(input, path, ["kind", "children"]);
      if (!Array.isArray(rule.children)) {
        fail(`${path}.children`, "must be an array");
      }
      if (rule.children.length < 2 || rule.children.length > MAX_COMBINATION_CHILDREN) {
        fail(`${path}.children`, `must contain between 2 and ${MAX_COMBINATION_CHILDREN} rules`);
      }
      const children = rule.children
        .map((child, index) => ruleAt(child, `${path}.children[${index}]`, depth + 1, counters))
        .sort((left, right) => ruleSortKey(left).localeCompare(ruleSortKey(right)));
      return { kind, children };
    }
    default:
      fail(`${path}.kind`, "rule kind is unsupported");
  }
}

function caveatAt(input: unknown, path: string): CaveatSelection {
  const template = objectKind(input, path, "template");
  switch (template) {
    case "amount_limit":
    case "amount_or_override":
    case "approval_gate": {
      objectAt(input, path, ["template"]);
      return { template };
    }
    case "task_purpose_scope":
    case "reconciliation_scope": {
      const caveat = objectAt(input, path, ["template", "taskId", "purpose", "resourceScope"]);
      return {
        template,
        taskId: claimLiteral(caveat.taskId, `${path}.taskId`),
        purpose: claimLiteral(caveat.purpose, `${path}.purpose`),
        resourceScope: claimLiteral(caveat.resourceScope, `${path}.resourceScope`),
      };
    }
    default:
      fail(`${path}.template`, "caveat template is unsupported");
  }
}

function objectKind(input: unknown, path: string, field = "kind"): string {
  const object = objectAt(input, path);
  const value = object[field];
  if (typeof value !== "string") {
    fail(`${path}.${field}`, "must be a string");
  }
  return value;
}

function objectAt(
  input: unknown,
  path: string,
  allowedKeys?: readonly string[],
): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    fail(path, "must be an object");
  }
  const prototype = Object.getPrototypeOf(input);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(path, "must be a plain object");
  }
  const object = input as Record<string, unknown>;
  if (allowedKeys) {
    const allowed = new Set(allowedKeys);
    for (const key of Object.keys(object)) {
      if (!allowed.has(key)) {
        fail(`${path}.${key}`, "field is unsupported");
      }
    }
  }
  return object;
}

function directRelation(input: unknown, path: string): DirectRelation {
  if (typeof input !== "string" || !DIRECT_RELATIONS.has(input as DirectRelation)) {
    fail(path, "direct relation is unsupported");
  }
  return input as DirectRelation;
}

function schemaIdentifier(input: unknown, path: string): string {
  if (typeof input !== "string" || !SCHEMA_IDENTIFIER.test(input)) {
    fail(path, "must be a lowercase SpiceDB identifier of at most 63 characters");
  }
  return input;
}

function publicIdentifier(input: unknown, path: string): string {
  if (typeof input !== "string" || !PUBLIC_IDENTIFIER.test(input)) {
    fail(path, "must be a bounded public identifier");
  }
  return input;
}

function claimLiteral(input: unknown, path: string): string {
  if (typeof input !== "string" || !CLAIM_LITERAL.test(input)) {
    fail(path, "must be a bounded task, purpose, or scope value");
  }
  return input;
}

function boundedInteger(input: unknown, minimum: number, maximum: number, path: string): number {
  if (!Number.isInteger(input) || (input as number) < minimum || (input as number) > maximum) {
    fail(path, `must be an integer between ${minimum} and ${maximum}`);
  }
  return input as number;
}

function numberEquals(input: unknown, expected: number, path: string): void {
  if (input !== expected) {
    fail(path, `must equal ${expected}`);
  }
}

function literalEquals<T extends string>(
  input: unknown,
  expected: T,
  path: string,
): asserts input is T {
  if (input !== expected) {
    fail(path, `must equal ${expected}`);
  }
}

function uniqueLeaf(counters: RuleCounters, key: string, path: string): void {
  if (counters.leaves.has(key)) {
    fail(path, "duplicate rule leaf is unsupported");
  }
  counters.leaves.add(key);
}

function ruleSortKey(rule: PolicyRule): string {
  switch (rule.kind) {
    case "direct":
      return `direct:${rule.relation}:${rule.caveat?.template ?? ""}`;
    case "subject_set":
      return `subject_set:${rule.relation}`;
    case "arrow":
      return `arrow:${rule.relation}:${rule.targetPermission}`;
    case "any":
    case "all":
      return `${rule.kind}:[${rule.children.map(ruleSortKey).join(",")}]`;
  }
}

function fail(path: string, message: string): never {
  throw new PolicyDraftValidationError(path, message);
}
