import { createHash } from "node:crypto";
import { constants as fsConstants } from "node:fs";
import {
  chmod,
  type FileHandle,
  lstat,
  mkdir,
  open,
  readFile,
  rename,
  stat,
  unlink,
} from "node:fs/promises";
import { basename, join } from "node:path";

import {
  M5_COMPILER_VERSION,
  type PolicyDraftV1,
  type RenderedPolicySource,
  renderPolicySource,
} from "../policy-draft/index.ts";
import { runDirectProcess } from "./process.ts";
import {
  POLICY_CANDIDATE_STATE_FORMAT_VERSION,
  type PolicyCandidateConfig,
  type PolicyCandidateDependencies,
  PolicyCandidateError,
  type PolicyCandidateFailureCode,
  type PolicyCandidateProjection,
  type PreparePolicyCandidateRequest,
  type ProcessInvocation,
} from "./types.ts";
import {
  validateCandidateConfig,
  validateCandidateReference,
  validateCandidateRequest,
} from "./validation.ts";

const PRIVATE_DIRECTORY_MODE = 0o700;
const PRIVATE_FILE_MODE = 0o600;
const PROCESS_TIMEOUT_MS = 30_000;
const MAX_PROCESS_OUTPUT_BYTES = 64 * 1024;
const MAX_SAFE_PROJECTION_BYTES = 64 * 1024;
const MAX_PRIVATE_ARTIFACT_BYTES = 4 * 1024 * 1024;
const CANDIDATE_LIFETIME_SECONDS = 24 * 60 * 60;
const HEX_DIGEST = /^[0-9a-f]{64}$/;

interface CandidatePaths {
  readonly directory: string;
  readonly schema: string;
  readonly compilerConfig: string;
  readonly policy: string;
  readonly diagnostic: string;
  readonly manifest: string;
  readonly review: string;
  readonly state: string;
}

interface CandidateTimes {
  readonly createdAt: number;
  readonly expiresAt: number;
  readonly validFrom: number;
  readonly validUntil: number;
}

interface CandidateState {
  readonly format_version: typeof POLICY_CANDIDATE_STATE_FORMAT_VERSION;
  readonly candidate_id: string;
  readonly request_digest: string;
  readonly status: "COMPILED" | "DRAFT";
  readonly valid_from_unix_seconds: number;
  readonly valid_until_unix_seconds: number;
  readonly created_at_unix_seconds: number;
  readonly expires_at_unix_seconds: number;
  readonly manifest_digest: string | null;
  readonly review_digest: string | null;
  readonly last_error: PolicyCandidateFailureCode | null;
}

export async function preparePolicyCandidate(
  input: unknown,
  candidateConfig: PolicyCandidateConfig,
  dependencies: PolicyCandidateDependencies = {},
): Promise<PolicyCandidateProjection> {
  const request = validateCandidateRequest(input);
  const config = validateCandidateConfig(candidateConfig);
  const nowUnixSeconds = dependencies.nowUnixSeconds ?? systemNowUnixSeconds;
  const runProcess = dependencies.runProcess ?? runDirectProcess;
  const paths = candidatePaths(config.candidateRoot, request.draft.candidateId);
  const rendered = renderPolicySource(request.draft);
  const recordId = deriveRecordId(request.draft);
  const digest = requestDigest(request);

  try {
    await validateOwnerRoot(config.candidateRoot);
    await validateExecutable(config.compilerExecutable);
    await validateExecutable(config.adminExecutable);
  } catch (error) {
    throw asCandidateError(error, "INVALID_OWNER_CONFIGURATION");
  }

  try {
    await mkdir(paths.directory, { mode: PRIVATE_DIRECTORY_MODE });
    await verifyMode(paths.directory, PRIVATE_DIRECTORY_MODE, "directory");
  } catch (error) {
    if (isConflict(error)) {
      return resumePolicyCandidate(
        paths,
        request,
        rendered,
        recordId,
        digest,
        config,
        runProcess,
        nowUnixSeconds(),
      );
    }
    throw asCandidateError(error, "CANDIDATE_STORE_UNAVAILABLE");
  }

  const times = candidateTimes(nowUnixSeconds(), request.validForSeconds);
  const initialState = candidateState(request.draft.candidateId, digest, times, "DRAFT", null);
  await writePrivateNew(paths.schema, rendered.schema);
  await writePrivateNew(paths.compilerConfig, rendered.compilerConfig);
  await writeState(paths.state, initialState);
  return advancePolicyCandidate(
    paths,
    request,
    rendered,
    recordId,
    config,
    runProcess,
    initialState,
  );
}

