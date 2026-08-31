import { spawn } from "node:child_process";

import type { CapabilityJobProjection, CapabilityProcessConfig } from "./types.ts";
import { CapabilityFlowError } from "./types.ts";
import { validateCapabilityJobProjection } from "./validation.ts";

const PROCESS_TIMEOUT_MS = 30_000;
const MAX_OUTPUT_BYTES = 4 * 1024 * 1024;

export interface CapabilityClientResult {
  readonly capabilityProtoHex: string | null;
  readonly projection: CapabilityJobProjection;
}

export async function runCapabilityClient(
  config: CapabilityProcessConfig,
  command: unknown,
): Promise<CapabilityClientResult> {
  if (config.mode !== "live" || !config.executable || !config.endpoint) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  const output = await run(config.executable, config.endpoint, command);
  let parsed: unknown;
  try {
    parsed = JSON.parse(output) as unknown;
  } catch {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  const object = parsed as Record<string, unknown>;
  const capabilityProtoHex = privateCapabilityHex(object.capability_proto_hex);
  const projection = validateCapabilityJobProjection({
    ...(typeof object.projection === "object" && object.projection !== null
      ? object.projection
      : {}),
    authorizes_requests: false,
    recorded_source: null,
  });
  return { capabilityProtoHex, projection };
}

function run(executable: string, endpoint: string, command: unknown): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let stdout = Buffer.alloc(0);
    let outputBytes = 0;
    let failed = false;
    let settled = false;
    const child = spawn(executable, [], {
      env: {
        NODE_ENV: process.env.NODE_ENV ?? "production",
        PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
        RAZIEL_RAZIELD_ENDPOINT: endpoint,
      },
      shell: false,
      stdio: ["pipe", "pipe", "pipe"] as const,
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
      count(chunk);
    });
    child.on("error", () => {
      if (!settled) {
        settled = true;
        reject(new CapabilityFlowError("CAPABILITY_UNAVAILABLE"));
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
      if (code === 3 && !failed) {
        reject(new CapabilityFlowError("CAPABILITY_BUSY"));
        return;
      }
      if (code === 4 && !failed) {
        reject(new CapabilityFlowError("CAPABILITY_NOT_FOUND"));
        return;
      }
      if (code !== 0 || failed || stdout.byteLength === 0) {
        reject(new CapabilityFlowError("CAPABILITY_UNAVAILABLE"));
        return;
      }
      resolve(stdout.toString("utf8"));
    });
    child.stdin.on("error", () => {
      failed = true;
    });
    child.stdin.end(JSON.stringify(command));
  });
}

function privateCapabilityHex(input: unknown): string | null {
  if (input === undefined) {
    return null;
  }
  if (
    typeof input !== "string" ||
    input.length === 0 ||
    input.length > MAX_OUTPUT_BYTES * 2 ||
    input.length % 2 !== 0 ||
    !/^[0-9a-f]+$/.test(input)
  ) {
    throw new CapabilityFlowError("CAPABILITY_UNAVAILABLE");
  }
  return input;
}
