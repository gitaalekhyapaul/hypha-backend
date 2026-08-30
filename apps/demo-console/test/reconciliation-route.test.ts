import { createHash } from "node:crypto";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";

import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import {
  exactGatewayQueryBody,
  readReconciliationQuery,
} from "../app/api/reconciliation/query/contract.ts";
import { POST as queryRoute } from "../app/api/reconciliation/query/route.ts";
import {
  type LiveCapabilityBinding,
  registerLiveJob,
  retainedCapabilityProtoHex,
  retainLiveCapability,
} from "../lib/agent-capability/live-registry.ts";
import { capabilityAuthorizesPublishedDataset } from "../lib/agent-capability/readiness.ts";
import { createOperatorAgentSession } from "../lib/agent-capability/session-store.ts";
import type {
  CapabilityJobProjection,
  PublishedAccessSelection,
} from "../lib/agent-capability/types.ts";
import {
  derivePasswordVerifierForProvisioning,
  OPERATOR_CSRF_HEADER,
} from "../lib/operator-auth/index.ts";

const ORIGIN = "http://127.0.0.1:3000";
const PASSWORD = "reconciliation route password";
const OPERATOR_ID = "operator-reconciliation";
const DATASET_ID = "synthetic-reconciliation";
const originalFetch = globalThis.fetch;
const roots: string[] = [];
const environmentNames = [
  "RAZIEL_OPERATOR_SECRET_PATH",
  "RAZIEL_OPERATOR_EXPECTED_ORIGIN",
  "RAZIEL_OPERATOR_LIVE_MODE",
  "RAZIEL_GATEWAY_INTERNAL_URL",
] as const;
const originalEnvironment = Object.fromEntries(
  environmentNames.map((name) => [name, process.env[name]]),
) as Record<(typeof environmentNames)[number], string | undefined>;

