import type { SyntheticDatasetClient } from "./server.ts";
import {
  DatasetClientError,
  SYNTHETIC_DATASET_CLASSIFICATION,
  type SyntheticDataset,
  type SyntheticDatasetCollection,
  type SyntheticDatasetDraft,
  type SyntheticTransactionDraft,
} from "./types.ts";

export interface ReconciliationSeedResult extends SyntheticDatasetCollection {
  readonly createdDatasetIds: readonly string[];
  readonly preservedDatasetIds: readonly string[];
}

const GENERATOR_ID = "reconciliation-ledgers-v1";
let seedTail: Promise<void> = Promise.resolve();

export function reconciliationSeedDrafts(): readonly SyntheticDatasetDraft[] {
  return [
    {
      datasetId: "processor-demo-v1",
      sourceId: GENERATOR_ID,
      sourceLabel: "Synthetic processor ledger, deterministic reconciliation generator v1",
      rows: [
        seedRow("processor-001", "2026-10-01T10:00:00Z", "12500", "recon-exact-001", "payment"),
        seedRow("processor-002", "2026-10-01T10:05:00Z", "4899", "recon-missing-002", "payment"),
        seedRow("processor-003", "2026-10-01T10:10:00Z", "9900", "recon-ambiguous-003", "payment"),
      ],
    },
    {
      datasetId: "bank-demo-v1",
      sourceId: GENERATOR_ID,
      sourceLabel: "Synthetic bank ledger, deterministic reconciliation generator v1",
      rows: [
        seedRow("bank-001", "2026-10-01T12:00:00Z", "12500", "recon-exact-001", "settlement"),
        seedRow("bank-002", "2026-10-01T12:05:00Z", "9900", "recon-ambiguous-003", "settlement"),
        seedRow("bank-003", "2026-10-01T12:10:00Z", "9900", "recon-ambiguous-003", "settlement"),
      ],
    },
  ];
}

export function seedReconciliationLedgers(
  client: Pick<SyntheticDatasetClient, "get" | "createDraft">,
): Promise<ReconciliationSeedResult> {
  // Local serialization reduces duplicate work. Store-level create-only protects other writers.
  const result = seedTail.then(() => seedMissingLedgers(client));
  seedTail = result.then(
    () => undefined,
    () => undefined,
  );
  return result;
}

async function seedMissingLedgers(
  client: Pick<SyntheticDatasetClient, "get" | "createDraft">,
): Promise<ReconciliationSeedResult> {
  const drafts = reconciliationSeedDrafts();
  const existing = new Map<string, SyntheticDataset>();
  // Validate the whole pair before creating a missing partner.
  for (const draft of drafts) {
    const dataset = await getExisting(client, draft.datasetId);
    if (dataset) {
      validateExisting(dataset, draft);
      existing.set(draft.datasetId, dataset);
    }
  }

  const datasets: SyntheticDataset[] = [];
  const createdDatasetIds: string[] = [];
  const preservedDatasetIds: string[] = [];
  for (const draft of drafts) {
    const dataset = existing.get(draft.datasetId) ?? (await getExisting(client, draft.datasetId));
    if (dataset) {
      validateExisting(dataset, draft);
      datasets.push(dataset);
      preservedDatasetIds.push(draft.datasetId);
    } else {
      try {
        datasets.push(await client.createDraft(draft));
        createdDatasetIds.push(draft.datasetId);
      } catch (error) {
        if (!(error instanceof DatasetClientError) || error.code !== "DATASET_CONFLICT") {
          throw error;
        }
        const current = await client.get(draft.datasetId);
        validateExisting(current, draft);
        datasets.push(current);
        preservedDatasetIds.push(draft.datasetId);
      }
    }
  }
  return {
    classification: SYNTHETIC_DATASET_CLASSIFICATION,
    datasets,
    createdDatasetIds,
    preservedDatasetIds,
  };
}

async function getExisting(
  client: Pick<SyntheticDatasetClient, "get">,
  datasetId: string,
): Promise<SyntheticDataset | undefined> {
  try {
    return await client.get(datasetId);
  } catch (error) {
    if (error instanceof DatasetClientError && error.code === "DATASET_NOT_FOUND") {
      return undefined;
    }
    throw error;
  }
}

function validateExisting(dataset: SyntheticDataset, draft: SyntheticDatasetDraft): void {
  const expectedRows = new Map(draft.rows.map((row) => [row.transactionId, row]));
  const matches =
    dataset.datasetId === draft.datasetId &&
    dataset.source.classification === SYNTHETIC_DATASET_CLASSIFICATION &&
    dataset.source.sourceId === draft.sourceId &&
    dataset.source.sourceLabel === draft.sourceLabel &&
    dataset.rows.length === draft.rows.length &&
    new Set(dataset.rows.map((row) => row.transactionId)).size === draft.rows.length &&
    dataset.rows.every((row) => {
      const expected = expectedRows.get(row.transactionId);
      return (
        expected !== undefined &&
        row.occurredAt === expected.occurredAt &&
        row.amountMinor === expected.amountMinor &&
        row.currency === expected.currency &&
        row.merchantReference === expected.merchantReference &&
        row.accountReference === expected.accountReference &&
        row.transactionType === expected.transactionType &&
        row.reconciliationStatus === expected.reconciliationStatus
      );
    });
  if (!matches) {
    throw new DatasetClientError(
      "DATASET_CONFLICT",
      409,
      `${draft.datasetId} already exists with different rows or source metadata. Existing data was preserved. Use the current dataset or create a separate draft.`,
    );
  }
}

function seedRow(
  transactionId: string,
  occurredAt: string,
  amountMinor: string,
  merchantReference: string,
  transactionType: string,
): SyntheticTransactionDraft {
  return {
    transactionId,
    occurredAt,
    amountMinor,
    currency: "SGD",
    merchantReference,
    accountReference: "synthetic-account-001",
    transactionType,
    reconciliationStatus: "UNMATCHED",
  };
}
