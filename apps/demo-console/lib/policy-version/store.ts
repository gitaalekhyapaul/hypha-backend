import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { lstat, open } from "node:fs/promises";
import { join } from "node:path";
import { runDirectProcess } from "../policy-candidate/process.ts";
import {
  loadPolicyCandidateProjection,
  prepareAllocatedPolicyCandidate,
} from "../policy-candidate/store.ts";
import {
  type PolicyCandidateDependencies,
  PolicyCandidateError,
} from "../policy-candidate/types.ts";
import { validateCandidateReference } from "../policy-candidate/validation.ts";
import type { PolicyVersionIdentity } from "../policy-draft/types.ts";
import type {
  PolicyVersionAllocation,
  PolicyVersionCandidate,
  PolicyVersionConfig,
} from "./types.ts";
import { validateAllocation, validateVersionConfig, validateVersionRequest } from "./validation.ts";

export async function preparePolicyVersion(
  input: unknown,
  configuration: PolicyVersionConfig,
  dependencies: PolicyCandidateDependencies = {},
): Promise<PolicyVersionCandidate> {
  const request = validateVersionRequest(input);
  const config = validateVersionConfig(configuration);
  await validateRoot(config.versionRoot);
  const digest = createHash("sha256").update(JSON.stringify(request)).digest("hex");
  const candidateId = request.draft.candidateId;
  let allocation = await readAllocation(config.versionRoot, candidateId);
  if (!allocation) {
    await (dependencies.runProcess ?? runDirectProcess)({
      executable: config.adminExecutable,
      args: [
        "allocate-policy-version",
        "--root",
        config.versionRoot,
        "--candidate-id",
        candidateId,
        "--request-digest",
        digest,
        ...(request.familyId ? ["--family-id", request.familyId] : []),
      ],
      cwd: config.versionRoot,
      timeoutMs: 30_000,
      maxOutputBytes: 64 * 1024,
    });
    allocation = await readAllocation(config.versionRoot, candidateId);
  }
  if (
    !allocation ||
    allocation.request_digest !== digest ||
    allocation.requested_family_id !== (request.familyId ?? null)
  ) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  const identity = identityFor(allocation);
  const candidate = await prepareAllocatedPolicyCandidate(
    {
      draft: request.draft,
      expectedApprovalEpoch: request.expectedApprovalEpoch,
      validForSeconds: request.validForSeconds,
    },
    identity,
    config,
    dependencies,
  );
  return { identity, candidate };
}

export async function loadPolicyVersionCandidate(
  input: unknown,
  configuration: PolicyVersionConfig,
  dependencies: Pick<PolicyCandidateDependencies, "nowUnixSeconds"> = {},
): Promise<PolicyVersionCandidate> {
  const candidateId = validateCandidateReference(input);
  const config = validateVersionConfig(configuration);
  await validateRoot(config.versionRoot);
  const allocation = await readAllocation(config.versionRoot, candidateId);
  if (!allocation) throw new PolicyCandidateError("CANDIDATE_NOT_FOUND");
  const identity = identityFor(allocation);
  const candidate = await loadPolicyCandidateProjection(candidateId, config, dependencies);
  if (
    candidate.policy_name !== `family:${identity.familyId}:v${identity.version}` ||
    candidate.policy_version !== String(identity.version) ||
    candidate.resource_type !== `dataset_${identity.versionId}`
  ) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  return { identity, candidate };
}

function identityFor(allocation: PolicyVersionAllocation): PolicyVersionIdentity {
  return {
    familyId: allocation.family_id,
    versionId: allocation.version_id,
    version: allocation.version,
  };
}

async function validateRoot(root: string): Promise<void> {
  try {
    const metadata = await lstat(root);
    if (
      !metadata.isDirectory() ||
      metadata.isSymbolicLink() ||
      (metadata.mode & 0o7777) !== 0o700 ||
      (typeof process.getuid === "function" && metadata.uid !== process.getuid())
    ) {
      throw new Error("invalid root");
    }
  } catch {
    throw new PolicyCandidateError("INVALID_OWNER_CONFIGURATION");
  }
}

async function readAllocation(
  root: string,
  candidateId: string,
): Promise<PolicyVersionAllocation | null> {
  let file: Awaited<ReturnType<typeof open>> | undefined;
  try {
    file = await open(join(root, `${candidateId}.json`), constants.O_RDONLY | constants.O_NOFOLLOW);
    const metadata = await file.stat();
    if (
      !metadata.isFile() ||
      metadata.nlink !== 1 ||
      metadata.size > 4096 ||
      (metadata.mode & 0o7777) !== 0o600 ||
      (typeof process.getuid === "function" && metadata.uid !== process.getuid())
    ) {
      throw new PolicyCandidateError("CANDIDATE_CONFLICT");
    }
    const bytes = Buffer.alloc(4097);
    const { bytesRead } = await file.read(bytes);
    if (bytesRead > 4096) throw new PolicyCandidateError("CANDIDATE_CONFLICT");
    const allocation = validateAllocation(
      JSON.parse(bytes.subarray(0, bytesRead).toString("utf8")),
    );
    if (allocation.candidate_id !== candidateId)
      throw new PolicyCandidateError("CANDIDATE_CONFLICT");
    return allocation;
  } catch (error) {
    if (error instanceof PolicyCandidateError) throw error;
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  } finally {
    await file?.close();
  }
}