export async function loadPolicyCandidateProjection(
  candidateIdInput: unknown,
  candidateConfig: PolicyCandidateConfig,
  dependencies: Pick<PolicyCandidateDependencies, "nowUnixSeconds"> = {},
): Promise<PolicyCandidateProjection> {
  const candidateId = validateCandidateReference(candidateIdInput);
  const config = validateCandidateConfig(candidateConfig);
  const paths = candidatePaths(config.candidateRoot, candidateId);
  const nowUnixSeconds = (dependencies.nowUnixSeconds ?? systemNowUnixSeconds)();
  try {
    await validateOwnerRoot(config.candidateRoot);
    await verifyMode(paths.directory, PRIVATE_DIRECTORY_MODE, "directory");
    const state = await readState(paths.state);
    if (state.candidate_id !== candidateId) {
      throw new PolicyCandidateError("CANDIDATE_CONFLICT");
    }
    if (!Number.isSafeInteger(nowUnixSeconds) || nowUnixSeconds >= state.expires_at_unix_seconds) {
      throw new PolicyCandidateError("CANDIDATE_EXPIRED");
    }
    if (state.status !== "COMPILED") {
      throw new PolicyCandidateError("CANDIDATE_NOT_READY");
    }
    await requireArtifactDigest(paths.manifest, state.manifest_digest);
    await requireArtifactDigest(paths.review, state.review_digest);
    const projection = await readStrictProjection(paths.review);
    if (
      projection.candidate_id !== candidateId ||
      projection.status !== "COMPILED" ||
      !projectionValueMatches(projection.created_at, timestamp(state.created_at_unix_seconds)) ||
      !projectionValueMatches(projection.expires_at, timestamp(state.expires_at_unix_seconds))
    ) {
      throw new PolicyCandidateError("CANDIDATE_CONFLICT");
    }
    return projection;
  } catch (error) {
    if (error instanceof PolicyCandidateError) {
      throw error;
    }
    if (isNotFound(error)) {
      throw new PolicyCandidateError("CANDIDATE_NOT_FOUND");
    }
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  }
}

