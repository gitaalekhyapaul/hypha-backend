import { chmod, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  datasetBindingResolver,
  FileDatasetBindingStore,
  parseDatasetBinding,
} from "../src/protected-request/dataset-binding.ts";
import {
  AuthorizationRejectedError,
  ProtectedQueryInfrastructureError,
} from "../src/protected-request/errors.ts";
import type { VerifiedCapabilityClaims } from "../src/protected-request/types.ts";

const directories: string[] = [];
afterEach(async () => {
  for (const directory of directories.splice(0))
    await rm(directory, { recursive: true, force: true });
});

async function fixture() {
  const binding = parseDatasetBinding(
    JSON.parse(
      await readFile(
        new URL("../../../tests/vectors/dataset-binding-v1.json", import.meta.url),
        "utf8",
      ),
    ),
  );
  const root = await mkdtemp(path.join(tmpdir(), "raziel-bindings-"));
  directories.push(root);
  const parent = path.join(root, binding.policy_hash, binding.resource_type);
  await mkdir(parent, { recursive: true, mode: 0o700 });
  const filename = path.join(parent, `${binding.resource_scope}.json`);
  await writeFile(filename, JSON.stringify(binding), { mode: 0o600 });
  const claims = {
    policyHashHex: binding.policy_hash,
    resourceType: binding.resource_type,
    resourceScope: binding.resource_scope,
    action: "query",
  } as VerifiedCapabilityClaims;
  return { root, filename, binding, claims, store: new FileDatasetBindingStore(root) };
}

describe("immutable dataset routing", () => {
  it("reads the shared Rust/TypeScript record and requires exact keys and request target", async () => {
    const { store, claims, binding } = await fixture();
    expect(await store.resolve(claims, binding.dataset_id)).toEqual(binding);
    for (const altered of [
      { ...claims, policyHashHex: "55".repeat(32) },
      { ...claims, resourceType: `${claims.resourceType}_other` },
      { ...claims, resourceScope: "other" },
      { ...claims, action: "export" },
      { ...claims, resourceScope: "../escape" },
    ])
      await expect(store.resolve(altered, binding.dataset_id)).rejects.toBeInstanceOf(
        AuthorizationRejectedError,
      );
    await expect(store.resolve(claims, "bank-ledger")).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );
  });

  it("rejects unknown fields, path swaps, noncanonical revisions and oversized records", async () => {
    const { store, claims, binding, filename } = await fixture();
    for (const value of [
      { ...binding, private_context: {} },
      { ...binding, policy_hash: "55".repeat(32) },
      { ...binding, dataset_revision: "01" },
      { ...binding, dataset_revision: "9007199254740992" },
      { ...binding, dataset_revision: 1 },
    ]) {
      await writeFile(filename, JSON.stringify(value));
      await expect(store.resolve(claims, binding.dataset_id)).rejects.toBeInstanceOf(
        ProtectedQueryInfrastructureError,
      );
    }
    await writeFile(filename, " ".repeat(4097));
    await expect(store.resolve(claims, binding.dataset_id)).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );
  });

  it("rejects unsafe permissions and symlinks", async () => {
    const { store, claims, binding, filename, root } = await fixture();
    await chmod(filename, 0o644);
    await expect(store.resolve(claims, binding.dataset_id)).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );
    await rm(filename);
    await writeFile(path.join(root, "outside.json"), JSON.stringify(binding), { mode: 0o600 });
    await symlink(path.join(root, "outside.json"), filename);
    await expect(store.resolve(claims, binding.dataset_id)).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );
  });

  it("required mode never falls back to the M5 dataset shortcut", async () => {
    const { root, claims, binding } = await fixture();
    const resolve = datasetBindingResolver({
      RAZIEL_DATASET_BINDING_MODE: "required",
      RAZIEL_DATASET_BINDING_ROOT: root,
    });
    await expect(
      resolve(
        { ...claims, resourceType: "dataset", resourceScope: binding.dataset_id },
        binding.dataset_id,
      ),
    ).rejects.toBeInstanceOf(AuthorizationRejectedError);
    expect(() => datasetBindingResolver({ RAZIEL_DATASET_BINDING_MODE: "required" })).toThrow(
      ProtectedQueryInfrastructureError,
    );
    expect(() => datasetBindingResolver({ RAZIEL_DATASET_BINDING_MODE: "typo" })).toThrow(
      ProtectedQueryInfrastructureError,
    );
    const legacy = datasetBindingResolver({});
    await expect(
      legacy(
        { ...claims, resourceType: "dataset", resourceScope: binding.dataset_id },
        binding.dataset_id,
      ),
    ).resolves.toBeUndefined();
    await expect(legacy(claims, binding.dataset_id)).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );
  });
});