afterEach(async () => {
  globalThis.fetch = originalFetch;
  restoreEnvironment();
  vi.restoreAllMocks();
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

describe("B-0051 protected reconciliation console route", () => {
  it("authenticates before reading the protected query body", async () => {
    await configureAuth();
    const fetchMock = vi.fn();
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const request = new Request(`${ORIGIN}/api/reconciliation/query`, { method: "POST" });
    Object.defineProperty(request, "text", {
      value: async () => {
        throw new Error("body should not be read");
      },
    });

    const response = await queryRoute(request);
    expect(response.status).toBe(403);
    expect(await response.json()).toEqual({ error: "OPERATOR_ORIGIN_REJECTED" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("signs exact deterministic query bytes and returns only safe public output", async () => {
    vi.spyOn(Date, "now").mockReturnValue(Date.parse("2026-08-27T04:05:06Z"));
    await configureAuth();
    const session = await login();
    const jobId = "41".repeat(32);
    await prepareLiveCapability(jobId, DATASET_ID);

    let forwarded: Record<string, unknown> | null = null;
    globalThis.fetch = vi.fn(async (input, init) => {
      expect(new URL(input instanceof Request ? input.url : input.toString()).pathname).toBe(
        `/api/datasets/${DATASET_ID}/protected-query`,
      );
      expect(init?.method).toBe("POST");
      forwarded = JSON.parse(String(init?.body)) as Record<string, unknown>;
      return Response.json(gatewaySuccess(jobId));
    }) as typeof fetch;

    const response = await queryRoute(
      mutationRequest(session, {
        job_id: jobId,
        dataset_id: DATASET_ID,
        from_occurred_at: "2026-08-01T00:00:00Z",
        to_occurred_at: "2026-08-02T00:00:00Z",
        limit: 25,
      }),
    );
    expect(response.status).toBe(200);
    const result = (await response.json()) as Record<string, unknown>;
    expect(result).toMatchObject({
      classification: "SYNTHETIC",
      dataset_id: DATASET_ID,
      row_count: 1,
      receipt: {
        receipt_id: "receipt-001",
        capability_id: jobId,
        outcome: "AUDIT_OUTCOME_ACCEPTED",
        reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
      },
    });
    expect(JSON.stringify(result)).not.toContain("raw_proof");
    expect(JSON.stringify(result)).not.toContain("private_policy");
    expect(JSON.stringify(result)).not.toContain("relationship_path");

    expect(forwarded).not.toBeNull();
    const envelope = forwarded as unknown as Record<string, unknown>;
    expect(Object.keys(envelope).sort()).toEqual(
      [
        "agent_public_key",
        "agent_request_signature",
        "canonical_request_target",
        "capability_proto_hex",
        "format_version",
        "request_body_base64",
        "request_body_hash",
        "request_method",
        "request_nonce",
        "request_timestamp_unix_seconds",
      ].sort(),
    );
    expect(envelope).not.toHaveProperty("capability_id");
    expect(envelope).not.toHaveProperty("agent_private_key");
    expect(envelope.format_version).toBe(1);
    expect(envelope.capability_proto_hex).toBe("cafe");
    expect(envelope.request_method).toBe("POST");
    expect(envelope.canonical_request_target).toBe(`/api/datasets/${DATASET_ID}/protected-query`);
    expect(envelope.request_timestamp_unix_seconds).toBe("1787803506");
    expect(envelope.agent_public_key).toMatch(/^[0-9a-f]{64}$/);
    expect(envelope.request_body_hash).toMatch(/^[0-9a-f]{64}$/);
    expect(envelope.request_nonce).toMatch(/^[0-9a-f]{64}$/);
    expect(envelope.agent_request_signature).toMatch(/^[0-9a-f]{128}$/);

    const exactBody = JSON.stringify({
      fromOccurredAt: "2026-08-01T00:00:00Z",
      toOccurredAt: "2026-08-02T00:00:00Z",
      limit: 25,
    });
    expect(Buffer.from(String(envelope.request_body_base64), "base64").toString("utf8")).toBe(
      exactBody,
    );
    expect(envelope.request_body_hash).toBe(
      createHash("sha256").update(exactBody, "utf8").digest("hex"),
    );
  });

  it("maps a mismatched dataset binding to one generic authorization failure", async () => {
    await configureAuth();
    const session = await login();
    const jobId = "42".repeat(32);
    await prepareLiveCapability(jobId, "different-dataset");
    const fetchMock = vi.fn();
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const response = await queryRoute(
      mutationRequest(session, {
        job_id: jobId,
        dataset_id: DATASET_ID,
        from_occurred_at: "2026-08-01T00:00:00Z",
        to_occurred_at: "2026-08-02T00:00:00Z",
        limit: 25,
      }),
    );
    expect(response.status).toBe(403);
    expect(await response.json()).toEqual({
      error: "RECONCILIATION_AUTHORIZATION_FAILED",
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("rejects undeclared browser fields and fixes gateway query property order", async () => {
    const parsed = await readReconciliationQuery(
      new Request(`${ORIGIN}/api/reconciliation/query`, {
        method: "POST",
        body: JSON.stringify({
          job_id: "43".repeat(32),
          dataset_id: DATASET_ID,
          from_occurred_at: "2026-08-01T00:00:00Z",
          to_occurred_at: "2026-08-02T00:00:00Z",
          limit: 10,
        }),
      }),
    );
    expect(new TextDecoder().decode(exactGatewayQueryBody(parsed))).toBe(
      '{"fromOccurredAt":"2026-08-01T00:00:00Z","toOccurredAt":"2026-08-02T00:00:00Z","limit":10}',
    );

    await expect(
      readReconciliationQuery(
        new Request(`${ORIGIN}/api/reconciliation/query`, {
          method: "POST",
          body: JSON.stringify({ ...parsed, request_nonce: "44".repeat(32) }),
        }),
      ),
    ).rejects.toMatchObject({ code: "INVALID_RECONCILIATION_QUERY", status: 400 });
  });
});

describe("B-0051 reconciliation navigation readiness", () => {
  it("accepts only the exact live succeeded capability for the published dataset", () => {
    const access = publishedAccess();
    const live = capabilityJob("51".repeat(32), access, "live");
    expect(capabilityAuthorizesPublishedDataset(live, access)).toBe(true);
    expect(capabilityAuthorizesPublishedDataset({ ...live, mode: "recorded" }, access)).toBe(false);
    expect(
      capabilityAuthorizesPublishedDataset(
        {
          ...live,
          capability: live.capability
            ? { ...live.capability, resource_scope: "different-dataset" }
            : null,
        },
        access,
      ),
    ).toBe(false);
  });

  it("does not retain queryable capability bytes before live success", () => {
    const jobId = "52".repeat(32);
    const access = publishedAccess();
    const binding: LiveCapabilityBinding = {
      action: "query",
      agentKeyThumbprint: access.agent.key_thumbprint,
      operatorId: "operator-pending",
      purpose: "payment-reconciliation",
      resourceScope: access.datasetId,
      resourceType: "dataset",
      snapshotId: access.snapshotId,
      taskId: "reconcile-batch-001",
    };
    registerLiveJob(jobId, binding);
    const retained = retainLiveCapability(
      { ...capabilityJob(jobId, access, "live"), state: "PROOF_JOB_STATE_PROVING" },
      "cafe",
      binding,
    );
    expect(retained.authorizes_requests).toBe(false);
    expect(() => retainedCapabilityProtoHex(jobId, binding.operatorId)).toThrowError(
      "CAPABILITY_NOT_FOUND",
    );
  });
});

interface LoggedInSession {
  readonly cookie: string;
  readonly csrfToken: string;
}

async function configureAuth(): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "raziel-reconciliation-route-test-"));
  roots.push(root);
  await chmod(root, 0o700);
  const salt = Buffer.alloc(16, 0x61);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  const secretPath = join(root, "operator-secret.json");
  await writeFile(
    secretPath,
    `${JSON.stringify({
      format_version: 1,
      operator_id: OPERATOR_ID,
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x62).toString("base64url"),
    })}\n`,
    { mode: 0o600 },
  );
  process.env.RAZIEL_OPERATOR_SECRET_PATH = secretPath;
  process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN = ORIGIN;
  process.env.RAZIEL_OPERATOR_LIVE_MODE = "false";
  process.env.RAZIEL_GATEWAY_INTERNAL_URL = "http://demo-gateway:3001";
}

async function login(): Promise<LoggedInSession> {
  const response = await loginRoute(
    new Request(`${ORIGIN}/api/operator/login`, {
      method: "POST",
      headers: { origin: ORIGIN },
      body: JSON.stringify({ password: PASSWORD }),
    }),
  );
  expect(response.status).toBe(200);
  const body = (await response.json()) as { readonly csrfToken: string };
  const cookie = response.headers.get("set-cookie")?.split(";", 1)[0];
  if (!cookie) {
    throw new Error("login did not set a cookie");
  }
  return { cookie, csrfToken: body.csrfToken };
}

function mutationRequest(session: LoggedInSession, body: unknown): Request {
  return new Request(`${ORIGIN}/api/reconciliation/query`, {
    method: "POST",
    headers: {
      cookie: session.cookie,
      origin: ORIGIN,
      "content-type": "application/json",
      [OPERATOR_CSRF_HEADER]: session.csrfToken,
    },
    body: JSON.stringify(body),
  });
}

async function prepareLiveCapability(jobId: string, resourceScope: string): Promise<void> {
  const agent = await createOperatorAgentSession(OPERATOR_ID);
  const binding: LiveCapabilityBinding = {
    action: "query",
    agentKeyThumbprint: agent.key_thumbprint,
    operatorId: OPERATOR_ID,
    purpose: "payment-reconciliation",
    resourceScope,
    resourceType: "dataset",
    snapshotId: "snapshot-live-001",
    taskId: "reconcile-batch-001",
  };
  registerLiveJob(jobId, binding);
  const access: PublishedAccessSelection = {
    ...publishedAccess(),
    agent: {
      ...publishedAccess().agent,
      key_thumbprint: agent.key_thumbprint,
      subject_id: agent.subject_id,
    },
    datasetId: resourceScope,
  };
  retainLiveCapability(capabilityJob(jobId, access, "live"), "cafe", binding);
}

function gatewaySuccess(jobId: string) {
  return {
    classification: "SYNTHETIC",
    dataset_id: DATASET_ID,
    row_count: 1,
    rows: [
      {
        transactionId: "txn-001",
        occurredAt: "2026-08-01T10:00:00Z",
        amountMinor: "12500",
        currency: "SGD",
        merchantReference: "merchant-a",
        accountReference: "account-1",
        reconciliationStatus: "UNMATCHED",
        sourceRecordDigest: "71".repeat(32),
        relationship_path: ["private"],
      },
    ],
    receipt: {
      format_version: 1,
      receipt_id: "receipt-001",
      receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
      request_id: "request-001",
      occurred_at: "2026-08-27T04:05:07Z",
      task_id: "reconcile-batch-001",
      capability_id: jobId,
      proof_job_id: jobId,
      policy_record_id: "policy-record-001",
      policy_hash: "72".repeat(32),
      snapshot_id: "snapshot-live-001",
      relationship_snapshot_root: "73".repeat(32),
      public_claims_digest: "74".repeat(32),
      proof_digest: "75".repeat(32),
      action: "query",
      resource_type: "dataset",
      resource_scope: DATASET_ID,
      outcome: "AUDIT_OUTCOME_ACCEPTED",
      reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
      raw_proof: "private-proof",
    },
    private_policy: { nodes: [] },
  };
}

function publishedAccess(): PublishedAccessSelection {
  return {
    agent: {
      key_thumbprint: "61".repeat(32),
      subject_type: "agent",
      subject_id: "61".repeat(32),
      created_at_unix_seconds: "1787800000",
    },
    agentId: "agent-reconciliation-01",
    datasetId: DATASET_ID,
    snapshotId: "snapshot-live-001",
    policyHash: "62".repeat(32),
    relationshipRoot: "63".repeat(32),
    snapshotExpiresAtUnixSeconds: "1787810000",
  };
}

function capabilityJob(
  jobId: string,
  access: PublishedAccessSelection,
  mode: "live" | "recorded",
): CapabilityJobProjection {
  return {
    job_id: jobId,
    state: "PROOF_JOB_STATE_SUCCEEDED",
    submitted_at: null,
    updated_at: null,
    public_error_code: "",
    metrics: null,
    capability: {
      capability_id: jobId,
      agent_key_thumbprint: access.agent.key_thumbprint,
      task_id: "reconcile-batch-001",
      purpose: "payment-reconciliation",
      action: "query",
      resource_type: "dataset",
      resource_scope: access.datasetId,
      policy_hash: access.policyHash,
      relationship_snapshot_root: access.relationshipRoot,
      snapshot_id: access.snapshotId,
      issued_at: null,
      expires_at: null,
      proof_system: "SP1_PLONK",
      verification_key_id: "sp1-vk-v1:test",
      proof_size_bytes: "2536",
    },
    mode,
    authorizes_requests: mode === "live",
    recorded_source: mode === "recorded" ? "recorded.json" : null,
  };
}

function restoreEnvironment(): void {
  for (const name of environmentNames) {
    const value = originalEnvironment[name];
    if (value === undefined) {
      delete process.env[name];
    } else {
      process.env[name] = value;
    }
  }
}