async function resumePolicyCandidate(
  paths: CandidatePaths,
  request: PreparePolicyCandidateRequest,
  rendered: RenderedPolicySource,
  recordId: string,
  digest: string,
  config: PolicyCandidateConfig,
  runProcess: (invocation: ProcessInvocation) => Promise<void>,
  nowUnixSeconds: number,
): Promise<PolicyCandidateProjection> {
  try {
    await verifyMode(paths.directory, PRIVATE_DIRECTORY_MODE, "directory");
    const state = await readState(paths.state);
    const times = timesFromState(state, request, digest, nowUnixSeconds);
    await requireExactPrivateInput(paths.schema, rendered.schema);
    await requireExactPrivateInput(paths.compilerConfig, rendered.compilerConfig);
    if (state.status === "COMPILED") {
      await requirePrivateOutput(paths.manifest);
      await requirePrivateOutput(paths.review);
      await requireArtifactDigest(paths.manifest, state.manifest_digest);
      await requireArtifactDigest(paths.review, state.review_digest);
      return readAndValidateProjection(
        paths.review,
        request,
        rendered.schema,
        rendered.ruleSummary,
        recordId,
        times,
      );
    }
    return advancePolicyCandidate(paths, request, rendered, recordId, config, runProcess, state);
  } catch (error) {
    if (error instanceof PolicyCandidateError) {
      throw error;
    }
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
}

async function advancePolicyCandidate(
  paths: CandidatePaths,
  request: PreparePolicyCandidateRequest,
  rendered: RenderedPolicySource,
  recordId: string,
  config: PolicyCandidateConfig,
  runProcess: (invocation: ProcessInvocation) => Promise<void>,
  state: CandidateState,
): Promise<PolicyCandidateProjection> {
  const times = stateTimes(state);
  const compilerOutputs = await privateOutputSet([paths.policy, paths.diagnostic]);
  if (compilerOutputs.length === 1) {
    await removePrivateOutputs(compilerOutputs);
  }

  if (compilerOutputs.length !== 2) {
    try {
      await runProcess(compilerInvocation(config, paths));
      await requirePrivateOutput(paths.policy);
      await requirePrivateOutput(paths.diagnostic);
    } catch {
      await recordFailure(paths.state, state, "COMPILER_FAILED");
      throw new PolicyCandidateError("COMPILER_FAILED");
    }
  }

  const inspectionOutputs = await privateOutputSet([paths.manifest, paths.review]);
  if (inspectionOutputs.length === 2) {
    try {
      const projection = await readAndValidateProjection(
        paths.review,
        request,
        rendered.schema,
        rendered.ruleSummary,
        recordId,
        times,
      );
      await writeState(paths.state, await compiledState(state, paths));
      return projection;
    } catch {
      await removePrivateOutputs(inspectionOutputs);
    }
  } else if (inspectionOutputs.length === 1) {
    await removePrivateOutputs(inspectionOutputs);
  }

  try {
    await runProcess(
      inspectorInvocation(config, paths, request, rendered.ruleSummary, recordId, times),
    );
    await requirePrivateOutput(paths.manifest);
    await requirePrivateOutput(paths.review);
    const projection = await readAndValidateProjection(
      paths.review,
      request,
      rendered.schema,
      rendered.ruleSummary,
      recordId,
      times,
    );
    await writeState(paths.state, await compiledState(state, paths));
    return projection;
  } catch (error) {
    await recordFailure(paths.state, state, "INSPECTION_FAILED");
    if (error instanceof PolicyCandidateError && error.code === "INSPECTION_FAILED") {
      throw error;
    }
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
}

function requestDigest(request: PreparePolicyCandidateRequest): string {
  return createHash("sha256").update(JSON.stringify(request)).digest("hex");
}

function candidateState(
  candidateId: string,
  requestDigestValue: string,
  times: CandidateTimes,
  status: CandidateState["status"],
  lastError: PolicyCandidateFailureCode | null,
): CandidateState {
  return {
    format_version: POLICY_CANDIDATE_STATE_FORMAT_VERSION,
    candidate_id: candidateId,
    request_digest: requestDigestValue,
    status,
    valid_from_unix_seconds: times.validFrom,
    valid_until_unix_seconds: times.validUntil,
    created_at_unix_seconds: times.createdAt,
    expires_at_unix_seconds: times.expiresAt,
    manifest_digest: null,
    review_digest: null,
    last_error: lastError,
  };
}

async function compiledState(
  state: CandidateState,
  paths: CandidatePaths,
): Promise<CandidateState> {
  return {
    ...state,
    status: "COMPILED",
    manifest_digest: await artifactDigest(paths.manifest),
    review_digest: await artifactDigest(paths.review),
    last_error: null,
  };
}

function compilerInvocation(
  config: PolicyCandidateConfig,
  paths: CandidatePaths,
): ProcessInvocation {
  return {
    executable: config.compilerExecutable,
    args: [
      "compile",
      "--schema",
      paths.schema,
      "--config",
      paths.compilerConfig,
      "--compiler-version",
      M5_COMPILER_VERSION,
      "--out",
      paths.policy,
      "--diagnostic-out",
      paths.diagnostic,
    ],
    cwd: paths.directory,
    timeoutMs: PROCESS_TIMEOUT_MS,
    maxOutputBytes: MAX_PROCESS_OUTPUT_BYTES,
  };
}

function inspectorInvocation(
  config: PolicyCandidateConfig,
  paths: CandidatePaths,
  request: PreparePolicyCandidateRequest,
  ruleSummary: string,
  recordId: string,
  times: CandidateTimes,
): ProcessInvocation {
  return {
    executable: config.adminExecutable,
    args: [
      "inspect-policy",
      "--policy",
      paths.policy,
      "--candidate-id",
      request.draft.candidateId,
      "--record-id",
      recordId,
      "--policy-name",
      request.draft.policyName,
      "--policy-version",
      request.draft.policyVersion.toString(),
      "--resource-type",
      request.draft.resourceType,
      "--permission",
      request.draft.permission,
      "--rule-summary",
      ruleSummary,
      "--expected-approval-epoch",
      request.expectedApprovalEpoch.toString(),
      "--valid-from",
      times.validFrom.toString(),
      "--valid-until",
      times.validUntil.toString(),
      "--created-at",
      times.createdAt.toString(),
      "--expires-at",
      times.expiresAt.toString(),
      "--output",
      paths.review,
      "--state-output",
      paths.manifest,
    ],
    cwd: paths.directory,
    timeoutMs: PROCESS_TIMEOUT_MS,
    maxOutputBytes: MAX_PROCESS_OUTPUT_BYTES,
  };
}

function candidatePaths(root: string, candidateId: string): CandidatePaths {
  const directory = join(root, candidateId);
  if (basename(directory) !== candidateId) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  return {
    directory,
    schema: join(directory, "schema.zed"),
    compilerConfig: join(directory, "compiler.json"),
    policy: join(directory, "policy.pb"),
    diagnostic: join(directory, "policy.json"),
    manifest: join(directory, "candidate.pb"),
    review: join(directory, "review.json"),
    state: join(directory, "state.json"),
  };
}

function candidateTimes(now: number, validForSeconds: number): CandidateTimes {
  if (!Number.isSafeInteger(now) || now <= 0) {
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  }
  const validUntil = now + validForSeconds;
  const expiresAt = now + CANDIDATE_LIFETIME_SECONDS;
  if (!Number.isSafeInteger(validUntil) || !Number.isSafeInteger(expiresAt)) {
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  }
  return { createdAt: now, expiresAt, validFrom: now, validUntil };
}

function deriveRecordId(draft: PolicyDraftV1): string {
  const digest = createHash("sha256")
    .update(draft.policyName)
    .update("\0")
    .update(draft.policyVersion.toString())
    .update("\0")
    .update(draft.candidateId)
    .digest("hex");
  return `policy-${digest.slice(0, 32)}`;
}

async function validateOwnerRoot(root: string): Promise<void> {
  const metadata = await lstat(root);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new PolicyCandidateError("INVALID_OWNER_CONFIGURATION");
  }
  await verifyOwner(metadata.uid);
  if ((metadata.mode & 0o777) !== PRIVATE_DIRECTORY_MODE) {
    throw new PolicyCandidateError("INVALID_OWNER_CONFIGURATION");
  }
}

async function validateExecutable(path: string): Promise<void> {
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink() || (metadata.mode & 0o111) === 0) {
    throw new PolicyCandidateError("INVALID_OWNER_CONFIGURATION");
  }
}

