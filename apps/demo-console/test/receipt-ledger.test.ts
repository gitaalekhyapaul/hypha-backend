import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import { GET as receiptsRoute } from "../app/api/receipts/route.ts";
import type { PublicAuditReceipt } from "../lib/receipt-ledger/index.ts";
import { derivePasswordVerifierForProvisioning } from "../lib/operator-auth/index.ts";

const ORIGIN = "http://127.0.0.1:3000";
const PASSWORD = "receipt ledger password";
const OPERATOR_ID = "operator-receipts";
const roots: string[] = [];
const environmentNames = [
  "RAZIEL_OPERATOR_SECRET_PATH",
  "RAZIEL_OPERATOR_EXPECTED_ORIGIN",
  "RAZIEL_OPERATOR_LIVE_MODE",
  "RAZIEL_RECEIPT_STORE_PATH",
] as const;
const originalEnvironment = Object.fromEntries(
  environmentNames.map((name) => [name, process.env[name]]),
) as Record<(typeof environmentNames)[number], string | undefined>;

afterEach(async () => {
  restoreEnvironment();
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

describe("B-0055 public receipt ledger", () => {
  it("requires an operator session before reading receipt storage", async () => {
    await configureAuth([]);

    const response = await receiptsRoute(new Request(`${ORIGIN}/api/receipts`));

    expect(response.status).toBe(401);
    expect(await response.json()).toEqual({ error: "OPERATOR_UNAUTHORIZED" });
  });

  it("returns an empty public ledger when the receipt file is absent", async () => {
    await configureAuth(undefined);
    const cookie = await login();

    const response = await receiptsRoute(authenticatedRequest("/api/receipts", cookie));

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      classification: "PUBLIC_AUDIT",
      total_count: 0,
      filtered_count: 0,
      receipts: [],
    });
  });

  it("filters exact public identifiers and exports only reserialized JSONL", async () => {
    const accepted = receipt({ capability_id: "11".repeat(32), receipt_id: "receipt-accepted" });
    const replayed = receipt({
      capability_id: "22".repeat(32),
      outcome: "AUDIT_OUTCOME_REJECTED",
      reason_code: "PUBLIC_ERROR_CODE_REQUEST_REPLAYED",
      receipt_id: "receipt-replayed",
    });
    await configureAuth([accepted, replayed]);
    const cookie = await login();
    const query = `field=capability_id&value=${replayed.capability_id}`;

    const response = await receiptsRoute(authenticatedRequest(`/api/receipts?${query}`, cookie));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      classification: "PUBLIC_AUDIT",
      total_count: 2,
      filtered_count: 1,
      receipts: [replayed],
    });

    const exported = await receiptsRoute(
      authenticatedRequest(`/api/receipts?format=jsonl&${query}`, cookie),
    );
    expect(exported.status).toBe(200);
    expect(exported.headers.get("content-disposition")).toBe(
      'attachment; filename="raziel-public-receipts.jsonl"',
    );
    expect(await exported.text()).toBe(`${JSON.stringify(replayed)}\n`);
    expect(JSON.stringify(replayed)).not.toContain("request_body");
    expect(JSON.stringify(replayed)).not.toContain("capability_proto_hex");
  });

  it("rejects unknown, private, and oversized stored fields", async () => {
    const privateRecord = { ...receipt(), witness: { path: [] } };
    await configureAuthRaw(`${JSON.stringify(privateRecord)}\n`);
    const cookie = await login();
    const privateResponse = await receiptsRoute(authenticatedRequest("/api/receipts", cookie));
    expect(privateResponse.status).toBe(503);
    expect(await privateResponse.json()).toEqual({ error: "RECEIPT_LEDGER_UNAVAILABLE" });

    process.env.RAZIEL_RECEIPT_STORE_PATH = await writeReceiptFile(
      `${JSON.stringify(receipt({ resource_scope: "x".repeat(5_000) }))}\n`,
    );
    const oversizedResponse = await receiptsRoute(authenticatedRequest("/api/receipts", cookie));
    expect(oversizedResponse.status).toBe(503);

    const invalidQuery = await receiptsRoute(
      authenticatedRequest("/api/receipts?field=proof_digest&value=abc", cookie),
    );
    expect(invalidQuery.status).toBe(400);
    expect(await invalidQuery.json()).toEqual({ error: "INVALID_RECEIPT_QUERY" });
  });
});

async function configureAuth(receipts: readonly PublicAuditReceipt[] | undefined): Promise<void> {
  await configureAuthRaw(receipts?.map((entry) => JSON.stringify(entry)).join("\n") ?? undefined);
}

async function configureAuthRaw(receiptText: string | undefined): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "raziel-receipt-ledger-test-"));
  roots.push(root);
  await chmod(root, 0o700);
  const salt = Buffer.alloc(16, 0x71);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  const secretPath = join(root, "operator-secret.json");
  await writeFile(
    secretPath,
    `${JSON.stringify({
      format_version: 1,
      operator_id: OPERATOR_ID,
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x72).toString("base64url"),
    })}\n`,
    { mode: 0o600 },
  );
  process.env.RAZIEL_OPERATOR_SECRET_PATH = secretPath;
  process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN = ORIGIN;
  process.env.RAZIEL_OPERATOR_LIVE_MODE = "false";
  process.env.RAZIEL_RECEIPT_STORE_PATH = join(root, "receipts.jsonl");
  if (receiptText !== undefined) {
    await writeFile(
      process.env.RAZIEL_RECEIPT_STORE_PATH,
      receiptText.length === 0 || receiptText.endsWith("\n") ? receiptText : `${receiptText}\n`,
      { mode: 0o600 },
    );
  }
}

async function writeReceiptFile(text: string): Promise<string> {
  const root = roots.at(-1);
  if (!root) {
    throw new Error("receipt test root is missing");
  }
  const filePath = join(root, `receipts-${Date.now()}.jsonl`);
  await writeFile(filePath, text, { mode: 0o600 });
  return filePath;
}

async function login(): Promise<string> {
  const response = await loginRoute(
    new Request(`${ORIGIN}/api/operator/login`, {
      method: "POST",
      headers: { origin: ORIGIN },
      body: JSON.stringify({ password: PASSWORD }),
    }),
  );
  expect(response.status).toBe(200);
  const cookie = response.headers.get("set-cookie")?.split(";", 1)[0];
  if (!cookie) {
    throw new Error("login did not set a cookie");
  }
  return cookie;
}

function authenticatedRequest(path: string, cookie: string): Request {
  return new Request(`${ORIGIN}${path}`, { headers: { cookie } });
}

function receipt(overrides: Partial<PublicAuditReceipt> = {}): PublicAuditReceipt {
  return {
    format_version: 1,
    receipt_id: "receipt-v1:public",
    receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
    occurred_at: "2026-08-30T07:00:00Z",
    request_id: "request-v1:public",
    task_id: "reconcile-batch-001",
    capability_id: "11".repeat(32),
    proof_job_id: "11".repeat(32),
    policy_record_id: "policy-record-1",
    policy_hash: "22".repeat(32),
    snapshot_id: "snapshot-1",
    relationship_snapshot_root: "33".repeat(32),
    public_claims_digest: "44".repeat(32),
    proof_digest: "55".repeat(32),
    action: "query",
    resource_type: "dataset",
    resource_scope: "reconciliation-demo",
    outcome: "AUDIT_OUTCOME_ACCEPTED",
    reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
    ...overrides,
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
