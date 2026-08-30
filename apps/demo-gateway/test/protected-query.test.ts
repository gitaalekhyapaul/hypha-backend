import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";

import { handleProtectedQueryRequest } from "../app/api/datasets/[datasetId]/protected-query/route.ts";
import {
  AuthorizationRejectedError,
  ProtectedQueryInfrastructureError,
} from "../src/protected-request/errors.ts";
import { JsonlReceiptStore } from "../src/protected-request/receipt-store.ts";
import { createPublicReceipt, ProtectedQueryService } from "../src/protected-request/service.ts";
import type {
  NonceStore,
  ProtectedRequestSignatureVerifier,
  ProtectedRequestVerifier,
  PublicReceipt,
  ReceiptStore,
  VerificationCache,
  VerifiedCapability,
  VerifiedCapabilityClaims,
} from "../src/protected-request/types.ts";
import {
  parseCapabilityInspectionOutput,
  parseVerifierOutput,
} from "../src/protected-request/verifier.ts";
import {
  encodeCachedVerification,
  parseCachedVerification,
} from "../src/protected-request/verified-cache.ts";

const NOW = 1_800_000_000n;
const DATASET_ID = "reconciliation-demo";
const AGENT_PUBLIC_KEY_HEX = "22".repeat(32);
const NONCE_HEX = "33".repeat(32);
const PROOF_DIGEST_HEX = "44".repeat(32);
const CAPABILITY_ID = "45".repeat(32);
const temporaryDirectories: string[] = [];

afterEach(async () => {
  vi.restoreAllMocks();
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => rm(directory, { recursive: true, force: true })),
  );
});

