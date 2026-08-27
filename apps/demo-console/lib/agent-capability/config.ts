import type { CapabilityProcessConfig } from "./types.ts";

export function loadCapabilityProcessConfig(): CapabilityProcessConfig {
  const mode = process.env.RAZIEL_CAPABILITY_MODE ?? "recorded";
  if (mode === "recorded") {
    return { mode, executable: null, endpoint: null };
  }
  if (mode !== "live") {
    throw new Error("RAZIEL_CAPABILITY_MODE must be recorded or live");
  }
  const executable = requiredAbsolutePath("RAZIEL_CAPABILITY_CLIENT_EXECUTABLE");
  const endpoint = process.env.RAZIEL_RAZIELD_ENDPOINT;
  if (!endpoint || (!endpoint.startsWith("http://") && !endpoint.startsWith("https://"))) {
    throw new Error("RAZIEL_RAZIELD_ENDPOINT must be an HTTP endpoint");
  }
  return { mode, executable, endpoint };
}

function requiredAbsolutePath(name: string): string {
  const value = process.env[name];
  if (!value?.startsWith("/")) {
    throw new Error(`${name} must be an absolute path`);
  }
  return value;
}
