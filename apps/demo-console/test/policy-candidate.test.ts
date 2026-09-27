import { createHash } from "node:crypto";
import { chmod, lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import {
  type PolicyCandidateConfig,
  PolicyCandidateError,
  type PolicyCandidateProjection,
  type ProcessInvocation,
  type ProcessRunner,
  preparePolicyCandidate,
  runDirectProcess,
} from "../lib/policy-candidate/index.ts";
import { loadPolicyVersionCandidate, preparePolicyVersion } from "../lib/policy-version/store.ts";
import { validateVersionRequest } from "../lib/policy-version/validation.ts";

const NOW = 1_800_000_000;
const POLICY_HASH = "ab".repeat(32);
const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

describe("M5 policy candidate preparation", () => {
  it("turns validated caller input into fixed private compiler and manifest files", async () => {
    const fixture = await createFixture();
    const invocations: ProcessInvocation[] = [];
    const projection = await preparePolicyCandidate(request(), fixture.config, {
      nowUnixSeconds: () => NOW,
      runProcess: successfulRunner(invocations),
    });

    expect(projection).toMatchObject({
      candidate_id: "candidate-m5-001",
      compiler_version: "1.54.0",
      policy_name: "reconciliation-query",
      policy_version: "1",
      resource_type: "dataset",
      permission: "query",
      rule_summary: "viewer",
      policy_hash: POLICY_HASH,
      expected_approval_epoch: "1",
      status: "COMPILED",
    });
    expect(invocations).toHaveLength(2);
    expect(invocations[0]).toMatchObject({
      executable: fixture.config.compilerExecutable,
      timeoutMs: 30_000,
      maxOutputBytes: 65_536,
    });
    expect(invocations[1]).toMatchObject({
      executable: fixture.config.adminExecutable,
      timeoutMs: 30_000,
      maxOutputBytes: 65_536,
    });
    expect(invocations.flatMap((invocation) => invocation.args)).not.toContain("raw .zed input");

    const candidateDirectory = join(fixture.config.candidateRoot, "candidate-m5-001");
    expect(mode(await lstat(candidateDirectory))).toBe(0o700);
    for (const file of [
      "schema.zed",
      "compiler.json",
      "policy.pb",
      "policy.json",
      "candidate.pb",
      "review.json",
      "state.json",
    ]) {
      expect(mode(await lstat(join(candidateDirectory, file)))).toBe(0o600);
    }
    expect(await readFile(join(candidateDirectory, "candidate.pb"))).toEqual(Buffer.from([2]));
    expect(JSON.parse(await readFile(join(candidateDirectory, "state.json"), "utf8"))).toEqual({
      format_version: 1,
      candidate_id: "candidate-m5-001",
      request_digest: expect.stringMatching(/^[0-9a-f]{64}$/),
      status: "COMPILED",
      valid_from_unix_seconds: NOW,
      valid_until_unix_seconds: NOW + 86_400,
      created_at_unix_seconds: NOW,
      expires_at_unix_seconds: NOW + 86_400,
      manifest_digest: expect.stringMatching(/^[0-9a-f]{64}$/),
      review_digest: expect.stringMatching(/^[0-9a-f]{64}$/),
      last_error: null,
    });
  });

  it("rejects caller-selected schema and unknown metadata before creating a candidate", async () => {
    const fixture = await createFixture();
    let called = false;
    const base = request();
    await expect(
      preparePolicyCandidate(
        {
          ...base,
          draft: { ...(base.draft as Record<string, unknown>), rawZed: "raw .zed input" },
        },
        fixture.config,
        {
          nowUnixSeconds: () => NOW,
          runProcess: async () => {
            called = true;
          },
        },
      ),
    ).rejects.toMatchObject({ code: "INVALID_CANDIDATE_REQUEST" });
    await expect(
      preparePolicyCandidate({ ...request(), schema: "raw .zed input" }, fixture.config),
    ).rejects.toMatchObject({ code: "INVALID_CANDIDATE_REQUEST" });
    expect(called).toBe(false);
    expect(await readFileNames(fixture.config.candidateRoot)).toEqual([]);
  });

  it("records a bounded compiler failure without storing process output", async () => {
    const fixture = await createFixture();
    const privateDiagnostic = "private compiler diagnostic and schema path";
    const runProcess: ProcessRunner = async () => {
      throw new Error(privateDiagnostic);
    };

    let caught: unknown;
    try {
      await preparePolicyCandidate(request(), fixture.config, {
        nowUnixSeconds: () => NOW,
        runProcess,
      });
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCandidateError);
    expect(caught).toMatchObject({ code: "COMPILER_FAILED" });
    expect(String(caught)).not.toContain(privateDiagnostic);

    const state = JSON.parse(
      await readFile(join(fixture.config.candidateRoot, "candidate-m5-001", "state.json"), "utf8"),
    );
    expect(state).toMatchObject({ status: "DRAFT", last_error: "COMPILER_FAILED" });

    const retried = await preparePolicyCandidate(request(), fixture.config, {
      nowUnixSeconds: () => NOW + 1,
      runProcess: successfulRunner([]),
    });
    expect(retried.status).toBe("COMPILED");
  });

  it("rejects an inspector projection that adds a private field", async () => {
    const fixture = await createFixture();
    const runner = successfulRunner([], { policy_ir: { definitions: ["private"] } });
    await expect(
      preparePolicyCandidate(request(), fixture.config, {
        nowUnixSeconds: () => NOW,
        runProcess: runner,
      }),
    ).rejects.toMatchObject({ code: "INSPECTION_FAILED" });
  });

  it("returns an exact completed retry and rejects changed input", async () => {
    const fixture = await createFixture();
    const first = await preparePolicyCandidate(request(), fixture.config, {
      nowUnixSeconds: () => NOW,
      runProcess: successfulRunner([]),
    });
    let reran = false;
    const retry = await preparePolicyCandidate(request(), fixture.config, {
      nowUnixSeconds: () => NOW + 1,
      runProcess: async () => {
        reran = true;
      },
    });
    expect(retry).toEqual(first);
    expect(reran).toBe(false);

    await expect(
      preparePolicyCandidate({ ...request(), expectedApprovalEpoch: 2 }, fixture.config, {
        nowUnixSeconds: () => NOW + 1,
        runProcess: successfulRunner([]),
      }),
    ).rejects.toMatchObject({ code: "CANDIDATE_CONFLICT" });
  });

  it("bounds direct process output and execution time", async () => {
    const fixture = await createFixture();
    const base = {
      executable: process.execPath,
      cwd: fixture.config.candidateRoot,
      maxOutputBytes: 64,
    } as const;

    await runDirectProcess({
      ...base,
      args: ["-e", "process.stdout.write('ok')"],
      timeoutMs: 1_000,
    });
    await expect(
      runDirectProcess({
        ...base,
        args: ["-e", "process.stdout.write('x'.repeat(65))"],
        timeoutMs: 1_000,
      }),
    ).rejects.toMatchObject({ code: "CANDIDATE_STORE_UNAVAILABLE" });
    await expect(
      runDirectProcess({
        ...base,
        args: ["-e", "setInterval(() => undefined, 1_000)"],
        timeoutMs: 20,
      }),
    ).rejects.toMatchObject({ code: "CANDIDATE_STORE_UNAVAILABLE" });
  });
});

describe("M6 owner-allocated policy candidates", () => {
  it("compiles the allocated namespace and reuses it without caller-selected versions", async () => {
    const fixture = await createFixture();
    const versionRoot = join(fixture.config.candidateRoot, "..", "versions");
    await mkdir(versionRoot, { mode: 0o700 });
    const config = { ...fixture.config, versionRoot };
    const invocations: ProcessInvocation[] = [];
    const runner = successfulRunner(invocations);
    const input = request();
    const prepared = await preparePolicyVersion(input, config, {
      nowUnixSeconds: () => NOW,
      runProcess: runner,
    });
    expect(prepared.identity).toEqual({
      familyId: "12".repeat(16),
      versionId: "34".repeat(16),
      version: 1,
    });
    expect(prepared.candidate).toMatchObject({
      policy_name: `family:${"12".repeat(16)}:v1`,
      policy_version: "1",
      resource_type: `dataset_${"34".repeat(16)}`,
      expected_approval_epoch: "1",
    });
    const schema = await readFile(
      join(config.candidateRoot, "candidate-m5-001", "schema.zed"),
      "utf8",
    );
    expect(schema).toContain(`definition dataset_${"34".repeat(16)}`);
    expect(invocations.map((call) => call.args[0])).toEqual([
      "allocate-policy-version",
      "compile",
      "inspect-policy",
    ]);
    expect(
      await preparePolicyVersion(input, config, {
        nowUnixSeconds: () => NOW + 1,
        runProcess: async () => {
          throw new Error("unexpected rerun");
        },
      }),
    ).toEqual(prepared);
    expect(
      await loadPolicyVersionCandidate("candidate-m5-001", config, {
        nowUnixSeconds: () => NOW + 1,
      }),
    ).toEqual(prepared);
    await expect(
      preparePolicyVersion({ ...input, validForSeconds: 90_000 }, config),
    ).rejects.toMatchObject({ code: "CANDIDATE_CONFLICT" });
    await expect(
      preparePolicyVersion({ ...input, familyId: "12".repeat(16) }, config),
    ).rejects.toMatchObject({ code: "CANDIDATE_CONFLICT" });
  });

  it("keeps a reserved version for a compiler retry and rejects a retargeted projection", async () => {
    const fixture = await createFixture();
    const versionRoot = join(fixture.config.candidateRoot, "..", "versions");
    await mkdir(versionRoot, { mode: 0o700 });
    const config = { ...fixture.config, versionRoot };
    const runner = successfulRunner([]);
    await expect(
      preparePolicyVersion(request(), config, {
        nowUnixSeconds: () => NOW,
        runProcess: async (invocation) => {
          if (invocation.args[0] === "compile") throw new Error("private diagnostic");
          await runner(invocation);
        },
      }),
    ).rejects.toMatchObject({ code: "COMPILER_FAILED" });
    const calls: ProcessInvocation[] = [];
    const retried = await preparePolicyVersion(request(), config, {
      nowUnixSeconds: () => NOW + 1,
      runProcess: successfulRunner(calls),
    });
    expect(calls.map((call) => call.args[0])).toEqual(["compile", "inspect-policy"]);
    expect(retried.identity.version).toBe(1);
    const path = join(versionRoot, "candidate-m5-001.json");
    const allocation = JSON.parse(await readFile(path, "utf8"));
    await writeFile(path, JSON.stringify({ ...allocation, version_id: "56".repeat(16) }));
    await expect(
      loadPolicyVersionCandidate("candidate-m5-001", config, { nowUnixSeconds: () => NOW + 2 }),
    ).rejects.toMatchObject({ code: "CANDIDATE_CONFLICT" });
  });

  it("rejects reapproval, client-selected identity, and private storage below candidate cleanup", async () => {
    expect(() => validateVersionRequest({ ...request(), expectedApprovalEpoch: 2 })).toThrow(
      PolicyCandidateError,
    );
    expect(() => validateVersionRequest({ ...request(), versionId: "12".repeat(16) })).toThrow(
      PolicyCandidateError,
    );
    expect(() => validateVersionRequest({ ...request(), familyId: "../private" })).toThrow(
      PolicyCandidateError,
    );
    const fixture = await createFixture();
    await expect(
      preparePolicyVersion(request(), {
        ...fixture.config,
        versionRoot: join(fixture.config.candidateRoot, "versions"),
      }),
    ).rejects.toMatchObject({ code: "INVALID_OWNER_CONFIGURATION" });
  });
});

function successfulRunner(
  invocations: ProcessInvocation[],
  extraProjection: Record<string, unknown> = {},
): ProcessRunner {
  return async (invocation) => {
    invocations.push(invocation);
    if (invocation.args[0] === "allocate-policy-version") {
      await writeFile(
        join(argument(invocation, "--root"), `${argument(invocation, "--candidate-id")}.json`),
        JSON.stringify({
          format_version: 1,
          candidate_id: argument(invocation, "--candidate-id"),
          request_digest: argument(invocation, "--request-digest"),
          requested_family_id: invocation.args.includes("--family-id")
            ? argument(invocation, "--family-id")
            : null,
          family_id: "12".repeat(16),
          version_id: "34".repeat(16),
          version: invocation.args.includes("--family-id") ? 3 : 1,
        }),
        { mode: 0o600 },
      );
      return;
    }
    if (invocation.executable.endsWith("compiler")) {
      await writeFile(argument(invocation, "--out"), Buffer.from([1]), { mode: 0o600 });
      await writeFile(argument(invocation, "--diagnostic-out"), "{}\n", { mode: 0o600 });
      return;
    }
    const projection = await projectionFor(invocation);
    await writeFile(
      argument(invocation, "--output"),
      `${JSON.stringify({ ...projection, ...extraProjection })}\n`,
      { mode: 0o600 },
    );
    await writeFile(argument(invocation, "--state-output"), Buffer.from([2]), { mode: 0o600 });
  };
}

async function projectionFor(invocation: ProcessInvocation): Promise<PolicyCandidateProjection> {
  const schema = await readFile(join(invocation.cwd, "schema.zed"));
  return {
    candidate_id: argument(invocation, "--candidate-id"),
    compiler_version: "1.54.0",
    record_id: argument(invocation, "--record-id"),
    policy_name: argument(invocation, "--policy-name"),
    policy_version: argument(invocation, "--policy-version"),
    resource_type: argument(invocation, "--resource-type"),
    permission: "query",
    rule_summary: argument(invocation, "--rule-summary"),
    source_schema_digest: createHash("sha256").update(schema).digest("hex"),
    policy_hash: POLICY_HASH,
    expected_approval_epoch: argument(invocation, "--expected-approval-epoch"),
    valid_from: timestamp(argument(invocation, "--valid-from")),
    valid_until: timestamp(argument(invocation, "--valid-until")),
    status: "COMPILED",
    created_at: timestamp(argument(invocation, "--created-at")),
    expires_at: timestamp(argument(invocation, "--expires-at")),
  };
}

function request(): Record<string, unknown> {
  return {
    draft: {
      formatVersion: 1,
      candidateId: "candidate-m5-001",
      policyName: "reconciliation-query",
      policyVersion: 1,
      resourceType: "dataset",
      permission: "query",
      rule: { kind: "direct", relation: "viewer" },
    },
    expectedApprovalEpoch: 1,
    validForSeconds: 86_400,
  };
}

async function createFixture(): Promise<{ readonly config: PolicyCandidateConfig }> {
  const root = await mkdtemp(join(tmpdir(), "raziel-policy-candidate-test-"));
  roots.push(root);
  await chmod(root, 0o700);
  const candidateRoot = join(root, "candidates");
  await mkdir(candidateRoot, { mode: 0o700 });
  const compilerExecutable = join(root, "compiler");
  const adminExecutable = join(root, "admin");
  await writeFile(compilerExecutable, "test executable", { mode: 0o700 });
  await writeFile(adminExecutable, "test executable", { mode: 0o700 });
  return { config: { candidateRoot, compilerExecutable, adminExecutable } };
}

function argument(invocation: ProcessInvocation, name: string): string {
  const index = invocation.args.indexOf(name);
  const value = invocation.args[index + 1];
  if (index < 0 || value === undefined) {
    throw new Error(`missing ${name}`);
  }
  return value;
}

function timestamp(seconds: string): { readonly seconds: string; readonly nanos: 0 } {
  return { nanos: 0, seconds };
}

function mode(metadata: { readonly mode: number }): number {
  return metadata.mode & 0o777;
}

async function readFileNames(path: string): Promise<string[]> {
  const { readdir } = await import("node:fs/promises");
  return readdir(path);
}
