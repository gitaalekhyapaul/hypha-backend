import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";
import { datasetChecksum } from "../../demo-gateway/src/reconciliation/model.ts";
import {
  DatasetConflictError,
  DatasetNotFoundError,
  SyntheticDatasetStore,
} from "../../demo-gateway/src/reconciliation/store.ts";
import { POST as seedRoute } from "../app/api/datasets/seed-reconciliation/route.ts";
import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import {
  DatasetClientError,
  type SyntheticDataset,
  SyntheticDatasetClient,
  type SyntheticDatasetDraft,
} from "../lib/dataset-client/index.ts";
import {
  reconciliationSeedDrafts,
  seedReconciliationLedgers,
} from "../lib/dataset-client/reconciliation-seed.ts";
import {
  derivePasswordVerifierForProvisioning,
  OPERATOR_CSRF_HEADER,
} from "../lib/operator-auth/index.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";
import { seedReconciliationLedgers as seedFromBrowser } from "../lib/portal-client/datasets.ts";

const ORIGIN = "http://127.0.0.1:3000";
const GATEWAY = "http://demo-gateway:3001";
const NOW = "2026-10-01T06:00:00Z";
const PASSWORD = "synthetic seed route password";
const roots: string[] = [];
const originalFetch = globalThis.fetch;
const environmentNames = [
  "RAZIEL_OPERATOR_SECRET_PATH",
  "RAZIEL_OPERATOR_EXPECTED_ORIGIN",
  "RAZIEL_OPERATOR_LIVE_MODE",
  "RAZIEL_GATEWAY_INTERNAL_URL",
] as const;
const originalEnvironment = Object.fromEntries(
  environmentNames.map((name) => [name, process.env[name]]),
);

