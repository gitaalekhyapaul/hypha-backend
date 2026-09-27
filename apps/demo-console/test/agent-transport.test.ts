import type { IncomingMessage } from "node:http";
import { describe, expect, it } from "vitest";
import { prepareAgentTransport, RAW_TARGET_HEADER } from "../server/agent-transport.ts";

describe("raw agent transport boundary", () => {
  it("replaces untrusted target metadata without URL normalization", () => {
    const request: Pick<IncomingMessage, "url" | "headers" | "rawHeaders"> = {
      url: "/api/agent/v1/me?",
      headers: { [RAW_TARGET_HEADER]: "/api/agent/v1/me" },
      rawHeaders: [RAW_TARGET_HEADER, "/api/agent/v1/me"],
    };
    expect(prepareAgentTransport(request)).toBe(true);
    expect(request.headers[RAW_TARGET_HEADER]).toBe("/api/agent/v1/me?");
  });

  it.each(["Authorization", "X-Raziel-Agent-Nonce", "X-Raziel-Agent-Signature"])(
    "rejects duplicate %s headers before Node's normalized map is used",
    (name) => {
      const request: Pick<IncomingMessage, "url" | "headers" | "rawHeaders"> = {
        url: "/api/agent/v1/me",
        headers: { [name.toLowerCase()]: "one", [RAW_TARGET_HEADER]: "forged" },
        rawHeaders: [name, "one", name.toLowerCase(), "two"],
      };
      expect(prepareAgentTransport(request)).toBe(false);
      expect(request.headers[RAW_TARGET_HEADER]).toBeUndefined();
    },
  );
});
