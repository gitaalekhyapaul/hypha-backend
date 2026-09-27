import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { fromBinary } from "@bufbuild/protobuf";
import { PolicyProgramSchema } from "@raziel/proto-ts";
import { describe, expect, it } from "vitest";

import {
  CAVEAT_CATALOG,
  type CaveatSelection,
  M5_COMPILER_LIMITS,
  M5_COMPILER_VERSION,
  type PolicyDraftV1,
  PolicyDraftValidationError,
  type PolicyRule,
  type PolicyVersionIdentity,
  renderPolicySource,
  renderPolicyVersionSource,
  validatePolicyVersionIdentity,
} from "../lib/policy-draft/index.ts";

const IDENTITY: PolicyVersionIdentity = {
  familyId: "0123456789abcdef0123456789abcdef",
  versionId: "fedcba9876543210fedcba9876543210",
  version: 1,
};
const DATASET = `dataset_${IDENTITY.versionId}`;
const TEAM = `team_${IDENTITY.versionId}`;
const ORGANIZATION = `organization_${IDENTITY.versionId}`;
const CAVEAT = `caveat_${IDENTITY.versionId}_0`;
const VIEWER: PolicyRule = { kind: "direct", relation: "viewer" };
const TEAM_RULE: PolicyRule = { kind: "subject_set", relation: "team_viewer" };
const ARROW: PolicyRule = {
  kind: "arrow",
  relation: "organization",
  targetPermission: "audit",
};
const MIXED: PolicyRule = {
  kind: "all",
  children: [
    ARROW,
    TEAM_RULE,
    {
      kind: "any",
      children: [
        { kind: "direct", relation: "auditor" },
        { kind: "direct", relation: "viewer", caveat: { template: "approval_gate" } },
      ],
    },
  ],
};
const BOUNDED: PolicyRule = {
  kind: "all",
  children: [
    ARROW,
    TEAM_RULE,
    { kind: "direct", relation: "owner" },
    {
      kind: "any",
      children: [
        { kind: "direct", relation: "auditor" },
        {
          kind: "all",
          children: [{ kind: "direct", relation: "approver" }, VIEWER],
        },
      ],
    },
  ],
};
const FAMILIES: readonly (readonly [string, PolicyRule])[] = [
  ...(["approver", "auditor", "owner", "viewer"] as const).map(
    (relation): readonly [string, PolicyRule] => [relation, { kind: "direct", relation }],
  ),
  ["subject set", TEAM_RULE],
  ["arrow", ARROW],
  ["union", { kind: "any", children: [VIEWER, TEAM_RULE] }],
  ["intersection", { kind: "all", children: [VIEWER, ARROW] }],
  ...CAVEAT_CATALOG.map((entry): readonly [string, PolicyRule] => [
    entry.template,
    { kind: "direct", relation: "viewer", caveat: selection(entry.template) },
  ]),
  ["mixed", MIXED],
  ["maximum depth and width", BOUNDED],
];

