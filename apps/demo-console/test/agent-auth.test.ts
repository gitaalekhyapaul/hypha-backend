import { createHash } from "node:crypto";
import { describe, expect, it, vi } from "vitest";
import { AgentAuthService, tokenActive } from "../lib/agent-auth/service.ts";
import {
  AgentAuthError,
  type AgentAuthStore,
  type AgentRecord,
  type AgentTokenRecord,
} from "../lib/agent-auth/types.ts";
import { tokenParts } from "../lib/agent-auth/validation.ts";

const NOW = 1800000000;
const AUDIENCE = "raziel-test";
const KEY = Buffer.alloc(32, 7).toString("base64");

class TestStore implements AgentAuthStore {
  readonly agents = new Map<string, AgentRecord>();
  readonly tokens = new Map<string, AgentTokenRecord>();
  readonly nonces = new Map<string, number>();
  unavailable = false;
  async register(agent: AgentRecord) {
    if ([...this.agents.values()].some((a) => a.keyThumbprint === agent.keyThumbprint))
      return false;
    this.agents.set(agent.agentId, agent);
    return true;
  }
  async getAgent(id: string) {
    return this.agents.get(id);
  }
  async listAgents() {
    return [...this.agents.values()];
  }
  async createToken(token: AgentTokenRecord) {
    this.tokens.set(token.tokenId, token);
    return true;
  }
  async getToken(id: string) {
    return this.tokens.get(id);
  }
  async listTokens(agentId: string) {
    return [...this.tokens.values()].filter((t) => t.agentId === agentId);
  }
  async revokeToken(agentId: string, id: string, now: number) {
    const token = this.tokens.get(id);
    if (!token || token.agentId !== agentId) return undefined;
    const revoked = { ...token, revokedAtUnixSeconds: token.revokedAtUnixSeconds ?? String(now) };
    this.tokens.set(id, revoked);
    return revoked;
  }
  async consumeNonce(input: Parameters<AgentAuthStore["consumeNonce"]>[0]) {
    if (this.unavailable) throw new AgentAuthError("SERVICE_UNAVAILABLE");
    const token = this.tokens.get(input.token.tokenId);
    if (
      !this.agents.get(input.agent.agentId)?.active ||
      !token ||
      !tokenActive(token, input.now, input.timestamp)
    )
      return "unauthenticated" as const;
    const key = `${input.token.tokenId}:${input.nonce}`;
    if ((this.nonces.get(key) ?? 0) > input.now) return "replay" as const;
    this.nonces.set(key, input.timestamp + 301);
    return "admitted" as const;
  }
}

async function fixture() {
  const store = new TestStore();
  // This unit double isolates admission rules. Rust and integration tests verify signatures.
  const verify = vi.fn(async (command: { agent_public_key: string }) =>
    createHash("sha256").update(Buffer.from(command.agent_public_key, "hex")).digest("hex"),
  );
  let now = NOW;
  const service = new AgentAuthService(AUDIENCE, store, verify, () => now);
  const agent = await service.register({
    formatVersion: 1,
    displayName: "Processor agent",
    publicKey: KEY,
  });
  const issued = await service.issueToken(agent.agentId, {
    formatVersion: 1,
    expiresAtUnixSeconds: String(NOW + 86400),
  });
  return {
    store,
    verify,
    service,
    agent,
    issued,
    setNow: (value: number) => {
      now = value;
    },
  };
}

function request(token: string, overrides: Record<string, string> = {}) {
  return new Request("http://localhost:3000/api/agent/v1/me", {
    headers: {
      authorization: `Bearer ${token}`,
      "x-raziel-agent-version": "1",
      "x-raziel-agent-timestamp": String(NOW),
      "x-raziel-agent-nonce": "a".repeat(64),
      "x-raziel-agent-signature": "b".repeat(128),
      "x-raziel-internal-raw-target": "/api/agent/v1/me",
      ...overrides,
    },
  });
}

