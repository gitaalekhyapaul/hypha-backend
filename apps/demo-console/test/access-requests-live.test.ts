import { randomUUID, webcrypto } from "node:crypto";
import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import {
  AccessIntentSchema,
  AccessRequestMutationSchema,
  AccessRequestState,
  agentApiRequestSignatureMessage,
  createAgentIdentity,
  createRazielClient,
  encodeHex,
  randomDigest,
  sha256,
} from "../../../packages/sdk-ts/dist/index.js";
import { createAccessHandlers } from "../lib/access-requests/http.ts";
import { rustAccessOwner } from "../lib/access-requests/process.ts";
import { rustAgentVerifier } from "../lib/agent-auth/process.ts";
import { RedisAgentAuthStore } from "../lib/agent-auth/redis-store.ts";
import { AgentAuthService } from "../lib/agent-auth/service.ts";
import type { AgentRegistration, AgentTokenMetadata } from "../lib/agent-auth/types.ts";
import { DatasetClientError, type SyntheticDataset } from "../lib/dataset-client/types.ts";
import { RAW_TARGET_HEADER } from "../server/agent-transport.ts";

const endpoint = process.env.RAZIEL_ACCESS_TEST_ENDPOINT;
const executable = process.env.RAZIEL_ACCESS_TEST_CLIENT;
const authExecutable = process.env.RAZIEL_ACCESS_TEST_AUTH;
const redisUrl = process.env.RAZIEL_REDIS_URL;