describe("immutable policy version rendering", () => {
  it.each(FAMILIES)("renders the supported %s family", (_name, rule) => {
    const rendered = renderPolicyVersionSource(draft(rule), IDENTITY);
    expect(rendered.identity).toEqual(IDENTITY);
    expect(rendered.signedPolicyName).toBe(`family:${IDENTITY.familyId}:v1`);
    expect(rendered.draft).toMatchObject({
      policyName: rendered.signedPolicyName,
      policyVersion: 1,
      resourceType: DATASET,
      permission: "query",
    });
    expect(rendered.compilerVersion).toBe(M5_COMPILER_VERSION);
    expect(rendered.schema).toContain(`definition ${DATASET} {`);
    expect(rendered.schema.match(/^definition agent \{\}$/gm)).toHaveLength(1);
    expect(rendered.inventory.definitions).toEqual(
      [...rendered.schema.matchAll(/^definition ([a-z0-9_]+) /gm)].map((match) => match[1]).sort(),
    );
    expect(rendered.inventory.caveats).toEqual(
      [...rendered.schema.matchAll(/^caveat ([a-z0-9_]+)\(/gm)].map((match) => match[1]),
    );
    for (const name of [...rendered.inventory.definitions, ...rendered.inventory.caveats]) {
      expect(name).toMatch(/^[a-z][a-z0-9_]{0,62}$/);
      if (name !== "agent") expect(name).toContain(IDENTITY.versionId);
    }
    expect(rendered.inventory.definitions.length).toBeLessThanOrEqual(16);
    expect(JSON.parse(rendered.compilerConfig).limits).toEqual(M5_COMPILER_LIMITS);
  });

  it("namespaces declarations, references, and compiler caveat keys together", () => {
    const rendered = renderPolicyVersionSource(draft(MIXED), IDENTITY);
    expect(rendered.inventory).toEqual({
      definitions: ["agent", DATASET, ORGANIZATION, TEAM],
      caveats: [CAVEAT],
    });
    expect(rendered.schema).toContain(`relation team_viewer: ${TEAM}#member`);
    expect(rendered.schema).toContain(`relation organization: ${ORGANIZATION}`);
    expect(rendered.schema).toContain(`relation viewer: agent with ${CAVEAT}`);
    expect(rendered.schema).toContain("permission audit = auditor");
    expect(rendered.schema).toContain(
      "permission query = ((auditor + viewer) & organization->audit & team_viewer)",
    );
    expect(JSON.parse(rendered.compilerConfig).caveats).toEqual({
      [CAVEAT]: {
        is_approved: "RELATIONSHIP_CONTEXT",
        is_suspended: "RELATIONSHIP_CONTEXT",
      },
    });
  });

  it.each(CAVEAT_CATALOG)("retains $template parameter provenance and expressions", (entry) => {
    const input = draft({ kind: "direct", relation: "viewer", caveat: selection(entry.template) });
    const legacy = renderPolicySource(input);
    const versioned = renderPolicyVersionSource(input, IDENTITY);
    expect(JSON.parse(versioned.compilerConfig).caveats).toEqual({
      [CAVEAT]: JSON.parse(legacy.compilerConfig).caveats[entry.schemaName],
    });
    expect(versioned.schema.split("\n\n")[0]?.split("\n").slice(1)).toEqual(
      legacy.schema.split("\n\n")[0]?.split("\n").slice(1),
    );
  });

  it("keeps literal namespace words intact and ignores display aliases for naming", () => {
    const literal = "dataset team organization m5_task_purpose_scope";
    const input = draft({
      kind: "direct",
      relation: "viewer",
      caveat: {
        template: "task_purpose_scope",
        taskId: "team",
        purpose: literal,
        resourceScope: "dataset",
      },
    });
    const identity = { ...IDENTITY, version: 2 };
    const rendered = renderPolicyVersionSource(input, identity);
    expect(rendered.schema).toContain('task_id == "team"');
    expect(rendered.schema).toContain(`purpose == "${literal}"`);
    expect(rendered.schema).toContain('resource_scope == "dataset"');
    expect(rendered.draft.policyVersion).toBe(2);
    const renamed = renderPolicyVersionSource(
      { ...input, policyName: "another-display-name", resourceType: "another_dataset" },
      identity,
    );
    expect(renamed).toEqual(rendered);
    expect(input.policyName).toBe("reconciliation-query");
    expect(input.resourceType).toBe("dataset");
    expect(input.policyVersion).toBe(1);
    expect(rendered.identity).not.toBe(identity);
  });

  it("separates versions and unrelated families with identical display names", () => {
    const identities = [
      IDENTITY,
      { ...IDENTITY, versionId: "1".repeat(32), version: 2 },
      { familyId: "2".repeat(32), versionId: "3".repeat(32), version: 1 },
    ];
    const outputs = identities.map((identity) => renderPolicyVersionSource(draft(MIXED), identity));
    expect(new Set(outputs.map((output) => output.signedPolicyName)).size).toBe(3);
    const ownedNames = new Set<string>();
    for (const output of outputs) {
      for (const name of [...output.inventory.definitions, ...output.inventory.caveats]) {
        if (name === "agent") continue;
        expect(ownedNames.has(name)).toBe(false);
        ownedNames.add(name);
      }
    }
    expect(renderPolicyVersionSource(draft(MIXED), IDENTITY)).toEqual(outputs[0]);
  });

  it("retains deterministic child ordering without modifying the caller's tree", () => {
    const input = draft(MIXED);
    const copy = structuredClone(input);
    const reordered = draft({ ...MIXED, children: [...MIXED.children].reverse() });
    expect(renderPolicyVersionSource(input, IDENTITY)).toEqual(
      renderPolicyVersionSource(reordered, IDENTITY),
    );
    expect(input).toEqual(copy);
  });

  it.each([1, 2_147_483_647])("accepts version bound %s within signed-name bounds", (version) => {
    const rendered = renderPolicyVersionSource(draft(VIEWER), { ...IDENTITY, version });
    expect(rendered.signedPolicyName).toBe(`family:${IDENTITY.familyId}:v${version}`);
    expect(Buffer.byteLength(rendered.signedPolicyName)).toBeLessThanOrEqual(128);
  });

  it.each(["0", "f"])("accepts hexadecimal ID boundary %s", (digit) => {
    const identity = { familyId: digit.repeat(32), versionId: digit.repeat(32), version: 1 };
    expect(renderPolicyVersionSource(draft(VIEWER), identity).identity).toEqual(identity);
  });

  it("retains the 128-character caveat literal bound", () => {
    const input = (length: number) =>
      draft({
        kind: "direct",
        relation: "viewer",
        caveat: {
          template: "task_purpose_scope",
          taskId: "t".repeat(length),
          purpose: "p".repeat(length),
          resourceScope: "s".repeat(length),
        },
      });
    expect(renderPolicyVersionSource(input(128), IDENTITY).schema).toContain("t".repeat(128));
    expect(() => renderPolicyVersionSource(input(129), IDENTITY)).toThrow(
      PolicyDraftValidationError,
    );
  });

  it.each([0, -1, 1.5, 2_147_483_648, Number.MAX_SAFE_INTEGER, NaN, Infinity, "1", null])(
    "rejects invalid version %s",
    (version) => {
      expect(() => renderPolicyVersionSource(draft(VIEWER), { ...IDENTITY, version })).toThrow(
        PolicyDraftValidationError,
      );
    },
  );

  for (const field of ["familyId", "versionId"] as const) {
    it.each([
      "",
      "a".repeat(31),
      "a".repeat(33),
      "A".repeat(32),
      "g".repeat(32),
      `${"a".repeat(32)}\n`,
      ` ${"a".repeat(31)}`,
      "../schema",
      `0x${"a".repeat(30)}`,
      "01234567-89ab-cdef-0123-456789abcdef",
      "\u00e9".repeat(32),
      123,
      null,
      undefined,
    ])(`rejects invalid ${field} %s`, (value) => {
      expect(() =>
        renderPolicyVersionSource(draft(VIEWER), { ...IDENTITY, [field]: value }),
      ).toThrow(`${field}: must be exactly 32 lowercase hexadecimal characters`);
    });
  }

  it.each([null, [], "identity", {}, { ...IDENTITY, schema: "raw" }, new Date(0)])(
    "rejects malformed identity %#",
    (identity) => {
      expect(() => validatePolicyVersionIdentity(identity)).toThrow(PolicyDraftValidationError);
    },
  );

  it("retains draft limits and fails closed for unsupported input", () => {
    const twoCaveats: PolicyRule = {
      kind: "all",
      children: (["viewer", "auditor"] as const).map((relation) => ({
        kind: "direct",
        relation,
        caveat: { template: "amount_limit" },
      })),
    };
    const excessiveDepth = { kind: "all", children: [BOUNDED, VIEWER] };
    for (const rule of [
      twoCaveats,
      excessiveDepth,
      { kind: "any", children: [VIEWER] },
      { kind: "any", children: [VIEWER, TEAM_RULE, ARROW, VIEWER, VIEWER] },
      { kind: "any", children: [VIEWER, VIEWER] },
      { kind: "any", children: [ARROW, ARROW] },
      { kind: "exclude", children: [VIEWER, TEAM_RULE] },
      { ...VIEWER, subject: "agent:*" },
    ]) {
      expect(() => renderPolicyVersionSource({ ...draft(VIEWER), rule }, IDENTITY)).toThrow(
        PolicyDraftValidationError,
      );
    }
    expect(() => renderPolicyVersionSource({ ...draft(VIEWER), rawZed: "raw" }, IDENTITY)).toThrow(
      PolicyDraftValidationError,
    );
  });

  it("preserves exact legacy source and result shape", () => {
    const rendered = renderPolicySource(draft(MIXED));
    expect(Object.keys(rendered)).toEqual([
      "draft",
      "compilerVersion",
      "schema",
      "compilerConfig",
      "ruleSummary",
    ]);
    expect(rendered.schema).toBe(
      [
        "caveat m5_approval_gate(is_approved bool, is_suspended bool) {",
        "  is_approved && !is_suspended",
        "}",
        "",
        "definition agent {}",
        "",
        "definition team {",
        "  relation member: agent",
        "}",
        "",
        "definition organization {",
        "  relation auditor: agent",
        "  permission audit = auditor",
        "}",
        "",
        "definition dataset {",
        "  relation auditor: agent",
        "  relation organization: organization",
        "  relation team_viewer: team#member",
        "  relation viewer: agent with m5_approval_gate",
        "",
        "  permission query = ((auditor + viewer) & organization->audit & team_viewer)",
        "}",
        "",
      ].join("\n"),
    );
    expect(rendered.draft.policyName).toBe("reconciliation-query");
    expect(rendered.draft.resourceType).toBe("dataset");
  });
});

const compiler = process.env.RAZIEL_POLICY_COMPILER_EXECUTABLE;

describe.skipIf(!compiler)("immutable version pinned compiler compatibility", () => {
  it.each(FAMILIES)("compiles self-contained %s source with matching inventory", (_name, rule) => {
    const rendered = renderPolicyVersionSource(draft(rule), IDENTITY);
    const root = mkdtempSync(join(tmpdir(), "raziel-policy-version-"));
    try {
      const schema = join(root, "schema.zed");
      const config = join(root, "compiler.json");
      const output = join(root, "policy.pb");
      writeFileSync(schema, rendered.schema, { mode: 0o600 });
      writeFileSync(config, rendered.compilerConfig, { mode: 0o600 });
      if (!compiler) throw new Error("compiler executable is required");
      execFileSync(
        compiler,
        [
          "compile",
          "--schema",
          schema,
          "--config",
          config,
          "--compiler-version",
          rendered.compilerVersion,
          "--out",
          output,
          "--diagnostic-out",
          join(root, "policy.json"),
        ],
        { timeout: 30_000, maxBuffer: 65_536, stdio: "pipe" },
      );
      const program = fromBinary(PolicyProgramSchema, readFileSync(output));
      expect(program.compilerVersion).toBe(M5_COMPILER_VERSION);
      expect(program.definitions.map((definition) => definition.name)).toEqual(
        rendered.inventory.definitions,
      );
      expect(program.caveats.map((caveat) => caveat.name)).toEqual(rendered.inventory.caveats);
      expect(
        program.definitions.find((definition) => definition.name === "agent")?.relations,
      ).toEqual([]);
      expect(program.permissions.some((permission) => permission.objectType === "agent")).toBe(
        false,
      );
      expect(Buffer.from(program.sourceSchemaDigest)).toEqual(
        createHash("sha256").update(rendered.schema).digest(),
      );
      expect(program.limits?.maxTotalPrivateInputBytes).toBe(4096);
      for (const definition of program.definitions) {
        for (const relation of definition.relations) {
          for (const subject of relation.allowedSubjectTypes) {
            expect(rendered.inventory.definitions).toContain(subject.objectType);
            if (subject.caveatName)
              expect(rendered.inventory.caveats).toContain(subject.caveatName);
          }
        }
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

function draft(rule: PolicyRule): PolicyDraftV1 {
  return {
    formatVersion: 1,
    candidateId: "candidate-version-test",
    policyName: "reconciliation-query",
    policyVersion: 1,
    resourceType: "dataset",
    permission: "query",
    rule,
  };
}

function selection(template: CaveatSelection["template"]): CaveatSelection {
  if (template === "task_purpose_scope" || template === "reconciliation_scope") {
    return {
      template,
      taskId: "task-test",
      purpose: "reconciliation",
      resourceScope: "synthetic-test",
    };
  }
  return { template };
}
