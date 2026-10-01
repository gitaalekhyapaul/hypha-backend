import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";

import { POST as createRoute, PUT as putRoute } from "../app/api/datasets/route.ts";

import {
  DATASET_CLASSIFICATION,
  type DatasetDraftInput,
  MAX_QUERY_RANGE_SECONDS,
  ReconciliationValidationError,
  type SyntheticTransactionInput,
  schemaFingerprint,
} from "../src/reconciliation/model.ts";
import * as datasetRuntime from "../src/reconciliation/runtime.ts";
import { DatasetConflictError, SyntheticDatasetStore } from "../src/reconciliation/store.ts";

const temporaryDirectories: string[] = [];
const NOW = "2026-08-27T03:00:00Z";

afterEach(async () => {
  vi.restoreAllMocks();
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => rm(directory, { recursive: true, force: true })),
  );
});

describe("synthetic reconciliation dataset store", () => {
  it.each(["DRAFT", "FINALIZED"])(
    "rejects exact and changed creates without writing an existing %s",
    async (status) => {
      const { store, filePath } = await temporaryStore();
      const input = createOnlyDraft();
      let original = await store.createDraft(input, NOW);
      if (status === "FINALIZED") {
        original = await store.finalize(input.datasetId, NOW);
      }
      const before = await readFile(filePath, "utf8");
      for (const attempt of [input, { ...input, sourceId: "operator-edited" }]) {
        await expect(store.createDraft(attempt, "2026-08-27T03:02:00Z")).rejects.toBeInstanceOf(
          DatasetConflictError,
        );
        expect(await readFile(filePath, "utf8")).toBe(before);
        expect(await store.get(input.datasetId)).toEqual(original);
      }
    },
  );

  it("admits one concurrent create and rejects the competing create without replacement", async () => {
    const { store } = await temporaryStore();
    const input = createOnlyDraft();
    const results = await Promise.allSettled([
      store.createDraft(input, NOW),
      store.createDraft({ ...input, sourceId: "competing-source" }, NOW),
    ]);
    expect(results[0]).toMatchObject({ status: "fulfilled" });
    expect(results[1]).toMatchObject({
      status: "rejected",
      reason: expect.any(DatasetConflictError),
    });
    expect(await store.list()).toHaveLength(1);
    expect((await store.get(input.datasetId)).source.sourceId).toBe(input.sourceId);
  });

  it("keeps an operator write queued before create-only and leaves operator PUT editable", async () => {
    const { store } = await temporaryStore();
    const input = createOnlyDraft();
    const operatorInput = { ...input, sourceId: "operator-local" };
    const results = await Promise.allSettled([
      store.putDraft(operatorInput, NOW),
      store.createDraft(input, NOW),
    ]);
    expect(results[0]).toMatchObject({ status: "fulfilled" });
    expect(results[1]).toMatchObject({
      status: "rejected",
      reason: expect.any(DatasetConflictError),
    });
    expect((await store.get(input.datasetId)).source.sourceId).toBe("operator-local");
    const edited = await store.putDraft({ ...operatorInput, rows: [secondRow()] }, NOW);
    expect(await store.get(input.datasetId)).toEqual(edited);
    expect(edited.rows[0]?.transactionId).toBe("txn-002");
  });

  it("rejects invalid new creates without a write and permits a later valid create", async () => {
    const { store } = await temporaryStore();
    const input = createOnlyDraft();
    await expect(
      store.createDraft({ ...input, rows: [{ ...firstRow(), amountMinor: "01" }] }, NOW),
    ).rejects.toBeInstanceOf(ReconciliationValidationError);
    expect(await store.list()).toEqual([]);
    expect((await store.createDraft(input, NOW)).status).toBe("DRAFT");
  });

  it("returns 201 for create-only POST, 409 for existing IDs, and keeps PUT edits unchanged", async () => {
    const { store, filePath } = await temporaryStore();
    vi.spyOn(datasetRuntime, "getSyntheticDatasetStore").mockReturnValue(store);
    const input = createOnlyDraft();
    const created = await createRoute(datasetRequest("POST", input));
    expect(created.status).toBe(201);
    expect(await created.json()).toMatchObject({ datasetId: input.datasetId, status: "DRAFT" });
    const before = await readFile(filePath, "utf8");
    for (const attempt of [input, { ...input, sourceId: "operator-edited" }]) {
      const conflict = await createRoute(datasetRequest("POST", attempt));
      expect(conflict.status).toBe(409);
      expect(await conflict.json()).toMatchObject({ error: "DATASET_CONFLICT" });
      expect(await readFile(filePath, "utf8")).toBe(before);
    }
    const edited = await putRoute(datasetRequest("PUT", { ...input, sourceId: "operator-edited" }));
    expect(edited.status).toBe(200);
    expect((await edited.json()).source.sourceId).toBe("operator-edited");
    await store.finalize(input.datasetId);
    const finalizedBytes = await readFile(filePath, "utf8");
    expect((await createRoute(datasetRequest("POST", input))).status).toBe(409);
    expect(await readFile(filePath, "utf8")).toBe(finalizedBytes);
  });

  it("returns a typed 400 for invalid create-only POST bodies without saving rows", async () => {
    const { store } = await temporaryStore();
    vi.spyOn(datasetRuntime, "getSyntheticDatasetStore").mockReturnValue(store);
    for (const body of [
      "{",
      JSON.stringify({ ...createOnlyDraft(), rows: [{ ...firstRow(), amountMinor: "01" }] }),
    ]) {
      const response = await createRoute(
        new Request("http://demo-gateway:3001/api/datasets", { method: "POST", body }),
      );
      expect(response.status).toBe(400);
      expect(await response.json()).toMatchObject({ error: "INVALID_SYNTHETIC_DATASET" });
      expect(await store.list()).toEqual([]);
    }
  });

  it("checks the pinned revision and recomputes its checksum in the same read as the query", async () => {
    const { store, filePath } = await temporaryStore();
    await store.putDraft(
      {
        datasetId: "bound-ledger",
        sourceId: "portal-local",
        sourceLabel: "Binding test",
        rows: [firstRow()],
      },
      NOW,
    );
    const dataset = await store.finalize("bound-ledger", NOW);
    const bound = {
      dataset_id: dataset.datasetId,
      dataset_revision: String(dataset.revision),
      dataset_checksum: dataset.finalization?.checksum ?? "",
    };
    const query = {
      fromOccurredAt: "2026-08-01T00:00:00Z",
      toOccurredAt: "2026-08-03T00:00:00Z",
      limit: 10,
    };
    expect(await store.query(dataset.datasetId, query, bound)).toHaveLength(1);
    for (const altered of [
      { ...bound, dataset_id: "other" },
      { ...bound, dataset_revision: "2" },
      { ...bound, dataset_checksum: "00".repeat(32) },
    ]) {
      await expect(store.query(dataset.datasetId, query, altered)).rejects.toBeInstanceOf(
        ReconciliationValidationError,
      );
    }
    const altered = { ...dataset, rows: [{ ...dataset.rows[0], amountMinor: "99999" }] };
    await writeFile(filePath, JSON.stringify({ formatVersion: 1, datasets: [altered] }));
    await expect(store.query(dataset.datasetId, query, bound)).rejects.toBeInstanceOf(
      ReconciliationValidationError,
    );
  });
  it("persists, finalizes, freezes, and clones one bounded dataset", async () => {
    const { directory, filePath, store } = await temporaryStore();
    const draft = await store.putDraft(
      {
        datasetId: "reconciliation-demo",
        sourceId: "portal-local",
        sourceLabel: "Portal-created demonstration rows",
        rows: [secondRow(), firstRow()],
      },
      NOW,
    );

    expect(draft.source.classification).toBe(DATASET_CLASSIFICATION);
    expect(draft.rows.map((row) => row.transactionId)).toEqual(["txn-001", "txn-002"]);
    expect(draft.rows.every((row) => /^[0-9a-f]{64}$/.test(row.sourceRecordDigest))).toBe(true);

    const finalized = await store.finalize("reconciliation-demo", "2026-08-27T03:01:00Z");
    expect(finalized.status).toBe("FINALIZED");
    expect(finalized.finalization).toEqual({
      finalizedAt: "2026-08-27T03:01:00Z",
      checksum: expect.stringMatching(/^[0-9a-f]{64}$/),
      schemaFingerprint: schemaFingerprint(),
      rowCount: 2,
    });
    await expect(
      store.putDraft(
        {
          datasetId: "reconciliation-demo",
          sourceId: "portal-local",
          sourceLabel: "Changed",
          rows: [firstRow()],
        },
        "2026-08-27T03:02:00Z",
      ),
    ).rejects.toThrow("immutable");

    const cloned = await store.clone(
      "reconciliation-demo",
      "reconciliation-demo-revision-2",
      "2026-08-27T03:03:00Z",
    );
    expect(cloned).toMatchObject({
      status: "DRAFT",
      revision: 2,
      derivedFromDatasetId: "reconciliation-demo",
    });
    expect(cloned.finalization).toBeUndefined();

    expect((await stat(directory)).mode & 0o777).toBe(0o700);
    expect((await stat(filePath)).mode & 0o777).toBe(0o600);
  });

  it("queries finalized rows with deterministic bounds and exact filters", async () => {
    const { store } = await temporaryStore();
    await store.putDraft(
      {
        datasetId: "query-demo",
        sourceId: "portal-local",
        sourceLabel: "Query demonstration",
        rows: [secondRow(), firstRow()],
      },
      NOW,
    );
    await store.finalize("query-demo", "2026-08-27T03:01:00Z");

    const rows = await store.query("query-demo", {
      fromOccurredAt: "2026-08-01T00:00:00Z",
      toOccurredAt: "2026-08-03T00:00:00Z",
      merchantReference: "merchant-a",
      limit: 10,
    });
    expect(rows.map((row) => row.transactionId)).toEqual(["txn-001"]);

    await expect(
      store.query("query-demo", {
        fromOccurredAt: "2026-01-01T00:00:00Z",
        toOccurredAt: new Date(
          Date.parse("2026-01-01T00:00:00Z") + (MAX_QUERY_RANGE_SECONDS + 1) * 1_000,
        )
          .toISOString()
          .replace(".000Z", "Z"),
      }),
    ).rejects.toThrow("range");
  });

  it("rejects duplicate rows, lossy amounts, and sub-second timestamps", async () => {
    const { store } = await temporaryStore();
    const base = {
      datasetId: "invalid-demo",
      sourceId: "portal-local",
      sourceLabel: "Invalid rows",
    };
    await expect(store.putDraft({ ...base, rows: [firstRow(), firstRow()] }, NOW)).rejects.toThrow(
      "unique",
    );
    await expect(
      store.putDraft({ ...base, rows: [{ ...firstRow(), amountMinor: "01" }] }, NOW),
    ).rejects.toThrow("canonical decimal");
    await expect(
      store.putDraft(
        { ...base, rows: [{ ...firstRow(), occurredAt: "2026-08-01T10:00:00.001Z" }] },
        NOW,
      ),
    ).rejects.toBeInstanceOf(ReconciliationValidationError);
  });
});