describe.skipIf(!endpoint || !executable || !authExecutable || !redisUrl)(
  "signed access facade through live Rust and Redis",
  () => {
    it("persists two isolated intents, reuses exact retries, and applies operator review", async () => {
      if (!endpoint || !executable || !authExecutable || !redisUrl)
        throw new Error("missing explicit live configuration");
      const audience = `access-test-${randomUUID()}`;
      const store = new RedisAgentAuthStore(redisUrl, audience);
      const auth = new AgentAuthService(audience, store, rustAgentVerifier(authExecutable));
      const identities = await Promise.all([createAgentIdentity(), createAgentIdentity()]);
      const registrations: {
        agent: AgentRegistration;
        token: string;
        metadata: AgentTokenMetadata;
      }[] = [];
      for (const [index, identity] of identities.entries()) {
        const agent = await auth.register({
          formatVersion: 1,
          displayName: `Synthetic access test ${index}`,
          publicKey: Buffer.from(identity.publicKey).toString("base64"),
        });
        const issued = await auth.issueToken(agent.agentId, {
          formatVersion: 1,
          expiresAtUnixSeconds: String(Math.floor(Date.now() / 1000) + 600),
        });
        registrations.push({ agent, ...issued });
      }
      // Dataset and operator-session admission have their own route tests. This gate
      // exercises actual agent signatures, owner RPCs, durable intent state and privacy.
      const dataset: SyntheticDataset = {
        formatVersion: 1,
        datasetId: "processor",
        revision: 1,
        source: {
          classification: "SYNTHETIC",
          sourceId: "access-test",
          sourceLabel: "Synthetic integration fixture",
        },
        status: "FINALIZED",
        rows: [],
        createdAt: "2026-09-25T00:00:00Z",
        updatedAt: "2026-09-25T00:00:00Z",
        finalization: {
          finalizedAt: "2026-09-25T00:00:00Z",
          checksum: "ab".repeat(32),
          schemaFingerprint: "cd".repeat(32),
          rowCount: 0,
        },
      };
      let datasetOffline = false;
      const handlers = createAccessHandlers({
        authenticate: async (request, body) => (await auth.authenticate(request, body)).agent,
        authorizeOperator: async () => {},
        owner: () => rustAccessOwner(executable, endpoint),
        dataset: async () => {
          if (datasetOffline) throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
          return dataset;
        },
      });
      const id = randomUUID();
      const path = `/api/agent/v1/access-requests/${id}`;
      const intent = {
        formatVersion: 1,
        requestId: id,
        taskId: "reconcile",
        purpose: "payment_reconciliation",
        action: "query",
        datasetId: "processor",
        datasetRevision: "1",
        requestContext: [
          { name: "private_label", stringValue: "context-must-not-reach-redis-or-public-output" },
        ],
      };
      const typedIntent = create(AccessIntentSchema, {
        ...intent,
        datasetRevision: 1n,
        requestContext: intent.requestContext.map((entry) => ({
          name: entry.name,
          value: { case: "stringValue" as const, value: entry.stringValue },
        })),
      });
      const clients = identities.map((identity, index) =>
        createRazielClient({
          baseUrl: "http://localhost",
          audience,
          tokenProvider: () => registrations[index]?.token ?? "",
          signer: {
            publicKey: identity.publicKey,
            sign: async (message) =>
              new Uint8Array(await webcrypto.subtle.sign("Ed25519", identity.privateKey, message)),
          },
          // Only the HTTP transport is in-process. The SDK signs the actual body,
          // and the handler calls the real Rust helper, Redis and owner RPC.
          fetch: async (url, init) => {
            const request = new Request(url, init);
            const parsed = new URL(request.url);
            request.headers.set(RAW_TARGET_HEADER, `${parsed.pathname}${parsed.search}`);
            if (parsed.pathname === "/api/agent/v1/access-requests")
              return request.method === "POST"
                ? handlers.agentCreate(request)
                : handlers.agentList(request);
            if (parsed.pathname === `${path}/cancel`) return handlers.agentCancel(request, id);
            if (parsed.pathname === path) return handlers.agentGet(request, id);
            throw new Error("unexpected SDK route");
          },
        }),
      );
      const [a, b] = clients;
      if (!a || !b) throw new Error("missing SDK clients");
      async function signed(index: number, target: string, payload?: unknown) {
        const identity = identities[index];
        const registration = registrations[index];
        if (!identity || !registration) throw new Error("test identity missing");
        const body = payload === undefined ? "" : JSON.stringify(payload);
        const nonce = randomDigest();
        const timestamp = BigInt(Math.floor(Date.now() / 1000));
        const method = payload === undefined ? "GET" : "POST";
        const message = agentApiRequestSignatureMessage({
          formatVersion: 1,
          audience,
          tokenId: registration.metadata.tokenId,
          agentKeyThumbprint: await sha256(identity.publicKey),
          requestMethod: method,
          canonicalRequestTarget: target,
          requestBodyHash: await sha256(Buffer.from(body)),
          requestNonce: nonce,
          requestTimestampUnixSeconds: timestamp,
        });
        const signature = new Uint8Array(
          await webcrypto.subtle.sign("Ed25519", identity.privateKey, message),
        );
        return new Request(`http://localhost${target}`, {
          method,
          headers: {
            [RAW_TARGET_HEADER]: target,
            authorization: `Bearer ${registration.token}`,
            "x-raziel-agent-version": "1",
            "x-raziel-agent-timestamp": String(timestamp),
            "x-raziel-agent-nonce": encodeHex(nonce),
            "x-raziel-agent-signature": encodeHex(signature),
            ...(payload === undefined ? {} : { "content-type": "application/json" }),
          },
          ...(payload === undefined ? {} : { body }),
        });
      }
      try {
        const first = await handlers.agentCreate(
          await signed(0, "/api/agent/v1/access-requests", intent),
        );
        expect(first.status).toBe(201);
        const created = await first.json();
        expect(created.agentId).toBe(registrations[0]?.agent.agentId);
        expect(created.state).toBe("ACCESS_REQUEST_STATE_PENDING_REVIEW");
        expect(JSON.stringify(created)).not.toContain("private_label");
        expect((await handlers.agentGet(await signed(1, path), id)).status).toBe(404);
        expect(
          (
            await handlers.agentCancel(
              await signed(1, `${path}/cancel`, { formatVersion: 1, expectedGeneration: "1" }),
              id,
            )
          ).status,
        ).toBe(404);

        datasetOffline = true;
        const retry = await a.createAccessRequest(typedIntent);
        expect(retry.agentId).toBe(created.agentId);
        expect(retry.generation).toBe(1n);
        expect(retry.state).toBe(AccessRequestState.PENDING_REVIEW);
        expect(
          (
            await handlers.agentCreate(
              await signed(0, "/api/agent/v1/access-requests", { ...intent, purpose: "changed" }),
            )
          ).status,
        ).toBe(409);
        datasetOffline = false;
        expect((await b.createAccessRequest(typedIntent)).agentId).toBe(
          registrations[1]?.agent.agentId,
        );
        for (const index of [0, 1]) {
          const list = await clients[index]?.listAccessRequests({ limit: 10 });
          expect(list?.requests).toEqual([
            expect.objectContaining({
              requestId: id,
              agentId: registrations[index]?.agent.agentId,
            }),
          ]);
        }
        const operatorRequest = new Request("http://localhost/api/operator/access-requests", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ formatVersion: 1, expectedGeneration: "1" }),
        });
        const rejected = await handlers.operatorMutation(
          operatorRequest,
          registrations[0]?.agent.agentId ?? "",
          id,
          "reject",
        );
        expect(rejected.status).toBe(200);
        expect(await rejected.json()).toMatchObject({
          state: "ACCESS_REQUEST_STATE_REJECTED",
          generation: "2",
        });
        expect((await a.getAccessRequest(id)).state).toBe(AccessRequestState.REJECTED);
        const cancelled = await b.cancelAccessRequest(
          id,
          create(AccessRequestMutationSchema, { formatVersion: 1, expectedGeneration: 1n }),
        );
        expect(cancelled).toMatchObject({
          state: AccessRequestState.CANCELED,
          generation: 2n,
        });
      } finally {
        for (const registration of registrations)
          await auth.revokeToken(registration.agent.agentId, registration.metadata.tokenId, {
            formatVersion: 1,
          });
        await store.close();
      }
    }, 30000);
  },
);