afterEach(async () => {
  globalThis.fetch = originalFetch;
  vi.restoreAllMocks();
  for (const name of environmentNames) {
    const value = originalEnvironment[name];
    if (value === undefined) delete process.env[name];
    else process.env[name] = value;
  }
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

describe("deterministic synthetic reconciliation seed", () => {
  it("maps exact, missing, and ambiguous candidates through the shared reference", () => {
    const [processor, bank] = seedPair();
    expect(reconciliationSeedDrafts()).toEqual(reconciliationSeedDrafts());
    expect(processor.datasetId).toBe("processor-demo-v1");
    expect(bank.datasetId).toBe("bank-demo-v1");
    expect(processor.rows.map((row) => row.amountMinor)).toEqual(["12500", "4899", "9900"]);
    expect(bank.rows.map((row) => row.amountMinor)).toEqual(["12500", "9900", "9900"]);
    const candidates = processor.rows.map(
      (payment) =>
        bank.rows.filter(
          (settlement) =>
            settlement.merchantReference === payment.merchantReference &&
            settlement.currency === payment.currency &&
            settlement.amountMinor === payment.amountMinor,
        ).length,
    );
    expect(candidates).toEqual([1, 0, 2]);
    for (const draft of [processor, bank]) {
      expect(draft.rows).toHaveLength(3);
      expect(draft.sourceId).toBe("reconciliation-ledgers-v1");
      expect(draft.sourceLabel).toContain("Synthetic");
      expect(new Set(draft.rows.map((row) => row.transactionId)).size).toBe(3);
      for (const row of draft.rows) {
        expect(row.amountMinor).toMatch(/^(0|[1-9][0-9]*)$/);
        expect(row.currency).toBe("SGD");
        expect(row.occurredAt).toMatch(/^2026-10-01T\d{2}:\d{2}:\d{2}Z$/);
        expect(row.reconciliationStatus).toBe("UNMATCHED");
      }
    }
  });

  it("returns fresh draft data so caller changes cannot modify the generator", () => {
    const drafts = reconciliationSeedDrafts();
    const expected = reconciliationSeedDrafts();
    Reflect.set(drafts[0]?.rows[0] ?? {}, "amountMinor", "1");
    expect(reconciliationSeedDrafts()).toEqual(expected);
  });

  it("creates only two DRAFT datasets through the client and real store", async () => {
    const fixture = await datasetFixture();
    const result = await seedReconciliationLedgers(fixture.client);
    expect(result.classification).toBe("SYNTHETIC");
    expect(result.createdDatasetIds).toEqual(["processor-demo-v1", "bank-demo-v1"]);
    expect(result.preservedDatasetIds).toEqual([]);
    expect(await fixture.store.list()).toHaveLength(2);
    for (const dataset of result.datasets) {
      expect(dataset.status).toBe("DRAFT");
      expect(dataset.revision).toBe(1);
      expect(dataset.finalization).toBeUndefined();
      expect(dataset.source.classification).toBe("SYNTHETIC");
      expect(dataset.rows).toHaveLength(3);
      expect(dataset.rows.every((row) => /^[0-9a-f]{64}$/.test(row.sourceRecordDigest))).toBe(true);
    }
    expect(fixture.writes).toEqual(["processor-demo-v1", "bank-demo-v1"]);
    for (const draft of seedPair()) {
      expect(fixture.transport).toHaveBeenCalledWith(
        new URL("/api/datasets", GATEWAY),
        expect.objectContaining({ method: "POST", body: JSON.stringify(draft) }),
      );
    }
  });

  it("retains row digests and checksums across independent imports", async () => {
    const first = await datasetFixture();
    const second = await datasetFixture();
    const left = await seedReconciliationLedgers(first.client);
    const right = await seedReconciliationLedgers(second.client);
    expect(left.datasets).toEqual(right.datasets);
    expect(left.datasets.map(datasetChecksum)).toEqual(right.datasets.map(datasetChecksum));
  });

  it("preserves both existing drafts byte-for-byte on a repeat call", async () => {
    const fixture = await datasetFixture();
    const original = await seedReconciliationLedgers(fixture.client);
    const before = await readFile(fixture.filePath, "utf8");
    const repeated = await seedReconciliationLedgers(fixture.client);
    expect(repeated.datasets).toEqual(original.datasets);
    expect(repeated.createdDatasetIds).toEqual([]);
    expect(repeated.preservedDatasetIds).toEqual(["processor-demo-v1", "bank-demo-v1"]);
    expect(await readFile(fixture.filePath, "utf8")).toBe(before);
    expect(fixture.writes).toHaveLength(2);
  });

  it("preserves finalized exact seed revisions without re-finalizing", async () => {
    const fixture = await datasetFixture();
    await seedReconciliationLedgers(fixture.client);
    for (const draft of seedPair()) await fixture.store.finalize(draft.datasetId, NOW);
    const before = await readFile(fixture.filePath, "utf8");
    const repeated = await seedReconciliationLedgers(fixture.client);
    expect(repeated.datasets.every((dataset) => dataset.status === "FINALIZED")).toBe(true);
    expect(await readFile(fixture.filePath, "utf8")).toBe(before);
    expect(fixture.writes).toHaveLength(2);
  });

  it("creates a missing partner without updating an exact preexisting seed", async () => {
    const fixture = await datasetFixture();
    const [processor] = seedPair();
    const existing = await fixture.store.putDraft(processor, NOW);
    const result = await seedReconciliationLedgers(fixture.client);
    expect(result.createdDatasetIds).toEqual(["bank-demo-v1"]);
    expect(result.preservedDatasetIds).toEqual(["processor-demo-v1"]);
    expect(await fixture.store.get(processor.datasetId)).toEqual(existing);
    expect(fixture.writes).toEqual(["bank-demo-v1"]);
  });

  it.each([
    ["source ID", { sourceId: "operator-local" }],
    ["source label", { sourceLabel: "Operator-edited synthetic draft" }],
    ["amount", { amountMinor: "99999" }],
    ["currency", { currency: "USD" }],
    ["reference", { merchantReference: "operator-reference" }],
    ["timestamp", { occurredAt: "2026-10-02T10:00:00Z" }],
    ["account", { accountReference: "operator-account" }],
    ["type", { transactionType: "refund" }],
    ["status", { reconciliationStatus: "MATCHED" }],
    ["transaction ID", { transactionId: "operator-transaction" }],
  ] as const)(
    "preserves a preexisting draft with changed %s and returns a conflict",
    async (_name, change) => {
      const fixture = await datasetFixture();
      const [processor] = seedPair();
      const modified: SyntheticDatasetDraft =
        "sourceId" in change || "sourceLabel" in change
          ? { ...processor, ...change }
          : {
              ...processor,
              rows: processor.rows.map((row, index) => (index === 0 ? { ...row, ...change } : row)),
            };
      await fixture.store.putDraft(modified, NOW);
      const before = await readFile(fixture.filePath, "utf8");
      await expect(seedReconciliationLedgers(fixture.client)).rejects.toMatchObject({
        code: "DATASET_CONFLICT",
        status: 409,
        publicMessage: expect.stringContaining("processor-demo-v1"),
      });
      expect(await readFile(fixture.filePath, "utf8")).toBe(before);
      expect(await fixture.store.list()).toHaveLength(1);
      expect(fixture.writes).toEqual([]);
    },
  );

  it.each(["added", "removed"])("preserves a draft with an %s row", async (kind) => {
    const fixture = await datasetFixture();
    const [processor] = seedPair();
    const firstRow = processor.rows[0];
    if (!firstRow) throw new Error("processor fixture row is missing");
    const rows =
      kind === "added"
        ? [...processor.rows, { ...firstRow, transactionId: "operator-extra" }]
        : processor.rows.slice(1);
    await fixture.store.putDraft({ ...processor, rows }, NOW);
    const before = await readFile(fixture.filePath, "utf8");
    await expect(seedReconciliationLedgers(fixture.client)).rejects.toBeInstanceOf(
      DatasetClientError,
    );
    expect(await readFile(fixture.filePath, "utf8")).toBe(before);
    expect(fixture.writes).toEqual([]);
  });

  it("validates the bank before creating an absent processor ledger", async () => {
    const fixture = await datasetFixture();
    const [, bank] = seedPair();
    await fixture.store.putDraft({ ...bank, sourceId: "operator-bank" }, NOW);
    const before = await readFile(fixture.filePath, "utf8");
    await expect(seedReconciliationLedgers(fixture.client)).rejects.toMatchObject({
      code: "DATASET_CONFLICT",
      publicMessage: expect.stringContaining("bank-demo-v1"),
    });
    expect(await readFile(fixture.filePath, "utf8")).toBe(before);
    expect(fixture.writes).toEqual([]);
  });

  it("leaves unrelated operator datasets unchanged", async () => {
    const fixture = await datasetFixture();
    const [processor] = seedPair();
    const other = await fixture.store.putDraft({ ...processor, datasetId: "operator-other" }, NOW);
    await seedReconciliationLedgers(fixture.client);
    expect(await fixture.store.get(other.datasetId)).toEqual(other);
    expect(await fixture.store.list()).toHaveLength(3);
  });

  it("retries a partial gateway failure without replacing the created partner", async () => {
    const fixture = await datasetFixture();
    let failBank = true;
    const client = {
      get: (id: string) => fixture.client.get(id),
      createDraft: (draft: SyntheticDatasetDraft) => {
        if (draft.datasetId === "bank-demo-v1" && failBank) {
          failBank = false;
          throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
        }
        return fixture.client.createDraft(draft);
      },
    };
    await expect(seedReconciliationLedgers(client)).rejects.toMatchObject({ status: 503 });
    const existing = await fixture.store.get("processor-demo-v1");
    const repeated = await seedReconciliationLedgers(client);
    expect(repeated.createdDatasetIds).toEqual(["bank-demo-v1"]);
    expect(await fixture.store.get("processor-demo-v1")).toEqual(existing);
    expect(fixture.writes).toEqual(["processor-demo-v1", "bank-demo-v1"]);
  });

  it("does not treat an unavailable read as an absent dataset", async () => {
    const fixture = await datasetFixture();
    const get = vi.fn().mockRejectedValue(new DatasetClientError("DATASET_UNAVAILABLE", 503));
    await expect(
      seedReconciliationLedgers({
        get,
        createDraft: fixture.client.createDraft.bind(fixture.client),
      }),
    ).rejects.toMatchObject({ status: 503 });
    expect(fixture.writes).toEqual([]);
  });

  it("rechecks an absent ID and preserves a draft created after pair validation", async () => {
    const fixture = await datasetFixture();
    const [processor] = seedPair();
    let reads = 0;
    const get = async (id: string) => {
      if (id === processor.datasetId && ++reads === 2) {
        await fixture.store.putDraft({ ...processor, sourceId: "operator-race" }, NOW);
      }
      return fixture.client.get(id);
    };
    await expect(
      seedReconciliationLedgers({
        get,
        createDraft: fixture.client.createDraft.bind(fixture.client),
      }),
    ).rejects.toMatchObject({ code: "DATASET_CONFLICT" });
    expect((await fixture.store.get(processor.datasetId)).source.sourceId).toBe("operator-race");
    expect(fixture.writes).toEqual([]);
  });

  it.each(["DRAFT", "FINALIZED"])(
    "preserves an operator %s created after the final read and before the seed write",
    async (status) => {
      const fixture = await datasetFixture();
      const [processor] = seedPair();
      let operatorDataset: SyntheticDataset | undefined;
      const client = {
        get: (id: string) => fixture.client.get(id),
        createDraft: async (draft: SyntheticDatasetDraft) => {
          if (draft.datasetId === processor.datasetId) {
            operatorDataset = await fixture.store.putDraft(
              {
                ...processor,
                sourceId: "operator-race",
                rows: processor.rows.map((row, index) =>
                  index === 0 ? { ...row, amountMinor: "1" } : row,
                ),
              },
              NOW,
            );
            if (status === "FINALIZED") {
              operatorDataset = await fixture.store.finalize(processor.datasetId, NOW);
            }
          }
          return fixture.client.createDraft(draft);
        },
      };
      let failure: unknown;
      try {
        await seedReconciliationLedgers(client);
      } catch (error) {
        failure = error;
      }
      expect(operatorDataset).toBeDefined();
      expect(await fixture.store.get(processor.datasetId)).toEqual(operatorDataset);
      expect(failure).toMatchObject({ code: "DATASET_CONFLICT", status: 409 });
      expect(fixture.writes).toEqual([]);
    },
  );

  it.each(["DRAFT", "FINALIZED"])(
    "preserves a concurrent exact %s seed on create conflict",
    async (status) => {
      const fixture = await datasetFixture();
      const [processor] = seedPair();
      let concurrentDataset: SyntheticDataset | undefined;
      const client = {
        get: (id: string) => fixture.client.get(id),
        createDraft: async (draft: SyntheticDatasetDraft) => {
          if (draft.datasetId === processor.datasetId) {
            concurrentDataset = await fixture.store.createDraft(draft, NOW);
            if (status === "FINALIZED") {
              concurrentDataset = await fixture.store.finalize(draft.datasetId, NOW);
            }
          }
          return fixture.client.createDraft(draft);
        },
      };
      const result = await seedReconciliationLedgers(client);
      expect(result.createdDatasetIds).toEqual(["bank-demo-v1"]);
      expect(result.preservedDatasetIds).toEqual(["processor-demo-v1"]);
      expect(result.datasets[0]).toEqual(concurrentDataset);
      expect(await fixture.store.get(processor.datasetId)).toEqual(concurrentDataset);
      expect(fixture.writes).toEqual(["bank-demo-v1"]);
    },
  );

  it("retries a lost create response without replacing the committed draft", async () => {
    const fixture = await datasetFixture();
    let loseResponse = true;
    const client = {
      get: (id: string) => fixture.client.get(id),
      createDraft: async (draft: SyntheticDatasetDraft) => {
        const created = await fixture.client.createDraft(draft);
        if (loseResponse) {
          loseResponse = false;
          throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
        }
        return created;
      },
    };
    await expect(seedReconciliationLedgers(client)).rejects.toMatchObject({ status: 503 });
    const committed = await fixture.store.get("processor-demo-v1");
    const result = await seedReconciliationLedgers(client);
    expect(result.createdDatasetIds).toEqual(["bank-demo-v1"]);
    expect(result.preservedDatasetIds).toEqual(["processor-demo-v1"]);
    expect(await fixture.store.get(committed.datasetId)).toEqual(committed);
    expect(fixture.writes).toEqual(["processor-demo-v1", "bank-demo-v1"]);
  });

  it("serializes concurrent seed calls and performs only two writes", async () => {
    const fixture = await datasetFixture();
    const [first, second] = await Promise.all([
      seedReconciliationLedgers(fixture.client),
      seedReconciliationLedgers(fixture.client),
    ]);
    expect(first.createdDatasetIds).toHaveLength(2);
    expect(second.createdDatasetIds).toHaveLength(0);
    expect(fixture.writes).toHaveLength(2);
  });

  it("supports normal editing, finalization, and stored reference queries", async () => {
    const fixture = await datasetFixture();
    await seedReconciliationLedgers(fixture.client);
    const [, bank] = seedPair();
    const edited = await fixture.client.putDraft({ ...bank, rows: bank.rows.slice(0, 2) });
    expect(edited.status).toBe("DRAFT");
    const finalized = await fixture.store.finalize(bank.datasetId, NOW);
    expect(finalized.finalization?.rowCount).toBe(2);
    expect(finalized.finalization?.checksum).toBe(datasetChecksum(finalized));
    const candidates = await fixture.store.query(bank.datasetId, {
      fromOccurredAt: "2026-10-01T00:00:00Z",
      toOccurredAt: "2026-10-01T23:59:59Z",
      merchantReference: "recon-ambiguous-003",
      limit: 10,
    });
    expect(candidates.map((row) => row.transactionId)).toEqual(["bank-002"]);
    await expect(seedReconciliationLedgers(fixture.client)).rejects.toMatchObject({
      code: "DATASET_CONFLICT",
    });
  });
});

describe("authenticated reconciliation seed facade", () => {
  it("rejects unauthenticated and cross-origin calls before any gateway access", async () => {
    await configureAuth();
    const transport = vi.fn();
    globalThis.fetch = transport as typeof fetch;
    const noOrigin = await seedRoute(
      new Request(`${ORIGIN}/api/datasets/seed-reconciliation`, { method: "POST" }),
    );
    expect(noOrigin.status).toBe(403);
    const noCookie = await seedRoute(
      new Request(`${ORIGIN}/api/datasets/seed-reconciliation`, {
        method: "POST",
        headers: { origin: ORIGIN },
      }),
    );
    expect(noCookie.status).toBe(401);
    const session = await login();
    const crossOrigin = await seedRoute(seedRequest(session, { origin: "https://other.example" }));
    expect(crossOrigin.status).toBe(403);
    expect(transport).not.toHaveBeenCalled();
  });

  it.each(["", "wrong-token"])("rejects missing or incorrect CSRF token %s", async (token) => {
    await configureAuth();
    const session = await login();
    const transport = vi.fn();
    globalThis.fetch = transport as typeof fetch;
    const response = await seedRoute(seedRequest(session, { [OPERATOR_CSRF_HEADER]: token }));
    expect(response.status).toBe(403);
    expect(transport).not.toHaveBeenCalled();
  });

  it("creates and lists both ledgers through an authorized POST, with no finalization", async () => {
    await configureAuth();
    const session = await login();
    const fixture = await datasetFixture();
    globalThis.fetch = fixture.transport;
    const response = await seedRoute(seedRequest(session));
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toContain("no-store");
    const body = await response.json();
    expect(body.createdDatasetIds).toEqual(["processor-demo-v1", "bank-demo-v1"]);
    expect(body.datasets.every((dataset: { status: string }) => dataset.status === "DRAFT")).toBe(
      true,
    );
    const listed = await fixture.client.list();
    expect(listed.datasets).toHaveLength(2);
    const repeated = await seedRoute(seedRequest(session));
    expect(repeated.status).toBe(200);
    expect((await repeated.json()).createdDatasetIds).toEqual([]);
    expect(fixture.writes).toHaveLength(2);
  });

  it("returns a meaningful 409 and preserves a modified preexisting dataset", async () => {
    await configureAuth();
    const session = await login();
    const fixture = await datasetFixture();
    const [, bank] = seedPair();
    await fixture.store.putDraft({ ...bank, sourceLabel: "Operator-edited bank draft" }, NOW);
    const before = await readFile(fixture.filePath, "utf8");
    globalThis.fetch = fixture.transport;
    const response = await seedRoute(seedRequest(session));
    expect(response.status).toBe(409);
    expect(await response.json()).toEqual({
      error: "DATASET_CONFLICT",
      message: expect.stringContaining("bank-demo-v1 already exists"),
    });
    expect(await readFile(fixture.filePath, "utf8")).toBe(before);
    expect(fixture.writes).toEqual([]);
  });
});

describe("portal seed request", () => {
  it("uses the fixed POST route with same-origin credentials and CSRF", async () => {
    const fixture = await datasetFixture();
    const result = await seedReconciliationLedgers(fixture.client);
    const transport = vi.fn(async () => Response.json(result));
    globalThis.fetch = transport as typeof fetch;
    expect(await seedFromBrowser("csrf-test-token")).toEqual(result);
    expect(transport).toHaveBeenCalledWith("/api/datasets/seed-reconciliation", {
      method: "POST",
      headers: { [OPERATOR_CSRF_HEADER]: "csrf-test-token" },
      credentials: "same-origin",
      cache: "no-store",
      signal: expect.any(AbortSignal),
    });
  });

  it("retains the conflict message for the dataset workspace", async () => {
    globalThis.fetch = vi.fn(async () =>
      Response.json(
        { error: "DATASET_CONFLICT", message: "bank-demo-v1 was preserved" },
        { status: 409 },
      ),
    ) as typeof fetch;
    await expect(seedFromBrowser("csrf-token")).rejects.toMatchObject({
      code: "DATASET_CONFLICT",
      status: 409,
      message: "bank-demo-v1 was preserved",
    });
  });

  it("retains session expiry with an empty 401 response", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 401 })) as typeof fetch;
    await expect(seedFromBrowser("csrf-token")).rejects.toMatchObject({ status: 401 });
  });

  it("reports unavailable transport without inventing seed results", async () => {
    globalThis.fetch = vi.fn(async () => {
      throw new Error("offline");
    }) as typeof fetch;
    await expect(seedFromBrowser("csrf-token")).rejects.toBeInstanceOf(PortalRequestError);
  });
});

