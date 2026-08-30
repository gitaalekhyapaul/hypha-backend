import type {
  AccessGraphPublicationProjection,
  PublishAccessGraphRequest,
} from "../access-graph/types.ts";
import type {
  AgentSessionProjection,
  CapabilityJobProjection,
  CreateCapabilityInput,
} from "../agent-capability/types.ts";
import type {
  SyntheticDataset,
  SyntheticDatasetCollection,
  SyntheticDatasetDraft,
} from "../dataset-client/types.ts";
import { OPERATOR_CSRF_HEADER } from "../operator-auth/types.ts";
import type { PolicyActivationProjection } from "../policy-activation/types.ts";
import type {
  PolicyCandidateProjection,
  PreparePolicyCandidateRequest,
} from "../policy-candidate/types.ts";
import type {
  ProtectedReconciliationQueryInput,
  ProtectedReconciliationQueryResult,
} from "./reconciliation.ts";
import {
  type PublicReceiptLedger,
  type ReceiptFilterSelection,
  receiptQueryPath,
} from "./receipts.ts";

export interface BrowserOperatorSession {
  readonly operatorId: string;
  readonly csrfToken: string;
  readonly expiresAtUnixSeconds: number;
}

export class PortalRequestError extends Error {
  public readonly code: string;
  public readonly status: number;

  public constructor(code: string, status: number, message?: string) {
    super(message ?? code);
    this.name = "PortalRequestError";
    this.code = code;
    this.status = status;
  }
}

export function operatorSession(): Promise<BrowserOperatorSession> {
  return request("/api/operator/session");
}

export function loginOperator(password: string): Promise<BrowserOperatorSession> {
  return request("/api/operator/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ password }),
  });
}

export function logoutOperator(csrfToken: string): Promise<{ readonly status: "SIGNED_OUT" }> {
  return mutationRequest("/api/operator/logout", csrfToken);
}

export function listDatasets(): Promise<SyntheticDatasetCollection> {
  return request("/api/datasets");
}

export function saveDataset(
  draft: SyntheticDatasetDraft,
  csrfToken: string,
): Promise<SyntheticDataset> {
  return mutationRequest("/api/datasets", csrfToken, "PUT", draft);
}

export function finalizeDataset(datasetId: string, csrfToken: string): Promise<SyntheticDataset> {
  return mutationRequest(`/api/datasets/${encodeURIComponent(datasetId)}/finalize`, csrfToken);
}

export function cloneDataset(
  datasetId: string,
  targetDatasetId: string,
  csrfToken: string,
): Promise<SyntheticDataset> {
  return mutationRequest(
    `/api/datasets/${encodeURIComponent(datasetId)}/clone`,
    csrfToken,
    "POST",
    { targetDatasetId },
  );
}

export function compilePolicyCandidate(
  candidate: PreparePolicyCandidateRequest,
  csrfToken: string,
): Promise<PolicyCandidateProjection> {
  return mutationRequest("/api/policies/candidates", csrfToken, "POST", candidate);
}

export function loadPolicyCandidate(candidateId: string): Promise<PolicyCandidateProjection> {
  return request(`/api/policies/candidates/${encodeURIComponent(candidateId)}`);
}

export function activatePolicyCandidate(
  candidateId: string,
  csrfToken: string,
): Promise<PolicyActivationProjection> {
  return mutationRequest(
    `/api/policies/candidates/${encodeURIComponent(candidateId)}/activate`,
    csrfToken,
  );
}

export function publishAccessGraph(
  graph: PublishAccessGraphRequest,
  csrfToken: string,
): Promise<AccessGraphPublicationProjection> {
  return mutationRequest("/api/access-graphs", csrfToken, "POST", graph);
}

export function createAgentSession(csrfToken: string): Promise<AgentSessionProjection> {
  return mutationRequest("/api/agent-sessions", csrfToken);
}

export function requestCapability(
  input: CreateCapabilityInput,
  csrfToken: string,
): Promise<CapabilityJobProjection> {
  return mutationRequest("/api/capabilities", csrfToken, "POST", input);
}

export function loadCapabilityJob(jobId: string): Promise<CapabilityJobProjection> {
  return request(`/api/capabilities/${encodeURIComponent(jobId)}`);
}

export function cancelCapabilityJob(
  jobId: string,
  csrfToken: string,
): Promise<CapabilityJobProjection> {
  return mutationRequest(`/api/capabilities/${encodeURIComponent(jobId)}/cancel`, csrfToken);
}

export function queryReconciliation(
  input: ProtectedReconciliationQueryInput,
  csrfToken: string,
): Promise<ProtectedReconciliationQueryResult> {
  return mutationRequest("/api/reconciliation/query", csrfToken, "POST", input);
}

export function loadReceipts(filter?: ReceiptFilterSelection): Promise<PublicReceiptLedger> {
  return request(receiptQueryPath(filter));
}

async function mutationRequest<T>(
  path: string,
  csrfToken: string,
  method = "POST",
  body?: unknown,
): Promise<T> {
  return request(path, {
    method,
    headers: {
      [OPERATOR_CSRF_HEADER]: csrfToken,
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, {
      ...init,
      cache: "no-store",
      credentials: "same-origin",
    });
  } catch {
    throw new PortalRequestError("PORTAL_UNAVAILABLE", 503);
  }
  let body: unknown;
  try {
    body = (await response.json()) as unknown;
  } catch {
    throw new PortalRequestError("PORTAL_UNAVAILABLE", 503);
  }
  if (!response.ok) {
    const failure = publicFailure(body);
    throw new PortalRequestError(failure.code, response.status, failure.message);
  }
  return body as T;
}

function publicFailure(input: unknown): { readonly code: string; readonly message?: string } {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    return { code: "PORTAL_UNAVAILABLE" };
  }
  const object = input as Record<string, unknown>;
  if (typeof object.error !== "string") {
    return { code: "PORTAL_UNAVAILABLE" };
  }
  return {
    code: object.error,
    ...(typeof object.message === "string" ? { message: object.message } : {}),
  };
}
