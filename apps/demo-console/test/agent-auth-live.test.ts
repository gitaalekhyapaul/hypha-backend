import { webcrypto } from "node:crypto";
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { describe, expect, it } from "vitest";
import { createAgentIdentity, createRazielClient } from "../../../packages/sdk-ts/dist/index.js";

const baseUrl = process.env.RAZIEL_AGENT_AUTH_TEST_URL;
const password = process.env.RAZIEL_AGENT_AUTH_TEST_PASSWORD;
const audience = process.env.RAZIEL_TRUST_DOMAIN_ID;
const operatorOrigin = process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN ?? baseUrl ?? "";

describe.skipIf(!baseUrl || !password || !audience)(
  "external SDK through live HTTP, Rust, and Redis",
  () => {
    it("registers two agents, separates keys/tokens, rejects replay, and rotates/revokes credentials", async () => {
      const login = await fetch(`${baseUrl}/api/operator/login`, {
        method: "POST",
        headers: { origin: operatorOrigin },
        body: JSON.stringify({ password }),
      });
      expect(login.status).toBe(200);
      const session = (await login.json()) as { csrfToken: string };
      const cookie = login.headers.get("set-cookie")?.split(";")[0] ?? "";
      const admin = async (path: string, body?: unknown) =>
        fetch(`${baseUrl}${path}`, {
          method: body === undefined ? "GET" : "POST",
          headers: {
            cookie,
            origin: operatorOrigin,
            "content-type": "application/json",
            "x-raziel-csrf-token": session.csrfToken,
          },
          ...(body === undefined ? {} : { body: JSON.stringify(body) }),
        });
      const identities = await Promise.all([createAgentIdentity(), createAgentIdentity()]);
      const registered: { id: string; tokenId: string; token: string }[] = [];
      for (const [index, identity] of identities.entries()) {
        const result = await admin("/api/operator/agents", {
          formatVersion: 1,
          displayName: `B0064 integration ${index}`,
          publicKey: Buffer.from(identity.publicKey).toString("base64"),
        });
        expect(result.status).toBe(201);
        const agent = (await result.json()) as { agentId: string };
        const issued = await admin(`/api/operator/agents/${agent.agentId}/tokens`, {
          formatVersion: 1,
          expiresAtUnixSeconds: String(Math.floor(Date.now() / 1000) + 600),
        });
        expect(issued.status).toBe(201);
        const token = (await issued.json()) as { token: string; metadata: { tokenId: string } };
        registered.push({ id: agent.agentId, tokenId: token.metadata.tokenId, token: token.token });
      }
      const client = (index: number, token: string, fetcher?: typeof fetch) => {
        const identity = identities[index];
        if (!identity || !baseUrl || !audience) throw new Error("missing test setup");
        return createRazielClient({
          baseUrl,
          audience,
          tokenProvider: () => token,
          signer: {
            publicKey: identity.publicKey,
            sign: async (message) =>
              new Uint8Array(await webcrypto.subtle.sign("Ed25519", identity.privateKey, message)),
          },
          ...(fetcher ? { fetch: fetcher } : {}),
        });
      };
      const [a, b] = registered;
      if (!a || !b) throw new Error("missing test identities");
      try {
        for (const [index, agent] of registered.entries()) {
          expect((await client(index, agent.token).me()).agentId).toBe(agent.id);
        }
        await expect(client(0, b.token).me()).rejects.toMatchObject({
          code: "NOT_AUTHORIZED",
          status: 401,
        });
        const rawStatuses: number[] = [];
        await client(0, a.token, async (url, init) => {
          const target = new URL(String(url));
          const headers = [...new Headers(init?.headers).entries()].flat();
          rawStatuses.push(
            await rawStatus(target, target.pathname, [
              ...headers,
              "Authorization",
              `Bearer ${a.token}`,
            ]),
          );
          rawStatuses.push(await rawStatus(target, `${target.pathname}?`, headers));
          rawStatuses.push(
            await rawStatus(target, "/api/agent/v1/other/../me", [
              ...headers,
              "x-raziel-internal-raw-target",
              target.pathname,
            ]),
          );
          return fetch(url, init);
        }).me();
        expect(rawStatuses).toEqual([401, 400, 400]);
        const list = await admin(`/api/operator/agents/${a.id}/tokens`);
        const metadata = await list.text();
        expect(metadata).not.toContain(a.token);
        expect(metadata).not.toContain("digest");
        const wronglyOwned = await admin(
          `/api/operator/agents/${b.id}/tokens/${a.tokenId}/revoke`,
          { formatVersion: 1 },
        );
        expect(wronglyOwned.status).toBe(404);

        let repeatStatus = 0;
        await client(0, a.token, async (url, init) => {
          const responses = await Promise.all([fetch(url, init), fetch(url, init)]);
          expect(responses.map((r) => r.status).sort()).toEqual([200, 409]);
          const replay = responses.find((r) => r.status === 409);
          repeatStatus = replay?.status ?? 0;
          expect(await replay?.json()).toEqual({
            error: { code: "REQUEST_REPLAYED", retryable: true },
          });
          const accepted = responses.find((r) => r.status === 200);
          if (!accepted) throw new Error("missing admitted response");
          return accepted;
        }).me();
        expect(repeatStatus).toBe(409);

        const operatorAttempt = await fetch(`${baseUrl}/api/operator/agents`, {
          headers: { authorization: `Bearer ${a.token}` },
        });
        expect(operatorAttempt.status).toBe(401);
        const cookieAttempt = await fetch(`${baseUrl}/api/agent/v1/me`, { headers: { cookie } });
        expect(cookieAttempt.status).toBe(401);
        const noCsrf = await fetch(`${baseUrl}/api/operator/agents`, {
          method: "POST",
          headers: { cookie, origin: operatorOrigin, "content-type": "application/json" },
          body: "{}",
        });
        expect(noCsrf.status).toBe(403);

        const rotation = await admin(`/api/operator/agents/${a.id}/tokens`, {
          formatVersion: 1,
          expiresAtUnixSeconds: String(Math.floor(Date.now() / 1000) + 600),
        });
        expect(rotation.status).toBe(201);
        const rotated = (await rotation.json()) as { token: string; metadata: { tokenId: string } };
        registered.push({ id: a.id, token: rotated.token, tokenId: rotated.metadata.tokenId });
        expect(
          (
            await admin(`/api/operator/agents/${a.id}/tokens/${a.tokenId}/revoke`, {
              formatVersion: 1,
            })
          ).status,
        ).toBe(200);
        await expect(client(0, a.token).me()).rejects.toMatchObject({ code: "NOT_AUTHORIZED" });
        expect((await client(0, rotated.token).me()).agentId).toBe(a.id);
        expect((await client(1, b.token).me()).agentId).toBe(b.id);
      } finally {
        for (const agent of registered) {
          expect(
            (
              await admin(`/api/operator/agents/${agent.id}/tokens/${agent.tokenId}/revoke`, {
                formatVersion: 1,
              })
            ).status,
          ).toBe(200);
        }
      }
    }, 30000);
  },
);

function rawStatus(url: URL, path: string, headers: string[]): Promise<number> {
  return new Promise((resolve, reject) => {
    const send = url.protocol === "https:" ? httpsRequest : httpRequest;
    const request = send(
      url,
      { path, headers: ["Host", url.host, ...headers], method: "GET" },
      (response) => {
        response.resume();
        response.on("end", () => resolve(response.statusCode ?? 0));
      },
    );
    request.on("error", reject);
    request.setTimeout(5000, () => request.destroy(new Error("raw request timed out")));
    request.end();
  });
}
