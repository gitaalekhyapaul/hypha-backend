import { create, toBinary } from "@bufbuild/protobuf";
import { describe, expect, it, vi } from "vitest";
import {
  createAgentIdentity,
  createRazielClient,
  createWebCryptoSigner,
  decodeHex,
  encodeHex,
  ProofCapabilitySchema,
  verifyProtectedRequestSignature,
} from "../../../packages/sdk-ts/dist/index.js";
import { ProtectedQueryService } from "../../demo-gateway/src/protected-request/service.ts";
import type {
  ProtectedRequestEnvelope,
  VerifiedCapability,
} from "../../demo-gateway/src/protected-request/types.ts";
import { AgentAuthError } from "../lib/agent-auth/types.ts";
import { createAgentQueryHandler } from "../lib/agent-query/http.ts";
import { RAW_TARGET_HEADER } from "../server/agent-transport.ts";

describe("packaged SDK through the facade and gateway", () => {
  it("reuses a capability on cold/cache paths and checks authentication on both", async () => {
    const identity = await createAgentIdentity();
    const now = BigInt(Math.floor(Date.now() / 1000));
    const datasetId = "processor:ledger";
    const capability = create(ProofCapabilitySchema, {
      formatVersion: 1,
      capabilityId: "aa".repeat(32),
      agentKeyThumbprint: identity.keyThumbprint,
      taskId: "reconcile",
      purpose: "payment_reconciliation",
      action: "query",
      resourceType: `dataset_${"bb".repeat(16)}`,
      resourceScope: datasetId,
      policyHash: new Uint8Array(32).fill(1),
      relationshipSnapshotRoot: new Uint8Array(32).fill(2),
      snapshotId: `snapshot-v1:${"02".repeat(32)}`,
      issuedAt: { seconds: now - 10n },
      expiresAt: { seconds: now + 600n },
    });
    // Only proof verification and infrastructure are controlled here. Real proof
    // execution remains a separate VM gate; these are not valid Plonk proof bytes.
    const verified: VerifiedCapability = {
      outcome: "verified",
      policyRecordId: "policy-1",
      proofDigestHex: "cc".repeat(32),
      claims: {
        formatVersion: 1,
        capabilityId: capability.capabilityId,
        agentKeyThumbprintHex: encodeHex(identity.keyThumbprint),
        taskId: capability.taskId,
        purpose: capability.purpose,
        action: "query",
        resourceType: capability.resourceType,
        resourceScope: datasetId,
        policyHashHex: encodeHex(capability.policyHash),
        relationshipSnapshotRootHex: encodeHex(capability.relationshipSnapshotRoot),
        snapshotId: capability.snapshotId,
        issuedAtUnixSeconds: now - 10n,
        expiresAtUnixSeconds: now + 600n,
      },
    };
    const signature = (id: string, envelope: ProtectedRequestEnvelope) =>
      verifyProtectedRequestSignature({
        formatVersion: 1,
        capabilityId: id,
        agentPublicKey: decodeHex(envelope.agentPublicKeyHex),
        requestMethod: envelope.requestMethod,
        canonicalRequestTarget: envelope.canonicalRequestTarget,
        requestBodyHash: decodeHex(envelope.requestBodyHashHex),
        requestNonce: decodeHex(envelope.requestNonceHex),
        requestTimestampUnixSeconds: envelope.requestTimestampUnixSeconds,
        agentRequestSignature: decodeHex(envelope.agentRequestSignatureHex),
      });
    const verify = vi.fn(async (envelope: ProtectedRequestEnvelope) => {
      expect(await signature(capability.capabilityId, envelope)).toBe(true);
      return verified;
    });
    let cached: VerifiedCapability | undefined;
    const nonces = new Set<string>();
    const service = new ProtectedQueryService({
      verifier: {
        inspectCapability: async () => ({
          capabilityId: capability.capabilityId,
          proofDigestHex: verified.proofDigestHex,
        }),
        verify,
      },
      signatureVerifier: { verify: signature },
      verificationCache: {
        get: async () => cached,
        put: async (value) => {
          cached = value;
        },
      },
      nonceStore: {
        claim: async (_id, nonce) => {
          if (nonces.has(nonce)) return false;
          nonces.add(nonce);
          return true;
        },
      },
      receiptStore: { append: async () => {} },
      resolveDatasetBinding: async () => ({
        format_version: 1,
        policy_hash: verified.claims.policyHashHex,
        resource_type: capability.resourceType,
        resource_scope: datasetId,
        dataset_id: datasetId,
        dataset_revision: "1",
        dataset_checksum: "dd".repeat(32),
        action: "query",
      }),
      queryDataset: async () => [
        {
          transactionId: "txn-1",
          occurredAt: "2026-09-01T00:00:00Z",
          amountMinor: "1200",
          currency: "SGD",
          reconciliationStatus: "UNMATCHED",
          sourceRecordDigest: "ee".repeat(32),
        },
      ],
    });
    let revoked = false;
    const authenticate = vi.fn(async () => {
      if (revoked) throw new AgentAuthError("UNAUTHENTICATED");
      return {
        formatVersion: 1 as const,
        agentId: "ff".repeat(16),
        displayName: "SDK consumer",
        publicKey: Buffer.from(identity.publicKey).toString("base64"),
        keyThumbprint: Buffer.from(identity.keyThumbprint).toString("base64"),
        createdAtUnixSeconds: now.toString(),
      };
    });
    const handler = createAgentQueryHandler({
      authenticate,
      gatewayUrl: () => "http://gateway:3001",
      fetch: async (_url, init) =>
        Response.json(
          await service.execute(datasetId, JSON.parse(await new Response(init?.body).text())),
        ),
    });
    const client = createRazielClient({
      baseUrl: "https://raziel.example",
      audience: "consumer-test",
      tokenProvider: () => `rz1.${"11".repeat(16)}.${Buffer.alloc(32, 1).toString("base64url")}`,
      signer: createWebCryptoSigner(identity),
      fetch: async (url, init) => {
        const request = new Request(String(url), init);
        request.headers.set(RAW_TARGET_HEADER, new URL(String(url)).pathname);
        return handler(request, datasetId);
      },
    });
    const bytes = toBinary(ProofCapabilitySchema, capability);
    const query = {
      fromOccurredAt: "2026-09-01T00:00:00Z",
      toOccurredAt: "2026-09-02T00:00:00Z",
      limit: 20,
    };
    const first = await client.queryDataset(datasetId, bytes, query);
    const second = await client.queryDataset(datasetId, bytes, query);
    expect(first.verification_path).toBe("VERIFIER");
    expect(second.verification_path).toBe("CACHE");
    expect(first.receipt.receipt_id).not.toBe(second.receipt.receipt_id);
    expect(second.rows[0]?.transactionId).toBe("txn-1");
    expect(verify).toHaveBeenCalledOnce();
    expect(nonces.size).toBe(2);
    revoked = true;
    await expect(client.queryDataset(datasetId, bytes, query)).rejects.toMatchObject({
      code: "NOT_AUTHORIZED",
    });
    expect(authenticate).toHaveBeenCalledTimes(3);
    expect(nonces.size).toBe(2);
  });
});
