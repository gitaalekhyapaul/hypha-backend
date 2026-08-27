import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";
import { POST as cloneRoute } from "../app/api/datasets/[datasetId]/clone/route.ts";
import { POST as finalizeRoute } from "../app/api/datasets/[datasetId]/finalize/route.ts";
import { GET as datasetsGetRoute, PUT as datasetsPutRoute } from "../app/api/datasets/route.ts";
import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import {
  derivePasswordVerifierForProvisioning,
  OPERATOR_CSRF_HEADER,
} from "../lib/operator-auth/index.ts";

const ORIGIN = "http://127.0.0.1:3000";
const PASSWORD = "dataset route password";
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

describe("M5 authenticated dataset facade", () => {
  it("does not read or forward an unauthenticated mutation body", async () => {
    await configureAuth();
    const fetchMock = vi.fn();
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const request = new Request(`${ORIGIN}/api/datasets`, { method: "PUT" });
    Object.defineProperty(request, "text", {
      value: async () => {
        throw new Error("body should not be read");
      },
    });

    const response = await datasetsPutRoute(request);
    expect(response.status).toBe(403);
    expect(await response.json()).toEqual({ error: "OPERATOR_ORIGIN_REJECTED" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("lists, saves, finalizes, and clones through fixed gateway paths", async () => {
    await configureAuth();
    const session = await login();
    const requests: Array<{ readonly method: string; readonly path: string }> = [];
    globalThis.fetch = vi.fn(async (input, init) => {
      const url = new URL(input instanceof Request ? input.url : input.toString());
      const method = init?.method ?? "GET";
      requests.push({ method, path: url.pathname });
      if (url.pathname.endsWith("/finalize")) {
        return Response.json(finalizedDataset());
      }
      if (url.pathname.endsWith("/clone")) {
        return Response.json(clonedDataset(), { status: 201 });
      }
      if (method === "PUT") {
        return Response.json(draftDataset());
      }
      return Response.json({ classification: "SYNTHETIC", datasets: [draftDataset()] });
    }) as typeof fetch;

    const listed = await datasetsGetRoute(
      new Request(`${ORIGIN}/api/datasets`, { headers: { cookie: session.cookie } }),
    );
    expect(listed.status).toBe(200);
    expect((await listed.json()).classification).toBe("SYNTHETIC");

    const saved = await datasetsPutRoute(
      mutationRequest("/api/datasets", session, "PUT", draftInput()),
    );
    expect(saved.status).toBe(200);
    expect((await saved.json()).status).toBe("DRAFT");

    const finalized = await finalizeRoute(
      mutationRequest("/api/datasets/reconciliation-demo/finalize", session),
      { params: Promise.resolve({ datasetId: "reconciliation-demo" }) },
    );
    expect(finalized.status).toBe(200);
    expect((await finalized.json()).finalization.rowCount).toBe(1);

    const cloned = await cloneRoute(
      mutationRequest("/api/datasets/reconciliation-demo/clone", session, "POST", {
        targetDatasetId: "reconciliation-demo-revision-2",
      }),
      { params: Promise.resolve({ datasetId: "reconciliation-demo" }) },
    );
    expect(cloned.status).toBe(201);
    expect((await cloned.json()).derivedFromDatasetId).toBe("reconciliation-demo");

    expect(requests).toEqual([
      { method: "GET", path: "/api/datasets" },
      { method: "PUT", path: "/api/datasets" },
      { method: "POST", path: "/api/datasets/reconciliation-demo/finalize" },
      { method: "POST", path: "/api/datasets/reconciliation-demo/clone" },
    ]);
  });

  it("rejects an upstream dataset response with an undeclared field", async () => {
    await configureAuth();
    const session = await login();
    globalThis.fetch = vi.fn(async () =>
      Response.json({
        classification: "SYNTHETIC",
        datasets: [{ ...draftDataset(), relationships: [] }],
      }),
    ) as typeof fetch;

    const response = await datasetsGetRoute(
      new Request(`${ORIGIN}/api/datasets`, { headers: { cookie: session.cookie } }),
    );
    expect(response.status).toBe(503);
    expect(await response.json()).toEqual({ error: "DATASET_UNAVAILABLE" });
  });
});

interface LoggedInSession {
  readonly cookie: string;
  readonly csrfToken: string;
}

async function configureAuth(): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "raziel-dataset-route-test-"));
  roots.push(root);
  await chmod(root, 0o700);
  const salt = Buffer.alloc(16, 0x33);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  const secretPath = join(root, "operator-secret.json");
  await writeFile(
    secretPath,
    `${JSON.stringify({
      format_version: 1,
      operator_id: "operator-dataset",
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x44).toString("base64url"),
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

function mutationRequest(
  path: string,
  session: LoggedInSession,
  method = "POST",
  body?: unknown,
): Request {
  return new Request(`${ORIGIN}${path}`, {
    method,
    headers: {
      cookie: session.cookie,
      origin: ORIGIN,
      [OPERATOR_CSRF_HEADER]: session.csrfToken,
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
}

function draftInput() {
  return {
    datasetId: "reconciliation-demo",
    sourceId: "portal-local",
    sourceLabel: "Portal-created demonstration rows",
    rows: [transactionDraft()],
  };
}

function transactionDraft() {
  return {
    transactionId: "txn-001",
    occurredAt: "2026-08-01T10:00:00Z",
    amountMinor: "12500",
    currency: "SGD",
    merchantReference: "merchant-a",
    accountReference: "account-1",
    transactionType: "payment",
    reconciliationStatus: "UNMATCHED",
  };
}

function transaction() {
  return {
    ...transactionDraft(),
    sourceRecordDigest: "11".repeat(32),
  };
}

function draftDataset() {
  return {
    formatVersion: 1,
    datasetId: "reconciliation-demo",
    revision: 1,
    source: {
      classification: "SYNTHETIC",
      sourceId: "portal-local",
      sourceLabel: "Portal-created demonstration rows",
    },
    status: "DRAFT",
    rows: [transaction()],
    createdAt: "2026-08-27T03:00:00Z",
    updatedAt: "2026-08-27T03:00:00Z",
  };
}

function finalizedDataset() {
  return {
    ...draftDataset(),
    status: "FINALIZED",
    updatedAt: "2026-08-27T03:01:00Z",
    finalization: {
      finalizedAt: "2026-08-27T03:01:00Z",
      checksum: "22".repeat(32),
      schemaFingerprint: "33".repeat(32),
      rowCount: 1,
    },
  };
}

function clonedDataset() {
  return {
    ...draftDataset(),
    datasetId: "reconciliation-demo-revision-2",
    revision: 2,
    derivedFromDatasetId: "reconciliation-demo",
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