describe("protected reconciliation query", () => {
  it("claims, verifies, reads bounded rows, then writes one public receipt", async () => {
    const operations: string[] = [];
    const receiptStore: ReceiptStore = {
      append: vi.fn(async () => {
        operations.push("receipt");
      }),
    };
    const service = serviceFixture({
      operations,
      receiptStore,
      queryDataset: vi.fn(async () => {
        operations.push("query");
        return [transaction()];
      }),
    });

    const result = await service.execute(DATASET_ID, envelope());

    expect(operations).toEqual([
      "inspect",
      "nonce",
      "cache-get",
      "verify",
      "cache-put",
      "query",
      "receipt",
    ]);
    expect(result).toMatchObject({
      classification: "SYNTHETIC",
      dataset_id: DATASET_ID,
      verification_path: "VERIFIER",
      row_count: 1,
      rows: [{ transactionId: "txn-001" }],
      receipt: {
        receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
        capability_id: CAPABILITY_ID,
        proof_job_id: CAPABILITY_ID,
        proof_digest: PROOF_DIGEST_HEX,
        action: "query",
        resource_type: "dataset",
        resource_scope: DATASET_ID,
        outcome: "AUDIT_OUTCOME_ACCEPTED",
        reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
      },
    });
    expect(result.receipt).not.toHaveProperty("rows");
    expect(result.receipt).not.toHaveProperty("request_body");
    expect(result.receipt).not.toHaveProperty("capability_proto_hex");
    expect(Object.keys(result.receipt).sort()).toEqual(
      [
        "format_version",
        "receipt_id",
        "receipt_type",
        "occurred_at",
        "request_id",
        "task_id",
        "capability_id",
        "proof_job_id",
        "policy_record_id",
        "policy_hash",
        "snapshot_id",
        "relationship_snapshot_root",
        "public_claims_digest",
        "proof_digest",
        "action",
        "resource_type",
        "resource_scope",
        "outcome",
        "reason_code",
      ].sort(),
    );
  });

  it("reuses cached verification with a fresh nonce and writes a new receipt", async () => {
    const operations: string[] = [];
    let cached: VerifiedCapability | undefined;
    const verificationCache: VerificationCache = {
      get: async () => {
        operations.push("cache-get");
        return cached;
      },
      put: async (verification, ttlSeconds) => {
        operations.push(`cache-put:${ttlSeconds}`);
        cached = verification;
      },
    };
    const service = serviceFixture({ operations, verificationCache });

    const cold = await service.execute(DATASET_ID, envelope());
    const hit = await service.execute(DATASET_ID, envelope({ request_nonce: "34".repeat(32) }));

    expect(cold.verification_path).toBe("VERIFIER");
    expect(hit.verification_path).toBe("CACHE");
    expect(cold.receipt.receipt_id).not.toBe(hit.receipt.receipt_id);
    expect(operations).toEqual([
      "inspect",
      "nonce",
      "cache-get",
      "verify",
      "cache-put:600",
      "query",
      "receipt",
      "inspect",
      "nonce",
      "cache-get",
      "signature",
      "query",
      "receipt",
    ]);
  });

  it("does not reuse a cached entry for a changed proof digest", async () => {
    const verificationCache: VerificationCache = {
      get: vi.fn(async () => undefined),
      put: vi.fn(async () => undefined),
    };
    const changedDigest = "55".repeat(32);
    const service = serviceFixture({ verificationCache, inspectionProofDigest: changedDigest });

    await expect(service.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );
    expect(verificationCache.get).toHaveBeenCalledWith(CAPABILITY_ID, changedDigest);
    expect(verificationCache.put).not.toHaveBeenCalled();
  });

  it("records a coarse rejected receipt for a correctly signed out-of-scope request", async () => {
    const deniedDatasetId = "reconciliation-denied-scope";
    const operations: string[] = [];
    const receipts: PublicReceipt[] = [];
    const queryDataset = vi.fn(async () => [transaction()]);
    const service = serviceFixture({
      operations,
      queryDataset,
      receiptStore: {
        append: async (receipt) => {
          operations.push("receipt");
          receipts.push(receipt);
        },
      },
    });

    await expect(
      service.execute(
        deniedDatasetId,
        envelope({
          canonical_request_target: "/api/datasets/reconciliation-denied-scope/protected-query",
        }),
      ),
    ).rejects.toBeInstanceOf(AuthorizationRejectedError);

    expect(operations).toEqual(["inspect", "nonce", "cache-get", "verify", "receipt"]);
    expect(queryDataset).not.toHaveBeenCalled();
    expect(receipts).toHaveLength(1);
    expect(receipts[0]).toMatchObject({
      receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
      capability_id: CAPABILITY_ID,
      resource_scope: DATASET_ID,
      outcome: "AUDIT_OUTCOME_REJECTED",
      reason_code: "PUBLIC_ERROR_CODE_NOT_AUTHORIZED",
    });
    expect(receipts[0]).not.toHaveProperty("requested_resource_scope");
    expect(JSON.stringify(receipts[0])).not.toContain(deniedDatasetId);
  });

  it("rejects invalid signatures and key bindings on cache hits before data access", async () => {
    const queryDataset = vi.fn(async () => [transaction()]);
    const cached: VerificationCache = {
      get: async () => verifiedCapability(),
      put: async () => undefined,
    };
    const invalidSignature = serviceFixture({
      verificationCache: cached,
      signatureValid: false,
      queryDataset,
    });
    await expect(invalidSignature.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );

    const wrongKey: VerificationCache = {
      get: async () => verifiedCapability({ agentKeyThumbprintHex: "66".repeat(32) }),
      put: async () => undefined,
    };
    const keyMismatch = serviceFixture({ verificationCache: wrongKey, queryDataset });
    await expect(keyMismatch.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );
    expect(queryDataset).not.toHaveBeenCalled();
  });

  it("uses the encoded route target and the 24-hour M5 nonce TTL", async () => {
    const scopedDatasetId = "dataset:reconciliation";
    const nonceStore: NonceStore = {
      claim: vi.fn(async () => true),
    };
    const service = serviceFixture({
      nonceStore,
      claims: claims({ resourceScope: scopedDatasetId }),
    });
    const result = await service.execute(
      scopedDatasetId,
      envelope({
        canonical_request_target: `/api/datasets/${encodeURIComponent(scopedDatasetId)}/protected-query`,
      }),
    );

    expect(result.dataset_id).toBe(scopedDatasetId);
    expect(nonceStore.claim).toHaveBeenCalledWith(CAPABILITY_ID, NONCE_HEX, 24 * 60 * 60);
  });

  it("rejects a replay before verification or dataset access", async () => {
    const operations: string[] = [];
    const queryDataset = vi.fn(async () => [transaction()]);
    const service = serviceFixture({ operations, nonceClaimed: false, queryDataset });

    await expect(service.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );
    expect(operations).toEqual(["inspect", "nonce"]);
    expect(queryDataset).not.toHaveBeenCalled();
  });

  it("rejects stale, altered, out-of-scope, and key-mismatched requests before data access", async () => {
    const queryDataset = vi.fn(async () => [transaction()]);
    const stale = serviceFixture({ queryDataset });
    await expect(
      stale.execute(DATASET_ID, envelope({ request_timestamp_unix_seconds: String(NOW - 301n) })),
    ).rejects.toBeInstanceOf(AuthorizationRejectedError);
    await expect(
      stale.execute(DATASET_ID, envelope({ request_timestamp_unix_seconds: Number(NOW) })),
    ).rejects.toBeInstanceOf(AuthorizationRejectedError);
    await expect(
      stale.execute(DATASET_ID, envelope({ request_timestamp_unix_seconds: `0${NOW}` })),
    ).rejects.toBeInstanceOf(AuthorizationRejectedError);
    await expect(
      stale.execute(DATASET_ID, envelope({ agent_public_key: "AB".repeat(32) })),
    ).rejects.toBeInstanceOf(AuthorizationRejectedError);

    const altered = envelope();
    altered.request_body_hash = "55".repeat(32);
    await expect(stale.execute(DATASET_ID, altered)).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );

    const outOfScope = serviceFixture({ queryDataset, claims: claims({ resourceScope: "other" }) });
    await expect(outOfScope.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );

    const wrongKey = serviceFixture({
      queryDataset,
      claims: claims({ agentKeyThumbprintHex: "66".repeat(32) }),
    });
    await expect(wrongKey.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );

    const mismatchedInspectedId = serviceFixture({
      queryDataset,
      claims: claims({ capabilityId: "46".repeat(32) }),
    });
    await expect(mismatchedInspectedId.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      AuthorizationRejectedError,
    );
    expect(queryDataset).not.toHaveBeenCalled();
  });

  it("treats verifier, Redis, and receipt failures as infrastructure failures", async () => {
    const verifierFailureOperations: string[] = [];
    const verifierFailure = serviceFixture({
      operations: verifierFailureOperations,
      verificationFailure: true,
    });
    await expect(verifierFailure.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );
    expect(verifierFailureOperations).toEqual(["inspect", "nonce", "cache-get", "verify"]);

    const redisFailure = serviceFixture({ nonceFailure: true });
    await expect(redisFailure.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );

    const receiptFailure = serviceFixture({ receiptFailure: true });
    await expect(receiptFailure.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );

    const queryFailureOperations: string[] = [];
    const queryFailure = serviceFixture({
      operations: queryFailureOperations,
      queryDataset: async () => {
        queryFailureOperations.push("query");
        throw new Error("dataset unavailable");
      },
    });
    await expect(queryFailure.execute(DATASET_ID, envelope())).rejects.toBeInstanceOf(
      ProtectedQueryInfrastructureError,
    );
    expect(queryFailureOperations).toEqual([
      "inspect",
      "nonce",
      "cache-get",
      "verify",
      "cache-put",
      "query",
    ]);
  });

  it("maps authorization and infrastructure failures to bounded public responses", async () => {
    const context = { params: Promise.resolve({ datasetId: DATASET_ID }) };
    const rejected = await handleProtectedQueryRequest(protectedRouteRequest(), context, {
      execute: async () => {
        throw new AuthorizationRejectedError();
      },
    });
    expect(rejected.status).toBe(403);
    expect(await rejected.json()).toEqual({ error: "AUTHORIZATION_REJECTED" });

    const unavailable = await handleProtectedQueryRequest(protectedRouteRequest(), context, {
      execute: async () => {
        throw new ProtectedQueryInfrastructureError();
      },
    });
    expect(unavailable.status).toBe(503);
    expect(await unavailable.json()).toEqual({ error: "PROTECTED_QUERY_UNAVAILABLE" });
  });

  it("parses the bounded safe verifier projection and rejects unknown output fields", () => {
    expect(
      parseCapabilityInspectionOutput(
        Buffer.from(
          JSON.stringify({ capability_id: CAPABILITY_ID, proof_digest: PROOF_DIGEST_HEX }),
        ),
      ),
    ).toEqual({ capabilityId: CAPABILITY_ID, proofDigestHex: PROOF_DIGEST_HEX });
    const parsed = parseVerifierOutput(
      Buffer.from(
        JSON.stringify({
          outcome: "verified",
          failure_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
          verified_claims: verifierClaimsProjection(),
          policy_record_id: "policy-record-1",
          snapshot_id: "snapshot-1",
          proof_digest: PROOF_DIGEST_HEX,
        }),
      ),
    );
    expect(parsed).toMatchObject({
      outcome: "verified",
      policyRecordId: "policy-record-1",
      proofDigestHex: PROOF_DIGEST_HEX,
    });
    expect(
      parseVerifierOutput(
        Buffer.from(
          JSON.stringify({ outcome: "rejected", failure_code: "PUBLIC_ERROR_CODE_REJECTED" }),
        ),
      ),
    ).toEqual({ outcome: "rejected" });

    expect(() =>
      parseVerifierOutput(
        Buffer.from(
          JSON.stringify({
            outcome: "verified",
            failure_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
            verified_claims: verifierClaimsProjection(),
            policy_record_id: "policy-record-1",
            snapshot_id: "snapshot-1",
            proof_digest: PROOF_DIGEST_HEX,
            relationships: [],
          }),
        ),
      ),
    ).toThrow(ProtectedQueryInfrastructureError);
  });

  it("round-trips only the bounded public cache projection", () => {
    const encoded = encodeCachedVerification(verifiedCapability());

    expect(parseCachedVerification(encoded)).toEqual(verifiedCapability());
    expect(encoded).not.toContain("zk_proof");
    expect(encoded).not.toContain("relationships");
    expect(encoded).not.toContain("caveat");
    const altered = { ...(JSON.parse(encoded) as Record<string, unknown>), witness: {} };
    expect(() => parseCachedVerification(JSON.stringify(altered))).toThrow(
      ProtectedQueryInfrastructureError,
    );
  });

  it("appends a stable mode-0600 JSONL receipt without private request material", async () => {
    const directory = await mkdtemp(path.join(tmpdir(), "raziel-receipt-test-"));
    temporaryDirectories.push(directory);
    const filePath = path.join(directory, "owner", "receipts.jsonl");
    const store = new JsonlReceiptStore(filePath);
    const receipt = createPublicReceipt({
      claims: claims(),
      policyRecordId: "policy-record-1",
      proofDigestHex: PROOF_DIGEST_HEX,
      requestMethod: "POST",
      canonicalRequestTarget: `/api/datasets/${DATASET_ID}/protected-query`,
      requestBodyHashHex: "12".repeat(32),
      requestNonceHex: NONCE_HEX,
      requestTimestampUnixSeconds: NOW,
      occurredAtUnixSeconds: NOW,
    });

    await store.append(receipt);
    await store.append(receipt);

    const lines = (await readFile(filePath, "utf8")).trim().split("\n");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toBe(lines[1]);
    expect(JSON.parse(lines[0] ?? "")).toEqual(receipt);
    expect(await readFile(filePath, "utf8")).not.toContain("capability_proto_hex");
    expect((await stat(filePath)).mode & 0o777).toBe(0o600);
  });
});