async function requirePrivateOutput(path: string): Promise<void> {
  await verifyMode(path, PRIVATE_FILE_MODE, "file");
}

async function verifyMode(
  path: string,
  expectedMode: number,
  expectedType: "directory" | "file",
): Promise<void> {
  const metadata = await lstat(path);
  const validType = expectedType === "file" ? metadata.isFile() : metadata.isDirectory();
  if (metadata.isSymbolicLink() || !validType || (metadata.mode & 0o777) !== expectedMode) {
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  }
  await verifyOwner(metadata.uid);
}

async function verifyOwner(owner: number): Promise<void> {
  if (typeof process.getuid === "function" && owner !== process.getuid()) {
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  }
}

async function writePrivateNew(path: string, value: string): Promise<void> {
  let handle: FileHandle | undefined;
  try {
    handle = await open(
      path,
      fsConstants.O_CREAT | fsConstants.O_EXCL | fsConstants.O_WRONLY,
      0o600,
    );
    await handle.writeFile(value, { encoding: "utf8" });
    await handle.sync();
  } catch (error) {
    throw asCandidateError(error, "CANDIDATE_STORE_UNAVAILABLE");
  } finally {
    await handle?.close();
  }
  await chmod(path, PRIVATE_FILE_MODE);
  await requirePrivateOutput(path);
}

async function writeState(path: string, state: CandidateState): Promise<void> {
  const temporaryPath = `${path}.tmp-${process.pid}`;
  try {
    await writePrivateNew(temporaryPath, `${JSON.stringify(state)}\n`);
    await rename(temporaryPath, path);
    await requirePrivateOutput(path);
  } catch (error) {
    await unlink(temporaryPath).catch(() => undefined);
    throw asCandidateError(error, "CANDIDATE_STORE_UNAVAILABLE");
  }
}

async function recordFailure(
  statePath: string,
  state: CandidateState,
  code: PolicyCandidateFailureCode,
): Promise<void> {
  await writeState(statePath, {
    ...state,
    status: "DRAFT",
    manifest_digest: null,
    review_digest: null,
    last_error: code,
  }).catch(() => undefined);
}

