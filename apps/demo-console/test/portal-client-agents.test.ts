import { afterEach, describe, expect, it, vi } from "vitest";
import {
  agentTokenExpiry,
  createAgentToken,
  DEFAULT_AGENT_TOKEN_DAYS,
  listAgentRegistrations,
  listAgentTokens,
  publicKeyHexToBase64,
  registerAgent,
  revokeAgentToken,
} from "../lib/portal-client/agents.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";

const PUBLIC_KEY_HEX = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
const PUBLIC_KEY_BASE64 = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
const AGENT = {
  formatVersion: 1,
  agentId: "agent-one",
  displayName: "Reconciliation agent",
  publicKey: PUBLIC_KEY_BASE64,
  keyThumbprint: PUBLIC_KEY_BASE64,
  createdAtUnixSeconds: "1780000000",
} as const;
const METADATA = {
  formatVersion: 1,
  tokenId: "12".repeat(16),
  agentId: AGENT.agentId,
  createdAtUnixSeconds: "1780000000",
  expiresAtUnixSeconds: "1782592000",
} as const;
// A deliberately synthetic test credential. No issued token enters a fixture.
const TOKEN = `rz1.${METADATA.tokenId}.${"a".repeat(43)}`;

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function mockResponse(body: unknown, status = 200) {
  const fetch = vi.fn().mockResolvedValue(Response.json(body, { status }));
  vi.stubGlobal("fetch", fetch);
  return fetch;
}

describe("agent registration browser client", () => {
  it("encodes a raw public key as canonical padded base64", () => {
    expect(publicKeyHexToBase64(PUBLIC_KEY_HEX)).toBe(PUBLIC_KEY_BASE64);
    expect(publicKeyHexToBase64(` ${PUBLIC_KEY_HEX.toUpperCase()}\n`)).toBe(PUBLIC_KEY_BASE64);
  });

  it.each(["", "00", "g".repeat(64), `0x${PUBLIC_KEY_HEX}`, "a".repeat(128), PUBLIC_KEY_BASE64])(
    "rejects malformed public-key input before a request (%s)",
    async (input) => {
      const fetch = mockResponse(AGENT);
      await expect(registerAgent("Agent", input, "csrf")).rejects.toMatchObject({
        code: "INVALID_PUBLIC_KEY",
      });
      expect(fetch).not.toHaveBeenCalled();
    },
  );

  it("registers with operator cookies, CSRF, and the exact ProtoJSON body", async () => {
    const fetch = mockResponse(AGENT, 201);
    expect(await registerAgent(" Reconciliation agent ", PUBLIC_KEY_HEX, "csrf-test")).toEqual(
      AGENT,
    );
    expect(fetch).toHaveBeenCalledWith(
      "/api/operator/agents",
      expect.objectContaining({
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
        headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "csrf-test" },
        body: JSON.stringify({
          formatVersion: 1,
          displayName: AGENT.displayName,
          publicKey: PUBLIC_KEY_BASE64,
        }),
      }),
    );
  });

  it("rejects an empty display name without sending", async () => {
    const fetch = mockResponse(AGENT);
    await expect(registerAgent("  ", PUBLIC_KEY_HEX, "csrf")).rejects.toMatchObject({
      code: "INVALID_DISPLAY_NAME",
    });
    expect(fetch).not.toHaveBeenCalled();
  });

  it("lists registrations through an uncached same-origin read", async () => {
    const fetch = mockResponse({ agents: [AGENT] });
    expect(await listAgentRegistrations()).toEqual({ agents: [AGENT] });
    expect(fetch).toHaveBeenCalledWith(
      "/api/operator/agents",
      expect.objectContaining({ credentials: "same-origin", cache: "no-store" }),
    );
    expect(fetch.mock.calls[0]?.[1]).not.toHaveProperty("body");
    expect(fetch.mock.calls[0]?.[1]).not.toHaveProperty("headers");
  });

  it.each([
    { agents: null },
    { agents: [{ ...AGENT, formatVersion: 2 }] },
    { agents: [{ ...AGENT, publicKey: PUBLIC_KEY_BASE64.replace(/=$/, "") }] },
  ])("rejects malformed registration responses", async (body) => {
    mockResponse(body);
    await expect(listAgentRegistrations()).rejects.toMatchObject({ status: 503 });
  });
});

