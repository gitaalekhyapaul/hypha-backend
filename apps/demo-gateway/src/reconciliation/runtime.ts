import { defaultDatasetStorePath, SyntheticDatasetStore } from "./store";

const STORE_SYMBOL = Symbol.for("raziel.m5.synthetic-dataset-store");

interface RazielGlobal {
  [STORE_SYMBOL]?: SyntheticDatasetStore;
}

export function getSyntheticDatasetStore(): SyntheticDatasetStore {
  const shared = globalThis as typeof globalThis & RazielGlobal;
  shared[STORE_SYMBOL] ??= new SyntheticDatasetStore(defaultDatasetStorePath());
  return shared[STORE_SYMBOL];
}
