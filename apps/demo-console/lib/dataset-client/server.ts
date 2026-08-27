import {
  type DatasetClientDependencies,
  DatasetClientError,
  type DatasetGatewayConfig,
  type SyntheticDataset,
  type SyntheticDatasetCollection,
  type SyntheticDatasetDraft,
} from "./types.ts";
import {
  decodeDataset,
  decodeDatasetCollection,
  decodeGatewayError,
  validateDatasetReference,
} from "./validation.ts";

const GATEWAY_TIMEOUT_MS = 5_000;
const MAX_GATEWAY_RESPONSE_BYTES = 2 * 1024 * 1024;

export class SyntheticDatasetClient {
  readonly #config: DatasetGatewayConfig;
  readonly #fetch: typeof fetch;

  public constructor(config: DatasetGatewayConfig, dependencies: DatasetClientDependencies = {}) {
    this.#config = config;
    this.#fetch = dependencies.fetch ?? fetch;
  }

  public async list(): Promise<SyntheticDatasetCollection> {
    return decodeDatasetCollection(await this.#request("GET", "/api/datasets"));
  }

  public async get(datasetIdInput: unknown): Promise<SyntheticDataset> {
    const datasetId = validateDatasetReference(datasetIdInput);
    return decodeDataset(await this.#request("GET", datasetPath(datasetId)));
  }

  public async putDraft(draft: SyntheticDatasetDraft): Promise<SyntheticDataset> {
    return decodeDataset(await this.#request("PUT", "/api/datasets", draft));
  }

  public async finalize(datasetIdInput: unknown): Promise<SyntheticDataset> {
    const datasetId = validateDatasetReference(datasetIdInput);
    return decodeDataset(await this.#request("POST", `${datasetPath(datasetId)}/finalize`));
  }

  public async clone(datasetIdInput: unknown, targetDatasetId: string): Promise<SyntheticDataset> {
    const datasetId = validateDatasetReference(datasetIdInput);
    return decodeDataset(
      await this.#request("POST", `${datasetPath(datasetId)}/clone`, { targetDatasetId }),
    );
  }

  async #request(method: string, path: string, body?: unknown): Promise<unknown> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), GATEWAY_TIMEOUT_MS);
    let response: Response;
    try {
      response = await this.#fetch(new URL(path, this.#config.baseUrl), {
        method,
        cache: "no-store",
        signal: controller.signal,
        ...(body === undefined
          ? {}
          : {
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify(body),
            }),
      });
    } catch {
      throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
    } finally {
      clearTimeout(timeout);
    }

    const declaredLength = response.headers.get("content-length");
    if (
      declaredLength !== null &&
      (!Number.isSafeInteger(Number(declaredLength)) ||
        Number(declaredLength) < 0 ||
        Number(declaredLength) > MAX_GATEWAY_RESPONSE_BYTES)
    ) {
      throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
    }
    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.byteLength === 0 || bytes.byteLength > MAX_GATEWAY_RESPONSE_BYTES) {
      throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
    }
    let payload: unknown;
    try {
      payload = JSON.parse(bytes.toString("utf8")) as unknown;
    } catch {
      throw new DatasetClientError("DATASET_UNAVAILABLE", 503);
    }
    if (!response.ok) {
      throw decodeGatewayError(payload);
    }
    return payload;
  }
}

function datasetPath(datasetId: string): string {
  return `/api/datasets/${encodeURIComponent(datasetId)}`;
}
