export const SYNTHETIC_DATASET_CLASSIFICATION = "SYNTHETIC" as const;
export const SYNTHETIC_DATASET_FORMAT_VERSION = 1 as const;

export type ReconciliationStatus = "UNMATCHED" | "MATCHED" | "REVIEW";
export type DatasetStatus = "DRAFT" | "FINALIZED";

export interface SyntheticTransactionDraft {
  readonly transactionId: string;
  readonly occurredAt: string;
  readonly amountMinor: string;
  readonly currency: string;
  readonly merchantReference?: string;
  readonly accountReference?: string;
  readonly transactionType?: string;
  readonly reconciliationStatus: ReconciliationStatus;
}

export interface SyntheticTransaction extends SyntheticTransactionDraft {
  readonly sourceRecordDigest: string;
}

export interface SyntheticDatasetDraft {
  readonly datasetId: string;
  readonly sourceId: string;
  readonly sourceLabel: string;
  readonly rows: readonly SyntheticTransactionDraft[];
}

export interface DatasetFinalization {
  readonly finalizedAt: string;
  readonly checksum: string;
  readonly schemaFingerprint: string;
  readonly rowCount: number;
}

export interface SyntheticDataset {
  readonly formatVersion: typeof SYNTHETIC_DATASET_FORMAT_VERSION;
  readonly datasetId: string;
  readonly revision: number;
  readonly derivedFromDatasetId?: string;
  readonly source: {
    readonly classification: typeof SYNTHETIC_DATASET_CLASSIFICATION;
    readonly sourceId: string;
    readonly sourceLabel: string;
  };
  readonly status: DatasetStatus;
  readonly rows: readonly SyntheticTransaction[];
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly finalization?: DatasetFinalization;
}

export interface SyntheticDatasetCollection {
  readonly classification: typeof SYNTHETIC_DATASET_CLASSIFICATION;
  readonly datasets: readonly SyntheticDataset[];
}

export type DatasetFailureCode =
  | "DATASET_CONFLICT"
  | "DATASET_NOT_FOUND"
  | "DATASET_UNAVAILABLE"
  | "GATEWAY_CONFIGURATION_INVALID"
  | "INVALID_DATASET_REQUEST"
  | "INVALID_SYNTHETIC_DATASET";

export interface DatasetGatewayConfig {
  readonly baseUrl: string;
}

export interface DatasetClientDependencies {
  readonly fetch?: typeof fetch;
}

export class DatasetClientError extends Error {
  public readonly code: DatasetFailureCode;
  public readonly status: number;
  public readonly publicMessage: string | undefined;

  public constructor(code: DatasetFailureCode, status: number, publicMessage?: string) {
    super(code);
    this.name = "DatasetClientError";
    this.code = code;
    this.status = status;
    this.publicMessage = publicMessage;
  }
}