describe("agent token browser client", () => {
  it("defaults to 30 days and accepts the 90-day ceiling", () => {
    expect(DEFAULT_AGENT_TOKEN_DAYS).toBe(30);
    expect(agentTokenExpiry(DEFAULT_AGENT_TOKEN_DAYS, 1780000000999)).toBe("1782592000");
    expect(agentTokenExpiry(90, 1780000000000)).toBe("1787776000");
  });

  it.each([0, -1, 91, 1.5, Number.NaN, Number.POSITIVE_INFINITY])(
    "rejects invalid lifetime %s",
    async (days) => {
      const fetch = mockResponse({ token: TOKEN, metadata: METADATA });
      await expect(createAgentToken(AGENT.agentId, days, "csrf")).rejects.toMatchObject({
        code: "INVALID_TOKEN_LIFETIME",
      });
      expect(fetch).not.toHaveBeenCalled();
    },
  );

  it("creates a token with a decimal-string expiry and no client secret", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(1780000000999);
    const fetch = mockResponse({ token: TOKEN, metadata: METADATA }, 201);
    expect(await createAgentToken(AGENT.agentId, 30, "csrf-test")).toEqual({
      token: TOKEN,
      metadata: METADATA,
    });
    expect(fetch).toHaveBeenCalledWith(
      "/api/operator/agents/agent-one/tokens",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "csrf-test" },
        credentials: "same-origin",
        cache: "no-store",
        body: JSON.stringify({ formatVersion: 1, expiresAtUnixSeconds: "1782592000" }),
      }),
    );
  });

  it("projects metadata only and escapes the agent path", async () => {
    const fetch = mockResponse({ tokens: [{ ...METADATA, agentId: "agent/one", token: TOKEN }] });
    const result = await listAgentTokens("agent/one");
    expect(result.tokens).toEqual([{ ...METADATA, agentId: "agent/one" }]);
    expect(JSON.stringify(result)).not.toContain(TOKEN);
    expect(fetch.mock.calls[0]?.[0]).toBe("/api/operator/agents/agent%2Fone/tokens");
  });

  it("rejects metadata from another agent", async () => {
    mockResponse({ tokens: [{ ...METADATA, agentId: "other-agent" }] });
    await expect(listAgentTokens(AGENT.agentId)).rejects.toMatchObject({ status: 503 });
  });

  it("does not accept a create token with a mismatched metadata ID", async () => {
    mockResponse({ token: TOKEN, metadata: { ...METADATA, tokenId: "34".repeat(16) } });
    await expect(createAgentToken(AGENT.agentId, 30, "csrf")).rejects.toMatchObject({
      status: 503,
    });
  });

  it("revokes with CSRF and a versioned body", async () => {
    const metadata = { ...METADATA, revokedAtUnixSeconds: "1780000001" };
    const fetch = mockResponse(metadata);
    expect(await revokeAgentToken(AGENT.agentId, METADATA.tokenId, "csrf-test")).toEqual(metadata);
    expect(fetch).toHaveBeenCalledWith(
      `/api/operator/agents/agent-one/tokens/${METADATA.tokenId}/revoke`,
      expect.objectContaining({
        method: "POST",
        body: '{"formatVersion":1}',
        headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "csrf-test" },
      }),
    );
  });

  it("does not report successful revocation without a revocation timestamp", async () => {
    mockResponse(METADATA);
    await expect(revokeAgentToken(AGENT.agentId, METADATA.tokenId, "csrf")).rejects.toMatchObject({
      status: 503,
    });
  });
});

describe("agent client failure handling", () => {
  it("keeps session expiry identifiable for non-JSON 401 responses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("Signed out", { status: 401 })));
    await expect(listAgentRegistrations()).rejects.toMatchObject({
      status: 401,
      code: "OPERATOR_UNAUTHORIZED",
    });
  });

  it.each([
    { error: "OPERATOR_CSRF_REJECTED" },
    { error: { code: "OPERATOR_CSRF_REJECTED", retryable: false } },
  ])("accepts existing and versioned error envelopes", async (body) => {
    mockResponse(body, 403);
    await expect(createAgentToken(AGENT.agentId, 30, "csrf")).rejects.toMatchObject({
      status: 403,
      code: "OPERATOR_CSRF_REJECTED",
    });
  });

  it("does not echo an untrusted error message containing secrets", async () => {
    mockResponse({ error: { code: "SERVICE_UNAVAILABLE", message: TOKEN } }, 503);
    await expect(listAgentTokens(AGENT.agentId)).rejects.toEqual(
      new PortalRequestError("SERVICE_UNAVAILABLE", 503),
    );
  });

  it("maps network failures to unavailable without retrying mutations", async () => {
    const fetch = vi.fn().mockRejectedValue(new Error("Private failure detail"));
    vi.stubGlobal("fetch", fetch);
    await expect(createAgentToken(AGENT.agentId, 30, "csrf")).rejects.toMatchObject({
      status: 503,
      message: "PORTAL_UNAVAILABLE",
    });
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("rejects non-JSON success responses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("unavailable")));
    await expect(listAgentRegistrations()).rejects.toMatchObject({ status: 503 });
  });

  it("forwards read cancellation", async () => {
    const fetch = mockResponse({ agents: [] });
    const controller = new AbortController();
    await listAgentRegistrations(controller.signal);
    const signal = fetch.mock.calls[0]?.[1]?.signal as AbortSignal;
    expect(signal.aborted).toBe(false);
    controller.abort();
    expect(signal.aborted).toBe(true);
  });
});
