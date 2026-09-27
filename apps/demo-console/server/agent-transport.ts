import type { IncomingMessage } from "node:http";

export const RAW_TARGET_HEADER = "x-raziel-internal-raw-target";

export function prepareAgentTransport(
  request: Pick<IncomingMessage, "url" | "headers" | "rawHeaders">,
): boolean {
  // Caller-supplied metadata never survives the HTTP boundary.
  delete request.headers[RAW_TARGET_HEADER];
  const seen = new Set();
  for (let index = 0; index < request.rawHeaders.length; index += 2) {
    const name = request.rawHeaders[index]?.toLowerCase() ?? "";
    if (name === "authorization" || name.startsWith("x-raziel-agent-")) {
      if (seen.has(name)) return false;
      seen.add(name);
    }
  }
  request.headers[RAW_TARGET_HEADER] = request.url ?? "";
  return true;
}
