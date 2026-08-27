import { spawn } from "node:child_process";

import {
  AccessGraphError,
  type AccessGraphProcessConfig,
  type AccessGraphPublicationProjection,
  type PublishAccessGraphRequest,
} from "./types.ts";
import { validatePublicationProjection, validatePublicationRequest } from "./validation.ts";

const PROCESS_TIMEOUT_MS = 120_000;
const MAX_OUTPUT_BYTES = 64 * 1024;

export async function publishAccessGraph(
  requestInput: unknown,
  operatorId: string,
  config: AccessGraphProcessConfig,
): Promise<AccessGraphPublicationProjection> {
  const request = validatePublicationRequest(requestInput);
  const output = await runPublicationClient(config, request, operatorId);
  let parsed: unknown;
  try {
    parsed = JSON.parse(output) as unknown;
  } catch {
    throw new AccessGraphError("ACCESS_GRAPH_UNAVAILABLE");
  }
  return validatePublicationProjection(parsed);
}

function runPublicationClient(
  config: AccessGraphProcessConfig,
  request: PublishAccessGraphRequest,
  operatorId: string,
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let stdout = Buffer.alloc(0);
    let outputBytes = 0;
    let failed = false;
    let settled = false;
    const child = spawn(config.executable, [], {
      env: {
        NODE_ENV: process.env.NODE_ENV ?? "production",
        PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
        RAZIEL_ACCESS_GRAPH_SOCKET_PATH: config.socketPath,
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
        reject(new AccessGraphError("ACCESS_GRAPH_UNAVAILABLE"));
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
        reject(new AccessGraphError("ACCESS_GRAPH_UNAVAILABLE"));
        return;
      }
      resolve(stdout.toString("utf8"));
    });
    child.stdin.on("error", () => {
      failed = true;
    });
    child.stdin.end(JSON.stringify({ ...request, operator_id: operatorId }));
  });
}