interface FixtureOverrides {
  readonly operations?: string[];
  readonly claims?: VerifiedCapabilityClaims;
  readonly nonceClaimed?: boolean;
  readonly inspectionFailure?: boolean;
  readonly verificationFailure?: boolean;
  readonly inspectionProofDigest?: string;
  readonly nonceFailure?: boolean;
  readonly cacheFailure?: boolean;
  readonly cachePutFailure?: boolean;
  readonly signatureFailure?: boolean;
  readonly signatureValid?: boolean;
  readonly receiptFailure?: boolean;
  readonly receiptStore?: ReceiptStore;
  readonly nonceStore?: NonceStore;
  readonly verificationCache?: VerificationCache;
  readonly signatureVerifier?: ProtectedRequestSignatureVerifier;
  readonly queryDataset?: () => Promise<readonly ReturnType<typeof transaction>[]>;
}

function serviceFixture(overrides: FixtureOverrides = {}): ProtectedQueryService {
  const operations = overrides.operations ?? [];
  const verifier: ProtectedRequestVerifier = {
    inspectCapability: async () => {
      operations.push("inspect");
      if (overrides.inspectionFailure) {
        throw new Error("verifier unavailable");
      }
      return {
        capabilityId: CAPABILITY_ID,
        proofDigestHex: overrides.inspectionProofDigest ?? PROOF_DIGEST_HEX,
      };
    },
    verify: async () => {
      operations.push("verify");
      if (overrides.verificationFailure) {
        throw new Error("verifier unavailable");
      }
      return {
        outcome: "verified",
        claims: overrides.claims ?? claims(),
        policyRecordId: "policy-record-1",
        proofDigestHex: PROOF_DIGEST_HEX,
      };
    },
  };
  const nonceStore: NonceStore =
    overrides.nonceStore ??
    ({
      claim: async () => {
        operations.push("nonce");
        if (overrides.nonceFailure) {
          throw new Error("Redis unavailable");
        }
        return overrides.nonceClaimed ?? true;
      },
    } satisfies NonceStore);
  const verificationCache: VerificationCache =
    overrides.verificationCache ??
    ({
      get: async () => {
        operations.push("cache-get");
        if (overrides.cacheFailure) {
          throw new Error("Redis unavailable");
        }
        return undefined;
      },
      put: async () => {
        operations.push("cache-put");
        if (overrides.cachePutFailure) {
          throw new Error("Redis unavailable");
        }
      },
    } satisfies VerificationCache);
  const signatureVerifier: ProtectedRequestSignatureVerifier =
    overrides.signatureVerifier ??
    ({
      verify: async () => {
        operations.push("signature");
        if (overrides.signatureFailure) {
          throw new Error("signature verifier unavailable");
        }
        return overrides.signatureValid ?? true;
      },
    } satisfies ProtectedRequestSignatureVerifier);
  const receiptStore: ReceiptStore =
    overrides.receiptStore ??
    ({
      append: async () => {
        operations.push("receipt");
        if (overrides.receiptFailure) {
          throw new Error("receipt unavailable");
        }
      },
    } satisfies ReceiptStore);
  return new ProtectedQueryService({
    verifier,
    verificationCache,
    signatureVerifier,
    nonceStore,
    receiptStore,
    queryDataset:
      overrides.queryDataset ??
      (async () => {
        operations.push("query");
        return [transaction()];
      }),
    nowUnixSeconds: () => NOW,
  });
}

