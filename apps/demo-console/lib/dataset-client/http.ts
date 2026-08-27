import { jsonResponse } from "../operator-auth/index.ts";
import { DatasetClientError, type DatasetFailureCode } from "./types.ts";

export function datasetResponse(value: unknown, status = 200): Response {
  return jsonResponse(value, status);
}

export function datasetErrorResponse(error: unknown): Response {
  const failure =
    error instanceof DatasetClientError
      ? error
      : new DatasetClientError("DATASET_UNAVAILABLE", 503);
  const body: { error: DatasetFailureCode; message?: string } = { error: failure.code };
  if (failure.publicMessage !== undefined) {
    body.message = failure.publicMessage;
  }
  return jsonResponse(body, failure.status);
}
