#!/usr/bin/env node

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import readline from "node:readline";

const markerPath = "target/raziel-sp1-plonk-test-executable";
const cargo = spawn(
  "cargo",
  [
    "test",
    "--locked",
    "--offline",
    "--release",
    "-p",
    "raziel-prover",
    "--test",
    "prove_guest",
    "--no-run",
    "--message-format=json",
  ],
  { stdio: ["ignore", "pipe", "inherit"] },
);

let executable;
const lines = readline.createInterface({ input: cargo.stdout });
for await (const line of lines) {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    continue;
  }
  if (
    message.reason === "compiler-artifact" &&
    message.target?.name === "prove_guest" &&
    message.profile?.test &&
    message.executable
  ) {
    executable = message.executable;
  }
  if (message.reason === "compiler-message" && message.message?.rendered) {
    process.stderr.write(message.message.rendered);
  }
}

const status = await new Promise((resolve) => cargo.on("close", resolve));
if (status !== 0) {
  process.exit(status ?? 1);
}
if (!executable?.startsWith("/workspace/target/release/deps/prove_guest-")) {
  console.error("Cargo did not report the expected prove_guest test executable");
  process.exit(1);
}

await fs.writeFile(markerPath, `${executable}\n`);
console.log(`prepared_test_executable=${executable}`);