function seedPair(): readonly [SyntheticDatasetDraft, SyntheticDatasetDraft] {
  const [processor, bank] = reconciliationSeedDrafts();
  if (!processor || !bank) throw new Error("seed pair is missing");
  return [processor, bank];
}

async function datasetFixture() {
  const root = await mkdtemp(join(tmpdir(), "raziel-reconciliation-seed-"));
  roots.push(root);
  const filePath = join(root, "transactions.json");
  const store = new SyntheticDatasetStore(filePath);
  const writes: string[] = [];
  const transport: typeof fetch = vi.fn(async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : input.toString());
    expect(url.origin).toBe(GATEWAY);
    const method = init?.method ?? "GET";
    if ((method === "POST" || method === "PUT") && url.pathname === "/api/datasets") {
      const draft = JSON.parse(String(init?.body)) as SyntheticDatasetDraft;
      try {
        const dataset = await (method === "POST"
          ? store.createDraft(draft, NOW)
          : store.putDraft(draft, NOW));
        writes.push(draft.datasetId);
        return Response.json(dataset, { status: method === "POST" ? 201 : 200 });
      } catch (error) {
        if (error instanceof DatasetConflictError) {
          return Response.json(
            { error: "DATASET_CONFLICT", message: error.message },
            { status: 409 },
          );
        }
        throw error;
      }
    }
    if (method === "GET" && url.pathname === "/api/datasets") {
      return Response.json({ classification: "SYNTHETIC", datasets: await store.list() });
    }
    if (method === "GET" && url.pathname.startsWith("/api/datasets/")) {
      try {
        return Response.json(
          await store.get(decodeURIComponent(url.pathname.slice("/api/datasets/".length))),
        );
      } catch (error) {
        if (error instanceof DatasetNotFoundError) {
          return Response.json({ error: "DATASET_NOT_FOUND" }, { status: 404 });
        }
        throw error;
      }
    }
    throw new Error(`unexpected seed gateway call ${method} ${url.pathname}`);
  });
  const client = new SyntheticDatasetClient({ baseUrl: GATEWAY }, { fetch: transport });
  return { client, store, transport, filePath, writes };
}