function verifiedCapability(
  claimOverrides: Partial<VerifiedCapabilityClaims> = {},
): VerifiedCapability {
  return {
    outcome: "verified",
    claims: claims(claimOverrides),
    policyRecordId: "policy-record-1",
    proofDigestHex: PROOF_DIGEST_HEX,
  };
}

function envelope(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const body = Buffer.from(
    JSON.stringify({
      fromOccurredAt: "2026-08-01T00:00:00Z",
      toOccurredAt: "2026-08-02T00:00:00Z",
      limit: 10,
    }),
  );
  return {
    format_version: 1,
    capability_proto_hex: "11".repeat(128),
    agent_public_key: AGENT_PUBLIC_KEY_HEX,
    request_method: "POST",
    canonical_request_target: `/api/datasets/${DATASET_ID}/protected-query`,
    request_body_base64: body.toString("base64"),
    request_body_hash: createHash("sha256").update(body).digest("hex"),
    request_nonce: NONCE_HEX,
    request_timestamp_unix_seconds: String(NOW),
    agent_request_signature: "77".repeat(64),
    ...overrides,
  };
}

function protectedRouteRequest(): Request {
  return new Request(`http://gateway/api/datasets/${DATASET_ID}/protected-query`, {
    method: "POST",
    body: JSON.stringify(envelope()),
  });
}

