import { constants } from "node:fs";
import { lstat, open } from "node:fs/promises";
import path from "node:path";
import { AuthorizationRejectedError, ProtectedQueryInfrastructureError } from "./errors";
import type { VerifiedCapabilityClaims } from "./types";

const MAX_BYTES = 4096;
const DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const RESOURCE_TYPE = /^[A-Za-z][A-Za-z0-9_]{0,62}$/;
const FIELDS = [
  "format_version",
  "policy_hash",
  "resource_type",
  "resource_scope",
  "dataset_id",
  "dataset_revision",
  "dataset_checksum",
  "action",
];

export interface DatasetBinding {
  readonly format_version: 1;
  readonly policy_hash: string;
  readonly resource_type: string;
  readonly resource_scope: string;
  readonly dataset_id: string;
  readonly dataset_revision: string;
  readonly dataset_checksum: string;
  readonly action: "query";
}

export type DatasetBindingResolver = (
  claims: VerifiedCapabilityClaims,
  datasetId: string,
) => Promise<DatasetBinding | undefined>;

export function parseDatasetBinding(input: unknown): DatasetBinding {
  if (typeof input !== "object" || input === null || Array.isArray(input)) throw unavailable();
  const value = input as Record<string, unknown>;
  if (
    Object.keys(value).length !== FIELDS.length ||
    Object.keys(value).some((key) => !FIELDS.includes(key)) ||
    value.format_version !== 1 ||
    value.action !== "query" ||
    !matches(value.policy_hash, DIGEST) ||
    !matches(value.resource_type, RESOURCE_TYPE) ||
    !matches(value.resource_scope, IDENTIFIER) ||
    !matches(value.dataset_id, IDENTIFIER) ||
    !matches(value.dataset_checksum, DIGEST) ||
    !matches(value.dataset_revision, /^[1-9][0-9]{0,15}$/) ||
    !Number.isSafeInteger(Number(value.dataset_revision))
  )
    throw unavailable();
  return value as unknown as DatasetBinding;
}

export class FileDatasetBindingStore {
  readonly #root: string;
  public constructor(root: string) {
    if (
      !path.isAbsolute(root) ||
      root === "/" ||
      root.includes("\0") ||
      root.split(path.sep).some((part) => part === "." || part === "..")
    )
      throw unavailable();
    this.#root = root;
  }

  public async resolve(
    claims: VerifiedCapabilityClaims,
    datasetId: string,
  ): Promise<DatasetBinding> {
    if (
      !DIGEST.test(claims.policyHashHex) ||
      !RESOURCE_TYPE.test(claims.resourceType) ||
      !IDENTIFIER.test(claims.resourceScope)
    )
      throw new AuthorizationRejectedError();
    try {
      // The gateway has a read-only mount. Every lookup checks the exact owner record,
      // including cache hits. Namespaced type prefixes never authorize a fallback.
      const root = await lstat(this.#root);
      if (!root.isDirectory() || (root.mode & 0o7777) !== 0o700) throw unavailable();
      let directory = this.#root;
      for (const part of [claims.policyHashHex, claims.resourceType]) {
        directory = path.join(directory, part);
        let metadata: Awaited<ReturnType<typeof lstat>>;
        try {
          metadata = await lstat(directory);
        } catch (error) {
          throw missingOrUnavailable(error);
        }
        if (
          !metadata.isDirectory() ||
          metadata.uid !== root.uid ||
          (metadata.mode & 0o7777) !== 0o700
        )
          throw unavailable();
      }
      const filename = path.join(directory, `${claims.resourceScope}.json`);
      let handle: Awaited<ReturnType<typeof open>>;
      try {
        handle = await open(
          filename,
          constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
        );
      } catch (error) {
        throw missingOrUnavailable(error);
      }
      let bytes: Buffer;
      try {
        const metadata = await handle.stat();
        if (
          !metadata.isFile() ||
          metadata.nlink !== 1 ||
          metadata.uid !== root.uid ||
          (metadata.mode & 0o7777) !== 0o600 ||
          metadata.size > MAX_BYTES
        )
          throw unavailable();
        bytes = Buffer.alloc(MAX_BYTES + 1);
        const { bytesRead } = await handle.read(bytes, 0, bytes.length, 0);
        if (bytesRead > MAX_BYTES) throw unavailable();
        bytes = bytes.subarray(0, bytesRead);
      } finally {
        await handle.close();
      }
      const binding = parseDatasetBinding(JSON.parse(bytes.toString("utf8")));
      if (
        binding.policy_hash !== claims.policyHashHex ||
        binding.resource_type !== claims.resourceType ||
        binding.resource_scope !== claims.resourceScope
      )
        throw unavailable();
      if (binding.dataset_id !== datasetId || binding.action !== claims.action)
        throw new AuthorizationRejectedError();
      return binding;
    } catch (error) {
      if (error instanceof AuthorizationRejectedError) throw error;
      throw unavailable();
    }
  }
}

export function datasetBindingResolver(
  environment: Readonly<Record<string, string | undefined>>,
): DatasetBindingResolver {
  switch (environment.RAZIEL_DATASET_BINDING_MODE ?? "legacy") {
    case "legacy":
      return legacyDatasetBinding;
    case "required": {
      const root = environment.RAZIEL_DATASET_BINDING_ROOT;
      if (!root) throw unavailable();
      const store = new FileDatasetBindingStore(root);
      return (claims, datasetId) => store.resolve(claims, datasetId);
    }
    default:
      throw unavailable();
  }
}

export async function legacyDatasetBinding(
  claims: VerifiedCapabilityClaims,
  datasetId: string,
): Promise<undefined> {
  if (claims.resourceType !== "dataset" || claims.resourceScope !== datasetId)
    throw new AuthorizationRejectedError();
  return undefined;
}

function matches(value: unknown, pattern: RegExp): value is string {
  return typeof value === "string" && pattern.test(value);
}
function unavailable(): ProtectedQueryInfrastructureError {
  return new ProtectedQueryInfrastructureError();
}
function missingOrUnavailable(error: unknown): Error {
  return error instanceof Error && "code" in error && error.code === "ENOENT"
    ? new AuthorizationRejectedError()
    : unavailable();
}
