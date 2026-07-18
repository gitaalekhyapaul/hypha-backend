import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const versions = Object.fromEntries(
  readFileSync(new URL("../tools/toolchain-versions.env", import.meta.url), "utf8")
    .split("\n")
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const separator = line.indexOf("=");
      return [line.slice(0, separator), line.slice(separator + 1)];
    }),
);

const failures = [];

function checkVersion(name, command, args, expected) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();

  if (result.error) {
    failures.push(`${name}: ${result.error.message}`);
    return;
  }

  if (result.status !== 0) {
    failures.push(`${name}: command failed (${output || `exit ${result.status}`})`);
    return;
  }

  if (!output.includes(expected)) {
    failures.push(`${name}: expected ${expected}, observed ${output}`);
    return;
  }

  process.stdout.write(`${name}: ${output}\n`);
}

if (process.versions.node !== versions.NODE_VERSION) {
  failures.push(`Node.js: expected ${versions.NODE_VERSION}, observed ${process.versions.node}`);
} else {
  process.stdout.write(`Node.js: ${process.versions.node}\n`);
}

checkVersion("pnpm", "pnpm", ["--version"], versions.PNPM_VERSION);
checkVersion("rustc", "rustc", ["--version"], versions.RUST_VERSION);
checkVersion("cargo", "cargo", ["--version"], versions.RUST_VERSION);
checkVersion("Go", "go", ["version"], `go${versions.GO_VERSION}`);
checkVersion("Buf", "buf", ["--version"], versions.BUF_VERSION);
checkVersion("SP1", "cargo", ["prove", "--version"], versions.SP1_CARGO_PROVE_COMMIT);

if (!new Set(["arm64", "x64"]).has(process.arch)) {
  failures.push(`Architecture: unsupported Node architecture ${process.arch}`);
} else {
  process.stdout.write(`Architecture: ${process.arch}\n`);
}

if (failures.length > 0) {
  process.stderr.write(`${failures.join("\n")}\n`);
  process.exit(1);
}
