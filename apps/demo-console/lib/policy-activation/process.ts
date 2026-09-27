import { spawn } from "node:child_process";
import { parseApprovedPolicyCatalogue } from "../policy-version/catalogue-types.ts";
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
  const output = await runActivationClient(config, [
    "--candidate-id",
    candidateId,
    "--approved-by",
    approvedBy,
  ]);
  let parsed: unknown;
  try {
    parsed = JSON.parse(output) as unknown;
  } catch {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
  return validateActivationProjection(parsed);
}

export async function listApprovedPolicyVersions(config: PolicyActivationConfig) {
  if (!config.publication) throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  try {
    return parseApprovedPolicyCatalogue(
      JSON.parse(await runActivationClient(config, ["--list-approved"], PROCESS_TIMEOUT_MS)),
    );
  } catch {
    throw new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
  }
}

async function runActivationClient(
  config: PolicyActivationConfig,
  args: readonly string[],
  timeoutMs = config.publication ? 125_000 : PROCESS_TIMEOUT_MS,
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let stdout = Buffer.alloc(0);
    let stderr = Buffer.alloc(0);
    let outputBytes = 0;
    let failed = false;
    let settled = false;
    const child = spawn(config.executable, args, {
      env: {
        NODE_ENV: process.env.NODE_ENV ?? "production",
        PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
        RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: config.socketPath,
        ...(config.publication ? { RAZIEL_POLICY_PUBLICATION_MODE: "composed" } : {}),
      },
      shell: false,
      stdio: ["ignore", "pipe", "pipe"] as const,
    });
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
    }, timeoutMs);
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
