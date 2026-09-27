import {
  type ApprovedPolicyCatalogue,
  parseApprovedPolicyCatalogue,
} from "../policy-version/catalogue-types.ts";
import { PortalRequestError } from "./browser.ts";

export async function getApprovedPolicyCatalogue(
  signal?: AbortSignal,
): Promise<ApprovedPolicyCatalogue> {
  let response: Response;
  try {
    const timeout = AbortSignal.timeout(15_000);
    response = await fetch("/api/policies/versions/approved", {
      method: "GET",
      cache: "no-store",
      credentials: "same-origin",
      signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
    });
  } catch {
    throw unavailable();
  }
  if (response.status === 401) throw new PortalRequestError("OPERATOR_UNAUTHORIZED", 401);
  if (!response.ok) throw unavailable();
  try {
    return parseApprovedPolicyCatalogue(await response.json());
  } catch {
    throw unavailable();
  }
}

function unavailable(): PortalRequestError {
  return new PortalRequestError("PORTAL_UNAVAILABLE", 503);
}