function claims(overrides: Partial<VerifiedCapabilityClaims> = {}): VerifiedCapabilityClaims {
  return {
    formatVersion: 1,
    capabilityId: CAPABILITY_ID,
    agentKeyThumbprintHex: createHash("sha256")
      .update(Buffer.from(AGENT_PUBLIC_KEY_HEX, "hex"))
      .digest("hex"),
    taskId: "task-1",
    purpose: "reconciliation",
    action: "query",
    resourceType: "dataset",
    resourceScope: DATASET_ID,
    policyHashHex: "88".repeat(32),
    relationshipSnapshotRootHex: "99".repeat(32),
    snapshotId: "snapshot-1",
    issuedAtUnixSeconds: NOW - 60n,
    expiresAtUnixSeconds: NOW + 600n,
    ...overrides,
  };
}

function verifierClaimsProjection(): Record<string, unknown> {
  const value = claims();
  return {
    format_version: 1,
    capability_id: value.capabilityId,
    agent_key_thumbprint: value.agentKeyThumbprintHex,
    task_id: value.taskId,
    purpose: value.purpose,
    action: value.action,
    resource_type: value.resourceType,
    resource_scope: value.resourceScope,
    policy_hash: value.policyHashHex,
    relationship_snapshot_root: value.relationshipSnapshotRootHex,
    snapshot_id: value.snapshotId,
    issued_at_unix_seconds: String(value.issuedAtUnixSeconds),
    expires_at_unix_seconds: String(value.expiresAtUnixSeconds),
  };
}

function transaction() {
  return {
    transactionId: "txn-001",
    occurredAt: "2026-08-01T10:00:00Z",
    amountMinor: "12500",
    currency: "SGD",
    merchantReference: "merchant-a",
    accountReference: "account-1",
    transactionType: "payment",
    reconciliationStatus: "UNMATCHED" as const,
    sourceRecordDigest: "cc".repeat(32),
  };
}
