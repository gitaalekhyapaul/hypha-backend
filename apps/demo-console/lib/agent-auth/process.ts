import { spawn } from "node:child_process";
import { AgentAuthError, type AgentAuthVerifier } from "./types.ts";

export function rustAgentVerifier(executable: string): AgentAuthVerifier {
  return (command) =>
    new Promise<string>((resolve, reject) => {
      const input = JSON.stringify(command);
      if (Buffer.byteLength(input) > 8192) {
        reject(new AgentAuthError("INVALID_REQUEST"));
        return;
      }
      const child = spawn(executable, [], {
        shell: false,
        env: { NODE_ENV: "production", PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin" },
        stdio: ["pipe", "pipe", "pipe"] as const,
      });
      let output = Buffer.alloc(0);
      let outputBytes = 0;
      let failed = false;
      const timer = setTimeout(() => {
        failed = true;
        child.kill("SIGKILL");
        reject(new AgentAuthError("SERVICE_UNAVAILABLE"));
      }, 2000);
      const count = (chunk: Buffer) => {
        outputBytes += chunk.length;
        if (outputBytes > 1024) {
          failed = true;
          child.kill("SIGKILL");
        }
      };
      child.stdout.on("data", (chunk: Buffer) => {
        count(chunk);
        if (!failed) output = Buffer.concat([output, chunk]);
      });
      child.stderr.on("data", count);
      child.stdin.on("error", () => {
        failed = true;
      });
      child.on("error", () => {
        clearTimeout(timer);
        reject(new AgentAuthError("SERVICE_UNAVAILABLE"));
      });
      child.on("close", (code) => {
        clearTimeout(timer);
        if (!failed && code === 2 && outputBytes === 0) {
          reject(new AgentAuthError("UNAUTHENTICATED"));
          return;
        }
        try {
          if (failed || code !== 0) throw new Error();
          const result = JSON.parse(output.toString("utf8")) as Record<string, unknown>;
          if (
            Object.keys(result).length !== 1 ||
            typeof result.agent_key_thumbprint !== "string" ||
            !/^[0-9a-f]{64}$/.test(result.agent_key_thumbprint)
          )
            throw new Error();
          resolve(result.agent_key_thumbprint);
        } catch {
          reject(new AgentAuthError("SERVICE_UNAVAILABLE"));
        }
      });
      child.stdin.end(input);
    });
}