async function readState(path: string): Promise<CandidateState> {
  await requirePrivateOutput(path);
  const metadata = await stat(path);
  if (metadata.size <= 0 || metadata.size > MAX_SAFE_PROJECTION_BYTES) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  const input: unknown = JSON.parse(await readFile(path, "utf8"));
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  const state = input as Record<string, unknown>;
  const keys = [
    "format_version",
    "candidate_id",
    "request_digest",
    "status",
    "valid_from_unix_seconds",
    "valid_until_unix_seconds",
    "created_at_unix_seconds",
    "expires_at_unix_seconds",
    "manifest_digest",
    "review_digest",
    "last_error",
  ];
  if (Object.keys(state).length !== keys.length || keys.some((key) => !(key in state))) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  if (
    state.format_version !== POLICY_CANDIDATE_STATE_FORMAT_VERSION ||
    typeof state.candidate_id !== "string" ||
    typeof state.request_digest !== "string" ||
    !HEX_DIGEST.test(state.request_digest) ||
    (state.status !== "DRAFT" && state.status !== "COMPILED") ||
    !safePositiveInteger(state.valid_from_unix_seconds) ||
    !safePositiveInteger(state.valid_until_unix_seconds) ||
    !safePositiveInteger(state.created_at_unix_seconds) ||
    !safePositiveInteger(state.expires_at_unix_seconds) ||
    !validOptionalDigest(state.manifest_digest) ||
    !validOptionalDigest(state.review_digest) ||
    (state.status === "COMPILED" &&
      (state.manifest_digest === null || state.review_digest === null)) ||
    (state.status === "DRAFT" &&
      (state.manifest_digest !== null || state.review_digest !== null)) ||
    (state.status === "COMPILED" && state.last_error !== null) ||
    !validFailureCode(state.last_error)
  ) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  return state as unknown as CandidateState;
}

function timesFromState(
  state: CandidateState,
  request: PreparePolicyCandidateRequest,
  digest: string,
  nowUnixSeconds: number,
): CandidateTimes {
  const times = stateTimes(state);
  if (
    state.candidate_id !== request.draft.candidateId ||
    state.request_digest !== digest ||
    times.createdAt !== times.validFrom ||
    times.validUntil - times.validFrom !== request.validForSeconds ||
    times.expiresAt - times.createdAt !== CANDIDATE_LIFETIME_SECONDS
  ) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
  if (!Number.isSafeInteger(nowUnixSeconds) || nowUnixSeconds >= times.expiresAt) {
    throw new PolicyCandidateError("CANDIDATE_EXPIRED");
  }
  return times;
}

function stateTimes(state: CandidateState): CandidateTimes {
  return {
    createdAt: state.created_at_unix_seconds,
    expiresAt: state.expires_at_unix_seconds,
    validFrom: state.valid_from_unix_seconds,
    validUntil: state.valid_until_unix_seconds,
  };
}

async function requireExactPrivateInput(path: string, expected: string): Promise<void> {
  await requirePrivateOutput(path);
  const metadata = await stat(path);
  if (
    metadata.size !== Buffer.byteLength(expected) ||
    (await readFile(path, "utf8")) !== expected
  ) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
}

async function privateOutputSet(paths: readonly string[]): Promise<string[]> {
  const present: string[] = [];
  for (const path of paths) {
    try {
      await requirePrivateOutput(path);
      present.push(path);
    } catch (error) {
      if (!isNotFound(error)) {
        throw error;
      }
    }
  }
  return present;
}

async function removePrivateOutputs(paths: readonly string[]): Promise<void> {
  for (const path of paths) {
    await requirePrivateOutput(path);
    await unlink(path);
  }
}

async function requireArtifactDigest(path: string, expected: string | null): Promise<void> {
  if (expected === null || (await artifactDigest(path)) !== expected) {
    throw new PolicyCandidateError("CANDIDATE_CONFLICT");
  }
}

async function artifactDigest(path: string): Promise<string> {
  await requirePrivateOutput(path);
  const metadata = await stat(path);
  if (metadata.size <= 0 || metadata.size > MAX_PRIVATE_ARTIFACT_BYTES) {
    throw new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE");
  }
  return createHash("sha256")
    .update(await readFile(path))
    .digest("hex");
}

function safePositiveInteger(input: unknown): boolean {
  return Number.isSafeInteger(input) && (input as number) > 0;
}

function validOptionalDigest(input: unknown): boolean {
  return input === null || (typeof input === "string" && HEX_DIGEST.test(input));
}

function validFailureCode(input: unknown): boolean {
  return (
    input === null ||
    input === "COMPILER_FAILED" ||
    input === "INSPECTION_FAILED" ||
    input === "CANDIDATE_STORE_UNAVAILABLE"
  );
}