describe("registered agent admission", () => {
  it("stores only token hashes and returns metadata without secret fields", async () => {
    const f = await fixture();
    expect(f.issued.token).toMatch(/^rz1\.[0-9a-f]{32}\.[A-Za-z0-9_-]{43}$/);
    const stored = JSON.stringify([...f.store.tokens.values()]);
    expect(stored).not.toContain(f.issued.token);
    expect(stored).not.toContain(tokenParts(f.issued.token).secret);
    expect(await f.service.listTokens(f.agent.agentId)).toEqual([f.issued.metadata]);
    expect(await f.service.listAgents()).toEqual([f.agent]);
    expect(JSON.stringify(f.issued.metadata)).not.toContain("digest");
  });

  it("rejects duplicate keys, weak-key helper rejection, noncanonical keys and unknown fields", async () => {
    const f = await fixture();
    await expect(
      f.service.register({ formatVersion: 1, displayName: "Other", publicKey: KEY }),
    ).rejects.toMatchObject({ code: "AGENT_ALREADY_REGISTERED" });
    await expect(
      f.service.register({ formatVersion: 1, displayName: "Other", publicKey: KEY.slice(0, -1) }),
    ).rejects.toMatchObject({ code: "INVALID_REQUEST" });
    await expect(
      f.service.register({
        formatVersion: 1,
        displayName: "Other",
        publicKey: KEY,
        agentId: "caller",
      }),
    ).rejects.toMatchObject({ code: "INVALID_REQUEST" });
    f.verify.mockRejectedValueOnce(new AgentAuthError("UNAUTHENTICATED"));
    await expect(
      f.service.register({ formatVersion: 1, displayName: "Other", publicKey: KEY }),
    ).rejects.toMatchObject({ code: "INVALID_REQUEST" });
  });

  it("allows exactly one concurrent admission and preserves the nonce after downstream failure", async () => {
    const f = await fixture();
    const results = await Promise.allSettled([
      f.service.authenticate(request(f.issued.token)),
      f.service.authenticate(request(f.issued.token)),
    ]);
    expect(results.filter((r) => r.status === "fulfilled")).toHaveLength(1);
    expect(results.find((r) => r.status === "rejected")).toMatchObject({
      reason: { code: "REQUEST_REPLAYED" },
    });
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "REQUEST_REPLAYED",
    });
  });

  it("uses registered identity, not an operator cookie or caller key", async () => {
    const f = await fixture();
    const result = await f.service.authenticate(
      request(f.issued.token, { "x-agent-public-key": "attacker" }),
    );
    expect(result.agent).toEqual(f.agent);
    expect(f.verify).toHaveBeenLastCalledWith(
      expect.objectContaining({
        agent_public_key: Buffer.from(KEY, "base64").toString("hex"),
        audience: AUDIENCE,
        canonical_request_target: "/api/agent/v1/me",
      }),
    );
    await expect(
      f.service.authenticate(
        new Request("http://localhost/api/agent/v1/me", {
          headers: { cookie: "raziel_operator=anything" },
        }),
      ),
    ).rejects.toMatchObject({ code: "UNAUTHENTICATED" });
  });

  it("rejects bad signatures before replay admission and fails closed on Redis/helper failures", async () => {
    const f = await fixture();
    f.verify.mockRejectedValueOnce(new AgentAuthError("UNAUTHENTICATED"));
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "UNAUTHENTICATED",
    });
    expect(f.store.nonces.size).toBe(0);
    f.verify.mockRejectedValueOnce(new AgentAuthError("SERVICE_UNAVAILABLE"));
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "SERVICE_UNAVAILABLE",
    });
    f.store.unavailable = true;
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "SERVICE_UNAVAILABLE",
    });
  });

  it("keeps token rotation on the same identity, rejects revoked tokens, and isolates other agents", async () => {
    const f = await fixture();
    const b = await f.service.register({
      formatVersion: 1,
      displayName: "Bank agent",
      publicKey: Buffer.alloc(32, 8).toString("base64"),
    });
    const bToken = await f.service.issueToken(b.agentId, {
      formatVersion: 1,
      expiresAtUnixSeconds: String(NOW + 600),
    });
    expect(await f.service.listTokens(b.agentId)).toEqual([bToken.metadata]);
    await expect(
      f.service.revokeToken(b.agentId, f.issued.metadata.tokenId, { formatVersion: 1 }),
    ).rejects.toMatchObject({ code: "NOT_FOUND" });
    const rotated = await f.service.issueToken(f.agent.agentId, {
      formatVersion: 1,
      expiresAtUnixSeconds: String(NOW + 600),
    });
    const revoked = await f.service.revokeToken(f.agent.agentId, f.issued.metadata.tokenId, {
      formatVersion: 1,
    });
    expect(
      await f.service.revokeToken(f.agent.agentId, f.issued.metadata.tokenId, { formatVersion: 1 }),
    ).toEqual(revoked);
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "UNAUTHENTICATED",
    });
    expect((await f.service.authenticate(request(rotated.token))).agent.agentId).toBe(
      f.agent.agentId,
    );
    expect((await f.service.authenticate(request(bToken.token))).agent.agentId).toBe(b.agentId);
  });

  it("rechecks revocation after signature validation", async () => {
    const f = await fixture();
    f.verify.mockImplementationOnce(async () => {
      await f.service.revokeToken(f.agent.agentId, f.issued.metadata.tokenId, { formatVersion: 1 });
      return createHash("sha256").update(Buffer.from(KEY, "base64")).digest("hex");
    });
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "UNAUTHENTICATED",
    });
    expect(f.store.nonces.size).toBe(0);
  });

  it.each(["01", "-1", "253402300800", "1e9"])(
    "rejects timestamp representation %s",
    async (stamp) => {
      const f = await fixture();
      await expect(
        f.service.authenticate(request(f.issued.token, { "x-raziel-agent-timestamp": stamp })),
      ).rejects.toMatchObject({ code: "UNAUTHENTICATED" });
    },
  );

  it("enforces inclusive freshness edges and validity at signing and admission", async () => {
    const f = await fixture();
    f.setNow(NOW + 300);
    await expect(f.service.authenticate(request(f.issued.token))).resolves.toMatchObject({
      audience: AUDIENCE,
    });
    f.setNow(NOW + 301);
    await expect(f.service.authenticate(request(f.issued.token))).rejects.toMatchObject({
      code: "UNAUTHENTICATED",
    });
    f.setNow(NOW);
    await expect(
      f.service.authenticate(
        request(f.issued.token, {
          "x-raziel-agent-timestamp": String(NOW + 30),
          "x-raziel-agent-nonce": "c".repeat(64),
        }),
      ),
    ).resolves.toMatchObject({ audience: AUDIENCE });
    await expect(
      f.service.authenticate(
        request(f.issued.token, { "x-raziel-agent-timestamp": String(NOW + 31) }),
      ),
    ).rejects.toMatchObject({ code: "UNAUTHENTICATED" });
    await expect(
      f.service.authenticate(
        request(f.issued.token, { "x-raziel-agent-timestamp": String(NOW - 1) }),
      ),
    ).rejects.toMatchObject({ code: "UNAUTHENTICATED" });
    f.setNow(NOW + 86400);
    await expect(
      f.service.authenticate(
        request(f.issued.token, { "x-raziel-agent-timestamp": String(NOW + 86400) }),
      ),
    ).rejects.toMatchObject({ code: "UNAUTHENTICATED" });
  });

  it("rejects duplicate headers, transformed bodies, wrong secrets, and overlong tokens", async () => {
    const f = await fixture();
    for (const headers of [
      { "x-raziel-agent-nonce": `${"a".repeat(64)}, ${"a".repeat(64)}` },
      { "content-encoding": "gzip" },
      { "content-length": "1" },
      { authorization: `Bearer ${f.issued.token}x` },
      {
        authorization: `Bearer rz1.${f.issued.metadata.tokenId}.${Buffer.alloc(32).toString("base64url")}`,
      },
    ])
      await expect(f.service.authenticate(request(f.issued.token, headers))).rejects.toMatchObject({
        code: "UNAUTHENTICATED",
      });
    await expect(
      f.service.authenticate(request(f.issued.token), new Uint8Array([1])),
    ).rejects.toMatchObject({ code: "UNAUTHENTICATED" });
  });

  it("bounds token lifetime and version", async () => {
    const f = await fixture();
    for (const expires of [NOW, NOW - 1, NOW + 90 * 86400 + 1]) {
      await expect(
        f.service.issueToken(f.agent.agentId, {
          formatVersion: 1,
          expiresAtUnixSeconds: String(expires),
        }),
      ).rejects.toMatchObject({ code: "INVALID_REQUEST" });
    }
    await expect(
      f.service.issueToken(f.agent.agentId, {
        formatVersion: 2,
        expiresAtUnixSeconds: String(NOW + 600),
      }),
    ).rejects.toMatchObject({ code: "INVALID_REQUEST" });
  });
});