interface LoggedInSession {
  readonly cookie: string;
  readonly csrfToken: string;
}

async function configureAuth(): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "raziel-seed-auth-"));
  roots.push(root);
  await chmod(root, 0o700);
  const salt = Buffer.alloc(16, 0x33);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  const secretPath = join(root, "operator-secret.json");
  await writeFile(
    secretPath,
    JSON.stringify({
      format_version: 1,
      operator_id: "operator-seed",
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x44).toString("base64url"),
    }),
    { mode: 0o600 },
  );
  process.env.RAZIEL_OPERATOR_SECRET_PATH = secretPath;
  process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN = ORIGIN;
  process.env.RAZIEL_OPERATOR_LIVE_MODE = "false";
  process.env.RAZIEL_GATEWAY_INTERNAL_URL = GATEWAY;
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
  const body = await response.json();
  const cookie = response.headers.get("set-cookie")?.split(";", 1)[0];
  if (!cookie) throw new Error("login did not set a cookie");
  return { cookie, csrfToken: body.csrfToken as string };
}

function seedRequest(session: LoggedInSession, overrides: Record<string, string> = {}): Request {
  return new Request(`${ORIGIN}/api/datasets/seed-reconciliation`, {
    method: "POST",
    headers: {
      cookie: session.cookie,
      origin: ORIGIN,
      [OPERATOR_CSRF_HEADER]: session.csrfToken,
      ...overrides,
    },
  });
}
