import { AccessGraphError, type AccessGraphProcessConfig } from "./types.ts";

export function loadAccessGraphConfig(): AccessGraphProcessConfig {
  const executable = process.env.RAZIEL_ACCESS_GRAPH_EXECUTABLE;
  const socketPath = process.env.RAZIEL_ACCESS_GRAPH_SOCKET_PATH;
  if (!absolutePath(executable) || !absolutePath(socketPath)) {
    throw new AccessGraphError("ACCESS_GRAPH_UNAVAILABLE");
  }
  return { executable, socketPath };
}

function absolutePath(value: string | undefined): value is string {
  return value?.startsWith("/") === true && value !== "/" && !value.includes("\0");
}
