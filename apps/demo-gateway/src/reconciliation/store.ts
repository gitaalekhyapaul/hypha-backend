import { constants } from "node:fs";
import { access, chmod, mkdir, open, readFile, rename } from "node:fs/promises";
import path from "node:path";

import {
  cloneDataset,
  createDraft,
  DATASET_FORMAT_VERSION,
  type DatasetDraftInput,
  datasetChecksum,
  finalizeDataset,
  queryDataset,
  type ReconciliationQuery,
  ReconciliationValidationError,
  type SyntheticDataset,
  type SyntheticTransaction,
} from "./model";

const STORE_FILE_MODE = 0o600;
const STORE_DIRECTORY_MODE = 0o700;

interface PersistedStore {
  readonly formatVersion: typeof DATASET_FORMAT_VERSION;
  readonly datasets: readonly SyntheticDataset[];
}

export class DatasetNotFoundError extends Error {
  public constructor() {
    super("dataset was not found");
    this.name = "DatasetNotFoundError";
  }
}

export class DatasetConflictError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "DatasetConflictError";
  }
}

export class SyntheticDatasetStore {
  readonly #filePath: string;
  #tail: Promise<void> = Promise.resolve();

  public constructor(filePath: string) {
    this.#filePath = path.resolve(filePath);
  }

  public async list(): Promise<readonly SyntheticDataset[]> {
    return this.#exclusive(async () => (await this.#read()).datasets);
  }

  public async get(datasetId: string): Promise<SyntheticDataset> {
    return this.#exclusive(async () => this.#getFrom(await this.#read(), datasetId));
  }

  public async putDraft(
    input: DatasetDraftInput,
    now = wholeSecondNow(),
  ): Promise<SyntheticDataset> {
    return this.#exclusive(async () => {
      const persisted = await this.#read();
      const existing = persisted.datasets.find((dataset) => dataset.datasetId === input.datasetId);
      const draft = createDraft(input, now, existing);
      await this.#write({
        formatVersion: DATASET_FORMAT_VERSION,
        datasets: replaceDataset(persisted.datasets, draft),
      });
      return draft;
    });
  }

  public async finalize(datasetId: string, now = wholeSecondNow()): Promise<SyntheticDataset> {
    return this.#exclusive(async () => {
      const persisted = await this.#read();
      const finalized = finalizeDataset(this.#getFrom(persisted, datasetId), now);
      await this.#write({
        formatVersion: DATASET_FORMAT_VERSION,
        datasets: replaceDataset(persisted.datasets, finalized),
      });
      return finalized;
    });
  }

  public async clone(
    sourceDatasetId: string,
    targetDatasetId: string,
    now = wholeSecondNow(),
  ): Promise<SyntheticDataset> {
    return this.#exclusive(async () => {
      const persisted = await this.#read();
      if (persisted.datasets.some((dataset) => dataset.datasetId === targetDatasetId)) {
        throw new DatasetConflictError("clone target already exists");
      }
      const cloned = cloneDataset(this.#getFrom(persisted, sourceDatasetId), targetDatasetId, now);
      await this.#write({
        formatVersion: DATASET_FORMAT_VERSION,
        datasets: [...persisted.datasets, cloned].sort((left, right) =>
          left.datasetId.localeCompare(right.datasetId),
        ),
      });
      return cloned;
    });
  }

  public async query(
    datasetId: string,
    query: ReconciliationQuery,
    expected?: {
      readonly dataset_id: string;
      readonly dataset_revision: string;
      readonly dataset_checksum: string;
    },
  ): Promise<readonly SyntheticTransaction[]> {
    return this.#exclusive(async () => {
      const dataset = this.#getFrom(await this.#read(), datasetId);
      if (
        expected &&
        (dataset.datasetId !== expected.dataset_id ||
          String(dataset.revision) !== expected.dataset_revision ||
          dataset.status !== "FINALIZED" ||
          dataset.finalization?.checksum !== expected.dataset_checksum ||
          dataset.finalization.rowCount !== dataset.rows.length ||
          datasetChecksum(dataset) !== expected.dataset_checksum)
      ) {
        throw new ReconciliationValidationError(
          "dataset binding does not match finalized revision",
        );
      }
      return queryDataset(dataset, query);
    });
  }

  async #read(): Promise<PersistedStore> {
    try {
      const raw = await readFile(this.#filePath, "utf8");
      const parsed = JSON.parse(raw) as Partial<PersistedStore>;
      if (parsed.formatVersion !== DATASET_FORMAT_VERSION || !Array.isArray(parsed.datasets)) {
        throw new ReconciliationValidationError("dataset store format is invalid");
      }
      return {
        formatVersion: DATASET_FORMAT_VERSION,
        datasets: parsed.datasets,
      };
    } catch (error) {
      if (isNodeError(error, "ENOENT")) {
        return { formatVersion: DATASET_FORMAT_VERSION, datasets: [] };
      }
      throw error;
    }
  }

  async #write(store: PersistedStore): Promise<void> {
    const directory = path.dirname(this.#filePath);
    await mkdir(directory, { recursive: true, mode: STORE_DIRECTORY_MODE });
    await chmod(directory, STORE_DIRECTORY_MODE);
    const temporaryPath = `${this.#filePath}.${process.pid}.${Date.now()}.tmp`;
    const handle = await open(
      temporaryPath,
      constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY,
      STORE_FILE_MODE,
    );
    try {
      await handle.writeFile(`${JSON.stringify(store, null, 2)}\n`, "utf8");
      await handle.sync();
    } finally {
      await handle.close();
    }
    await rename(temporaryPath, this.#filePath);
    await chmod(this.#filePath, STORE_FILE_MODE);
  }

  #getFrom(store: PersistedStore, datasetId: string): SyntheticDataset {
    const dataset = store.datasets.find((candidate) => candidate.datasetId === datasetId);
    if (!dataset) {
      throw new DatasetNotFoundError();
    }
    return dataset;
  }

  #exclusive<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.#tail.then(operation, operation);
    this.#tail = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }
}

export function defaultDatasetStorePath(): string {
  return (
    process.env.RAZIEL_TRANSACTION_STORE_PATH ??
    path.join(process.cwd(), ".raziel", "m5", "transactions.json")
  );
}

export function wholeSecondNow(): string {
  return new Date(Math.floor(Date.now() / 1_000) * 1_000).toISOString().replace(".000Z", "Z");
}

export async function datasetStoreExists(filePath: string): Promise<boolean> {
  try {
    await access(filePath, constants.F_OK);
    return true;
  } catch (error) {
    if (isNodeError(error, "ENOENT")) {
      return false;
    }
    throw error;
  }
}

function replaceDataset(
  datasets: readonly SyntheticDataset[],
  replacement: SyntheticDataset,
): SyntheticDataset[] {
  return [
    ...datasets.filter((dataset) => dataset.datasetId !== replacement.datasetId),
    replacement,
  ].sort((left, right) => left.datasetId.localeCompare(right.datasetId));
}

function isNodeError(error: unknown, code: string): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error && error.code === code;
}