async function readAndValidateProjection(
  path: string,
  request: PreparePolicyCandidateRequest,
  schema: string,
  ruleSummary: string,
  recordId: string,
  times: CandidateTimes,
): Promise<PolicyCandidateProjection> {
  const projection = await readStrictProjection(path);
  const sourceSchemaDigest = createHash("sha256").update(schema).digest("hex");
  const expected = {
    candidate_id: request.draft.candidateId,
    compiler_version: M5_COMPILER_VERSION,
    record_id: recordId,
    policy_name: request.draft.policyName,
    policy_version: request.draft.policyVersion.toString(),
    resource_type: request.draft.resourceType,
    permission: request.draft.permission,
    rule_summary: ruleSummary,
    source_schema_digest: sourceSchemaDigest,
    expected_approval_epoch: request.expectedApprovalEpoch.toString(),
    valid_from: timestamp(times.validFrom),
    valid_until: timestamp(times.validUntil),
    status: "COMPILED",
    created_at: timestamp(times.createdAt),
    expires_at: timestamp(times.expiresAt),
  } as const;
  for (const [key, expectedValue] of Object.entries(expected)) {
    if (
      !projectionValueMatches(projection[key as keyof PolicyCandidateProjection], expectedValue)
    ) {
      throw new PolicyCandidateError("INSPECTION_FAILED");
    }
  }
  if (!HEX_DIGEST.test(projection.policy_hash)) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
  return projection;
}

async function readStrictProjection(path: string): Promise<PolicyCandidateProjection> {
  await requirePrivateOutput(path);
  const metadata = await stat(path);
  if (metadata.size <= 0 || metadata.size > MAX_SAFE_PROJECTION_BYTES) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
  const value: unknown = JSON.parse(await readFile(path, "utf8"));
  return strictProjection(value);
}

function projectionValueMatches(actual: unknown, expected: unknown): boolean {
  if (typeof expected === "string") {
    return actual === expected;
  }
  if (typeof expected !== "object" || expected === null || Array.isArray(expected)) {
    return false;
  }
  if (typeof actual !== "object" || actual === null || Array.isArray(actual)) {
    return false;
  }
  const expectedTimestamp = expected as Record<string, unknown>;
  const actualTimestamp = actual as Record<string, unknown>;
  return (
    actualTimestamp.seconds === expectedTimestamp.seconds &&
    actualTimestamp.nanos === expectedTimestamp.nanos
  );
}

function strictProjection(input: unknown): PolicyCandidateProjection {
  const keys = [
    "candidate_id",
    "compiler_version",
    "record_id",
    "policy_name",
    "policy_version",
    "resource_type",
    "permission",
    "rule_summary",
    "source_schema_digest",
    "policy_hash",
    "expected_approval_epoch",
    "valid_from",
    "valid_until",
    "status",
    "created_at",
    "expires_at",
  ] as const;
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
  const object = input as Record<string, unknown>;
  if (Object.keys(object).length !== keys.length || keys.some((key) => !(key in object))) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
  for (const key of keys) {
    if (["valid_from", "valid_until", "created_at", "expires_at"].includes(key)) {
      validateTimestamp(object[key]);
    } else if (typeof object[key] !== "string") {
      throw new PolicyCandidateError("INSPECTION_FAILED");
    }
  }
  if (!HEX_DIGEST.test(object.source_schema_digest as string)) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
  return object as unknown as PolicyCandidateProjection;
}

function validateTimestamp(input: unknown): void {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
  const timestampValue = input as Record<string, unknown>;
  if (
    Object.keys(timestampValue).length !== 2 ||
    typeof timestampValue.seconds !== "string" ||
    !/^-?[0-9]+$/.test(timestampValue.seconds) ||
    timestampValue.nanos !== 0
  ) {
    throw new PolicyCandidateError("INSPECTION_FAILED");
  }
}

function timestamp(seconds: number): { readonly seconds: string; readonly nanos: 0 } {
  return { seconds: seconds.toString(), nanos: 0 };
}

function systemNowUnixSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

function isConflict(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { readonly code?: unknown }).code === "EEXIST"
  );
}

function isNotFound(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { readonly code?: unknown }).code === "ENOENT"
  );
}

function asCandidateError(
  error: unknown,
  fallback: PolicyCandidateFailureCode,
): PolicyCandidateError {
  return error instanceof PolicyCandidateError ? error : new PolicyCandidateError(fallback);
}
