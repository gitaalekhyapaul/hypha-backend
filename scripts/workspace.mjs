import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const action = process.argv[2];
const allowedActions = new Set([
  "build",
  "format",
  "format:check",
  "generate",
  "generate:check",
  "lint",
  "test",
  "test:e2e",
  "typecheck",
]);

if (!allowedActions.has(action)) {
  process.stderr.write(`unsupported workspace action: ${action ?? "<missing>"}\n`);
  process.exit(2);
}

function run(command, args, options = {}) {
  process.stdout.write(`\n> ${command} ${args.join(" ")}\n`);
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
  });

  if (result.error) {
    process.stderr.write(`${command}: ${result.error.message}\n`);
    process.exit(1);
  }

  if (options.capture && result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (options.capture && result.stderr) {
    process.stderr.write(result.stderr);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }

  return result.stdout ?? "";
}

function goFiles() {
  const output = run("git", ["ls-files", "--cached", "--others", "--exclude-standard", "*.go"], {
    capture: true,
  });
  return output.split("\n").filter(Boolean);
}

const commands = {
  build: () => {
    run("pnpm", ["-r", "--if-present", "build"]);
    run("cargo", ["build", "--workspace", "--exclude", "raziel-authorization-guest"]);
    run("cargo", ["prove", "build", "--locked", "--packages", "raziel-authorization-guest"]);
    run("go", ["build", "./..."], { cwd: "tools/spicedb-ir-compiler" });
  },
  format: () => {
    run("pnpm", [
      "exec",
      "biome",
      "check",
      "--write",
      "--formatter-enabled=true",
      "--linter-enabled=false",
      ".",
    ]);
    run("cargo", ["fmt", "--all"]);
    const files = goFiles();
    if (files.length > 0) run("gofmt", ["-w", ...files]);
  },
  "format:check": () => {
    run("pnpm", [
      "exec",
      "biome",
      "check",
      "--formatter-enabled=true",
      "--linter-enabled=false",
      ".",
    ]);
    run("cargo", ["fmt", "--all", "--", "--check"]);
    const files = goFiles();
    if (files.length > 0) {
      const diff = run("gofmt", ["-d", ...files], { capture: true });
      if (diff.trim()) process.exit(1);
    }
  },
  generate: () => run("buf", ["generate"]),
  "generate:check": () => {
    const root = mkdtempSync(join(tmpdir(), "raziel-generation-"));
    const first = join(root, "first");
    const second = join(root, "second");

    try {
      run("buf", ["generate", "--output", first]);
      run("buf", ["generate", "--output", second]);
      run("diff", ["-ru", first, second]);
    } finally {
      rmSync(root, { force: true, recursive: true });
    }
  },
  lint: () => {
    run("pnpm", ["exec", "biome", "lint", "."]);
    run("cargo", [
      "clippy",
      "--workspace",
      "--all-targets",
      "--exclude",
      "raziel-authorization-guest",
      "--",
      "-D",
      "warnings",
    ]);
    run("go", ["vet", "./..."], { cwd: "tools/spicedb-ir-compiler" });
    run("buf", ["lint"]);
  },
  test: () => {
    run("pnpm", ["-r", "--if-present", "test"]);
    run("cargo", ["test", "--workspace", "--exclude", "raziel-authorization-guest"]);
    run("go", ["test", "./..."], { cwd: "tools/spicedb-ir-compiler" });
  },
  "test:e2e": () => run("pnpm", ["-r", "--if-present", "test:e2e"]),
  typecheck: () => {
    run("pnpm", ["-r", "--if-present", "typecheck"]);
    run("cargo", [
      "check",
      "--workspace",
      "--all-targets",
      "--exclude",
      "raziel-authorization-guest",
    ]);
    run("go", ["test", "-run", "^$", "./..."], {
      cwd: "tools/spicedb-ir-compiler",
    });
  },
};

commands[action]();
