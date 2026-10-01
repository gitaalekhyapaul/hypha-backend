import type { ReconciliationSeedResult } from "../dataset-client/reconciliation-seed.ts";
import { OPERATOR_CSRF_HEADER } from "../operator-auth/types.ts";
import { PortalRequestError } from "./browser.ts";

export async function seedReconciliationLedgers(
  csrfToken: string,
): Promise<ReconciliationSeedResult> {
  let response: Response;
  try {
    response = await fetch("/api/datasets/seed-reconciliation", {
      method: "POST",
      headers: { [OPERATOR_CSRF_HEADER]: csrfToken },
      credentials: "same-origin",
      cache: "no-store",
      signal: AbortSignal.timeout(30_000),
    });
  } catch {
    throw new PortalRequestError("PORTAL_UNAVAILABLE", 503);
  }
  if (response.status === 401) {
    throw new PortalRequestError("OPERATOR_UNAUTHORIZED", 401);
  }
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new PortalRequestError("PORTAL_UNAVAILABLE", response.ok ? 503 : response.status);
  }
  if (!response.ok) {
    const failure =
      typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
    throw new PortalRequestError(
      typeof failure.error === "string" ? failure.error : "PORTAL_UNAVAILABLE",
      response.status,
      typeof failure.message === "string" ? failure.message : undefined,
    );
  }
  return body as ReconciliationSeedResult;
}
