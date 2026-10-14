import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fixture = mkdtempSync(resolve(tmpdir(), "raziel-proof-preparation-"));
const log = resolve(fixture, "calls.jsonl");
const put = (path, content, mode = 0o644) => {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content, { mode });
};
const mock = `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
fs.appendFileSync(process.env.CALL_LOG, JSON.stringify(args) + '\\n');
if (process.env.FAIL_CALL && args.join(' ').includes(process.env.FAIL_CALL)) process.exit(42);
`;
const env = {
  ...process.env,
  CALL_LOG: log,
  PATH: `${fixture}/bin:${process.env.PATH}`,
  CARGO_HOME: `${fixture}/cargo`,
  GOMODCACHE: `${fixture}/cache`,
  RAZIEL_SP1_CRATE_VERSION: "6.3.1",
};
const calls = () => readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
const run = (script, args = [], extra = {}, success = true) => {
  writeFileSync(log, "");
  const result = spawnSync(script, args, { env: { ...env, ...extra }, encoding: "utf8" });
  assert.equal(result.status === 0, success, result.stderr);
  if (!success) assert.ok(!result.stdout.includes("=pass"));
  return result;
};

try {
  const guest = readFileSync(resolve(root, "tools/authorization-guest/approved-v1.elf"));
  const guestDigest = createHash("sha256").update(guest).digest("hex");
  assert.equal(guest.length, 386088);
  assert.equal(
    readFileSync(resolve(root, "tools/authorization-guest/SHA256SUMS"), "utf8"),
    `${guestDigest}  approved-v1.elf\n`,
  );
  put(`${fixture}/bin/go`, mock, 0o755);
  put(`${fixture}/bin/docker`, mock, 0o755);
  const crate = `${fixture}/cargo/registry/src/test/sp1-recursion-gnark-ffi-6.3.1/go`;
  const manifest = "module example.invalid/cache-test\n\ngo 1.24.0\n";
  const sums = "example.invalid/module v1.0.0 h1:fixture-checksum\n";
  put(`${crate}/go.mod`, manifest);
  put(`${crate}/go.sum`, sums);
  const resetCache = () => {
    put(`${fixture}/cache/raziel-sp1-gnark.mod`, manifest);
    put(`${fixture}/cache/raziel-sp1-gnark.sum`, sums);
  };
  const check = resolve(root, "scripts/prepare-proof-sandbox-dependencies");
  resetCache();
  run(check, ["--check"]);
  assert.deepEqual(calls(), [
    ["mod", "download", "all"],
    ["mod", "verify"],
  ]);
  assert.equal(readFileSync(`${fixture}/cache/raziel-sp1-gnark.mod`, "utf8"), manifest);
  put(`${fixture}/cache/raziel-sp1-gnark.mod`, `${manifest}require example.invalid/other v1.0.0\n`);
  run(check, ["--check"], {}, false);
  assert.deepEqual(calls(), []);
  resetCache();
  put(`${fixture}/cache/raziel-sp1-gnark.sum`, sums.replace("fixture-checksum", "changed"));
  run(check, ["--check"], {}, false);
  resetCache();
  put(
    `${fixture}/cache/raziel-sp1-gnark.sum`,
    `${sums}example.invalid/module v1.0.0 h1:conflicting-checksum\n`,
  );
  run(check, ["--check"], {}, false);
  resetCache();
  run(check, ["--check"], { FAIL_CALL: "download all" }, false);
  assert.equal(calls().length, 1);
  run(check, ["--check"], { FAIL_CALL: "mod verify" }, false);
  run(check, ["--unknown"], {}, false);
  rmSync(`${fixture}/cache/raziel-sp1-gnark.mod`);
  run(check, ["--check"], {}, false);
  resetCache();
  run(check);
  assert.equal(readFileSync(`${crate}/go.mod`, "utf8"), manifest);
  assert.equal(readFileSync(`${crate}/go.sum`, "utf8"), sums);

  put(`${fixture}/scripts/compose-m6`, mock, 0o755);
  put(`${fixture}/scripts/restore-m6-proof-go-cache`, mock, 0o755);
  copyFileSync(
    resolve(root, "scripts/prepare-m6-proof-runtime"),
    `${fixture}/scripts/prepare-m6-proof-runtime`,
  );
  const prepare = `${fixture}/scripts/prepare-m6-proof-runtime`;
  run(prepare);
  assert.deepEqual(calls(), [
    ["run", "--rm", "--no-deps", "m6-toolchain", "cargo", "fetch", "--locked"],
    ["run", "--rm", "--no-deps", "m6-toolchain", "./scripts/prepare-sp1-native-dependencies"],
    ["run", "--rm", "--no-deps", "m6-proof-dependency-fetch"],
    ["--profile", "apps", "run", "--rm", "--no-deps", "m6-proof-build"],
  ]);
  const source = "raziel_razield-proof-go-mod-cache";
  run(prepare, ["--reuse-go-cache", source]);
  assert.deepEqual(calls()[2], [source]);
  run(prepare, [], { FAIL_CALL: "m6-proof-dependency-fetch" }, false);
  assert.equal(calls().length, 3);
  run(prepare, ["--reuse-go-cache", source], { FAIL_CALL: source }, false);
  assert.equal(calls().length, 3);
  run(prepare, ["--reuse-go-cache"], {}, false);
  assert.deepEqual(calls(), []);
  run(prepare, ["--reuse-go-cache", ""], {}, false);
  assert.deepEqual(calls(), []);

  const restore = resolve(root, "scripts/restore-m6-proof-go-cache");
  run(restore, [source]);
  const recorded = calls();
  assert.deepEqual(recorded[0], ["volume", "inspect", source, "raziel-m6-live_cargo-registry"]);
  const command = recorded.find((args) => args[0] === "run");
  assert.ok(command.includes(`type=volume,source=${source},target=/cache-source,readonly`));
  assert.ok(
    command.includes(
      "type=volume,source=raziel-m6-live_razield-proof-go-mod-cache,target=/cache-target",
    ),
  );
  assert.ok(command.includes("--read-only"));
  assert.equal(command[command.indexOf("--network") + 1], "none");
  assert.ok(command.includes("--pull=never"));
  assert.ok(!command.join(" ").includes("/var/lib/raziel"));
  assert.ok(!command.join(" ").includes("docker.sock"));
  run(restore, [source], { FAIL_CALL: "volume inspect" }, false);
  assert.equal(calls().length, 1);
  for (const invalid of [
    "raziel-m6-live_razield-proof-go-mod-cache",
    "private-snapshots",
    "--help",
  ]) {
    run(restore, [invalid], {}, false);
    assert.deepEqual(calls(), []);
  }
  console.log("m6_proof_preparation_checks=pass runtime=not-started");
} finally {
  rmSync(fixture, { recursive: true, force: true });
}
