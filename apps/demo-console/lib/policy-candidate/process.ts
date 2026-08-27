import { spawn } from "node:child_process";

import { PolicyCandidateError, type ProcessInvocation } from "./types.ts";

export async function runDirectProcess(invocation: ProcessInvocation): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let outputBytes = 0;
    let failure: "output" | "timeout" | undefined;
    let settled = false;
    const environment: NodeJS.ProcessEnv = {
      NODE_ENV: process.env.NODE_ENV ?? "production",
      PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
    };
    const child = spawn(invocation.executable, [...invocation.args], {
      cwd: invocation.cwd,
      env: environment,
      shell: false,
      stdio: ["ignore", "pipe", "pipe"] as const,
    });

    const fail = (): void => {
      if (!settled) {
        settled = true;
        reject(new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE"));
      }
    };
    const countOutput = (chunk: Buffer): void => {
      outputBytes += chunk.byteLength;
      if (outputBytes > invocation.maxOutputBytes && failure === undefined) {
        failure = "output";
        child.kill("SIGKILL");
      }
    };
    child.stdout.on("data", countOutput);
    child.stderr.on("data", countOutput);
    child.on("error", fail);

    const timer = setTimeout(() => {
      if (failure === undefined) {
        failure = "timeout";
        child.kill("SIGKILL");
      }
    }, invocation.timeoutMs);
    timer.unref();

    child.on("close", (code) => {
      clearTimeout(timer);
      if (settled) {
        return;
      }
      settled = true;
      if (code === 0 && failure === undefined) {
        resolve();
      } else {
        reject(new PolicyCandidateError("CANDIDATE_STORE_UNAVAILABLE"));
      }
    });
  });
}
