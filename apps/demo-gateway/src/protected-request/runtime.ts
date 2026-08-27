import { getSyntheticDatasetStore } from "../reconciliation/runtime";
import { ProtectedQueryInfrastructureError } from "./errors";
import { RedisNonceStore } from "./nonce-store";
import { JsonlReceiptStore } from "./receipt-store";
import { ProtectedQueryService } from "./service";
import { ProcessProtectedRequestVerifier } from "./verifier";

const SERVICE_SYMBOL = Symbol.for("raziel.m5.protected-query-service");

interface RazielGlobal {
  [SERVICE_SYMBOL]?: ProtectedQueryService;
}

export function getProtectedQueryService(): ProtectedQueryService {
  const shared = globalThis as typeof globalThis & RazielGlobal;
  shared[SERVICE_SYMBOL] ??= createService();
  return shared[SERVICE_SYMBOL];
}

function createService(): ProtectedQueryService {
  const executablePath = requiredEnvironment("RAZIEL_VERIFIER_CLIENT_EXECUTABLE");
  const endpoint = requiredEnvironment("RAZIEL_VERIFIERD_ENDPOINT");
  const redisUrl = requiredEnvironment("RAZIEL_REDIS_URL");
  const receiptPath = requiredEnvironment("RAZIEL_RECEIPT_STORE_PATH");
  return new ProtectedQueryService({
    verifier: new ProcessProtectedRequestVerifier({ executablePath, endpoint }),
    nonceStore: new RedisNonceStore(redisUrl),
    receiptStore: new JsonlReceiptStore(receiptPath),
    queryDataset: (datasetId, query) => getSyntheticDatasetStore().query(datasetId, query),
  });
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new ProtectedQueryInfrastructureError();
  }
  return value;
}
