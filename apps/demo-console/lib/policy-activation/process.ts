import { spawn } from "node:child_process";

import {
  type PolicyActivationConfig,
  PolicyActivationError,
  type PolicyActivationProjection,
} from "./types.ts";
import { validateActivationIdentity, validateActivationProjection } from "./validation.ts";

const PROCESS_TIMEOUT_MS = 20_000;
const MAX_OUTPUT_BYTES = 64 * 1024;

export async function activatePolicyCandidate(
  candidateIdInput: unknown,
  approvedByInput: unknown,
  config: PolicyActivationConfig,
): Promise<PolicyActivationProjection> {
  const { candidateId, approvedBy } = validateActivationIdentity(candidateIdInput, approvedByInput);
  const output = await runActivationClient(config, candidateId, approvedBy);
  let parsed: unknown;
  try {
    parsed = JSON.parse(output) as unknown;
  } catch {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  return validateActivationProjection(parsed);
}

async function runActivationClient(
  config: PolicyActivationConfig,
  candidateId: string,
  approvedBy: string,
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let stdout = Buffer.alloc(0);
    let stderr = Buffer.alloc(0);
    let outputBytes = 0;
    let failed = false;
    let settled = false;
    const child = spawn(
      config.executable,
      ["--candidate-id", candidateId, "--approved-by", approvedBy],
      {
        env: {
          NODE_ENV: process.env.NODE_ENV ?? "production",
          PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
          RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: config.socketPath,
        },
        shell: false,
        stdio: ["ignore", "pipe", "pipe"] as const,
      },
    );
    const count = (chunk: Buffer): boolean => {
      outputBytes += chunk.byteLength;
      if (outputBytes > MAX_OUTPUT_BYTES && !failed) {
        failed = true;
        child.kill("SIGKILL");
        return false;
      }
      return true;
    };
    child.stdout.on("data", (chunk: Buffer) => {
      if (count(chunk)) {
        stdout = Buffer.concat([stdout, chunk]);
      }
    });
    child.stderr.on("data", (chunk: Buffer) => {
      if (count(chunk)) {
        stderr = Buffer.concat([stderr, chunk]);
      }
    });
    child.on("error", () => {
      if (!settled) {
        settled = true;
        reject(new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE"));
      }
    });
    const timer = setTimeout(() => {
      failed = true;
      child.kill("SIGKILL");
    }, PROCESS_TIMEOUT_MS);
    timer.unref();
    child.on("close", (code) => {
      clearTimeout(timer);
      if (settled) {
        return;
      }
      settled = true;
      if (code !== 0 || failed || stdout.byteLength === 0) {
        reject(
          new PolicyActivationError(
            failed ? "POLICY_ACTIVATION_UNAVAILABLE" : activationFailureCode(stderr),
          ),
        );
        return;
      }
      resolve(stdout.toString("utf8"));
    });
  });
}

function activationFailureCode(
  stderr: Buffer,
): "POLICY_ACTIVATION_CONFLICT" | "POLICY_ACTIVATION_REJECTED" | "POLICY_ACTIVATION_UNAVAILABLE" {
  switch (stderr.toString("utf8").trim()) {
    case "raziel-policy-activate: policy activation conflicts with trusted state":
      return "POLICY_ACTIVATION_CONFLICT";
    case "raziel-policy-activate: policy candidate was rejected":
      return "POLICY_ACTIVATION_REJECTED";
    default:
      return "POLICY_ACTIVATION_UNAVAILABLE";
  }
}
