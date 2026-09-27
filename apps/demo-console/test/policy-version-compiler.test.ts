import { chmod, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { loadPolicyVersionCandidate, preparePolicyVersion } from "../lib/policy-version/store.ts";

const admin = process.env.RAZIEL_POLICY_VERSION_TEST_ADMIN;
const compiler = process.env.RAZIEL_POLICY_COMPILER_EXECUTABLE;

describe.skipIf(!admin || !compiler)("real owner allocation and pinned policy compiler", () => {
  it("prepares coexisting family versions without reusing identities or changing older bytes", async () => {
    if (!admin || !compiler) throw new Error("owner tools are required");
    const root = await mkdtemp(join(tmpdir(), "raziel-policy-version-live-"));
    try {
      await chmod(root, 0o700);
      const config = {
        candidateRoot: join(root, "candidates"),
        versionRoot: join(root, "versions"),
        adminExecutable: admin,
        compilerExecutable: compiler,
      };
      await mkdir(config.candidateRoot, { mode: 0o700 });
      await mkdir(config.versionRoot, { mode: 0o700 });
      const firstInput = request("version-live-first", "viewer");
      const first = await preparePolicyVersion(firstInput, config);
      const source = await readFile(
        join(config.candidateRoot, first.candidate.candidate_id, "schema.zed"),
      );
      const secondInput = {
        ...request("version-live-second", "auditor"),
        familyId: first.identity.familyId,
      };
      const second = await preparePolicyVersion(secondInput, config);
      expect(first.identity.version).toBe(1);
      expect(second.identity.version).toBe(2);
      expect(second.identity.familyId).toBe(first.identity.familyId);
      expect(second.identity.versionId).not.toBe(first.identity.versionId);
      expect(second.candidate.policy_hash).not.toBe(first.candidate.policy_hash);
      expect(second.candidate.policy_name).not.toBe(first.candidate.policy_name);
      expect(await preparePolicyVersion(firstInput, config)).toEqual(first);
      expect(await preparePolicyVersion(secondInput, config)).toEqual(second);
      expect(await loadPolicyVersionCandidate(first.candidate.candidate_id, config)).toEqual(first);
      expect(
        await readFile(join(config.candidateRoot, first.candidate.candidate_id, "schema.zed")),
      ).toEqual(source);
      await expect(
        preparePolicyVersion({ ...firstInput, validForSeconds: 90_000 }, config),
      ).rejects.toMatchObject({ code: "CANDIDATE_CONFLICT" });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

function request(candidateId: string, relation: "viewer" | "auditor") {
  return {
    draft: {
      formatVersion: 1,
      candidateId,
      policyName: "processor-ledger-access",
      policyVersion: 99,
      resourceType: "caller_alias",
      permission: "query",
      rule: { kind: "direct", relation },
    },
    expectedApprovalEpoch: 1,
    validForSeconds: 86_400,
  };
}