async function temporaryStore(): Promise<{
  readonly directory: string;
  readonly filePath: string;
  readonly store: SyntheticDatasetStore;
}> {
  const directory = await mkdtemp(path.join(tmpdir(), "raziel-m5-dataset-"));
  temporaryDirectories.push(directory);
  const filePath = path.join(directory, "owner", "transactions.json");
  return {
    directory: path.dirname(filePath),
    filePath,
    store: new SyntheticDatasetStore(filePath),
  };
}

function firstRow(): SyntheticTransactionInput {
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

function createOnlyDraft(): DatasetDraftInput {
  return {
    datasetId: "create-only-demo",
    sourceId: "portal-local",
    sourceLabel: "Synthetic create-only test",
    rows: [firstRow()],
  };
}

function datasetRequest(method: "POST" | "PUT", input: DatasetDraftInput): Request {
  return new Request("http://demo-gateway:3001/api/datasets", {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

function secondRow(): SyntheticTransactionInput {
  return {
    transactionId: "txn-002",
    occurredAt: "2026-08-02T11:30:00Z",
    amountMinor: "-2500",
    currency: "SGD",
    merchantReference: "merchant-b",
    accountReference: "account-1",
    transactionType: "refund",
    reconciliationStatus: "REVIEW",
  };
}
