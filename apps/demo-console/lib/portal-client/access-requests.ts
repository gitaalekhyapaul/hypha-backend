import { create, fromJson, type JsonValue, toJson } from "@bufbuild/protobuf";
import {
  type AccessRequest,
  AccessRequestMutationSchema,
  AccessRequestSchema,
  AccessRequestState,
  type ListAccessRequestsResponse,
  ListAccessRequestsResponseSchema,
  ProofJobState,
} from "@raziel/proto-ts";
import { OPERATOR_CSRF_HEADER } from "../operator-auth/types.ts";
import { PortalRequestError } from "./browser.ts";

export type { AccessRequest, ListAccessRequestsResponse } from "@raziel/proto-ts";
export { AccessRequestState } from "@raziel/proto-ts";

const AGENT_ID = /^[0-9a-f]{32}$/;
const REQUEST_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const MAX_TIMESTAMP = 253_402_300_799n;
const MAX_UINT64 = 18_446_744_073_709_551_615n;
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;

export interface AccessRequestGrant {
  readonly formatVersion: 1;
  readonly teamId: string;
  readonly organizationId: string;
  readonly approvedAmount: string;
  readonly isApproved: boolean;
  readonly isSuspended: boolean;
  readonly isOverride: boolean;
}

export interface AccessRequestApproval {
  readonly formatVersion: 1;
  readonly expectedGeneration: string;
  readonly policyRecordId: string;
  readonly policyVersionId: string;
  readonly approvalEpoch: string;
  readonly grant: AccessRequestGrant;
}

const publicErrors = new Set([
  "INVALID_REQUEST",
  "NOT_AUTHORIZED",
  "NOT_FOUND",
  "CONFLICT",
  "REQUEST_REPLAYED",
  "EXPIRED",
  "CAPACITY_EXCEEDED",
  "SERVICE_UNAVAILABLE",
  "OPERATOR_UNAUTHORIZED",
  "OPERATOR_CSRF_REJECTED",
  "OPERATOR_ORIGIN_REJECTED",
]);

export class AccessRequestClientError extends PortalRequestError {
  public constructor(
    code: string,
    status: number,
    public readonly retryable = false,
  ) {
    super(code, status);
    this.name = "AccessRequestClientError";
  }
}

export async function listAccessRequests(
  options: { readonly limit?: number; readonly cursor?: string; readonly agentId?: string } = {},
  signal?: AbortSignal,
): Promise<ListAccessRequestsResponse> {
  const limit = options.limit ?? 25;
  if (!Number.isInteger(limit) || limit < 1 || limit > 50) throw invalidRequest();
  if (options.agentId !== undefined && !AGENT_ID.test(options.agentId)) throw invalidRequest();
  if (options.cursor !== undefined && !validCursor(options.cursor)) throw invalidRequest();
  const query = new URLSearchParams({ limit: String(limit) });
  if (options.cursor !== undefined) query.set("cursor", options.cursor);
  if (options.agentId !== undefined) query.set("agentId", options.agentId);
  const body = await request(`/api/operator/access-requests?${query}`, {}, signal);
  try {
    const result = fromJson(ListAccessRequestsResponseSchema, body as JsonValue);
    assertCanonicalJson(
      body,
      toJson(ListAccessRequestsResponseSchema, result, { alwaysEmitImplicit: true }),
    );
    if (
      result.formatVersion !== 1 ||
      !Array.isArray(object(body).requests) ||
      result.requests.length > limit ||
      (result.nextCursor !== undefined &&
        (!validCursor(result.nextCursor) || result.nextCursor === options.cursor))
    ) {
      throw unavailable();
    }
    const identities = new Set<string>();
    for (const item of result.requests) {
      validateRequest(item);
      const identity = `${item.agentId}/${item.requestId}`;
      if (
        item.capability.length !== 0 ||
        identities.has(identity) ||
        (options.agentId !== undefined && item.agentId !== options.agentId)
      ) {
        throw unavailable();
      }
      identities.add(identity);
    }
    return result;
  } catch {
    throw unavailable();
  }
}

export async function getAccessRequest(
  agentId: string,
  requestId: string,
  signal?: AbortSignal,
): Promise<AccessRequest> {
  return decodeRequest(
    await request(requestPath(agentId, requestId), {}, signal),
    agentId,
    requestId,
  );
}

export function rejectAccessRequest(
  agentId: string,
  requestId: string,
  expectedGeneration: bigint,
  csrfToken: string,
): Promise<AccessRequest> {
  return mutateRequest("reject", agentId, requestId, expectedGeneration, csrfToken);
}

export function cancelAccessRequest(
  agentId: string,
  requestId: string,
  expectedGeneration: bigint,
  csrfToken: string,
): Promise<AccessRequest> {
  return mutateRequest("cancel", agentId, requestId, expectedGeneration, csrfToken);
}

export async function approveAccessRequest(
  agentId: string,
  requestId: string,
  approval: AccessRequestApproval,
  csrfToken: string,
): Promise<AccessRequest> {
  const path = requestPath(agentId, requestId);
  if (!csrfToken) throw invalidRequest();
  const body = approvalBody(approval);
  const result = decodeRequest(
    await request(`${path}/approve`, {
      method: "POST",
      headers: { "Content-Type": "application/json", [OPERATOR_CSRF_HEADER]: csrfToken },
      body: JSON.stringify(body),
    }),
    agentId,
    requestId,
  );
  const binding = result.policyBinding;
  if (
    result.state !== AccessRequestState.APPROVED_WAITING ||
    result.generation <= BigInt(body.expectedGeneration) ||
    !binding ||
    binding.policyRecordId !== body.policyRecordId ||
    binding.policyHash.length !== 32 ||
    binding.resourceType !== `dataset_${body.policyVersionId}` ||
    !SAFE_ID.test(binding.resourceScope)
  ) {
    throw unavailable();
  }
  return result;
}

function approvalBody(input: AccessRequestApproval): AccessRequestApproval {
  strictFields(input, [
    "formatVersion",
    "expectedGeneration",
    "policyRecordId",
    "policyVersionId",
    "approvalEpoch",
    "grant",
  ]);
  strictFields(input.grant, [
    "formatVersion",
    "teamId",
    "organizationId",
    "approvedAmount",
    "isApproved",
    "isSuspended",
    "isOverride",
  ]);
  const grant = input.grant;
  if (
    input.formatVersion !== 1 ||
    !positiveDecimal(input.expectedGeneration, MAX_UINT64) ||
    typeof input.policyRecordId !== "string" ||
    input.policyRecordId.length === 0 ||
    input.policyRecordId.length > 256 ||
    typeof input.policyVersionId !== "string" ||
    !AGENT_ID.test(input.policyVersionId) ||
    !positiveDecimal(input.approvalEpoch, MAX_UINT64) ||
    grant.formatVersion !== 1 ||
    typeof grant.teamId !== "string" ||
    !SAFE_ID.test(grant.teamId) ||
    typeof grant.organizationId !== "string" ||
    !SAFE_ID.test(grant.organizationId) ||
    !positiveDecimal(grant.approvedAmount, BigInt(Number.MAX_SAFE_INTEGER)) ||
    typeof grant.isApproved !== "boolean" ||
    typeof grant.isSuspended !== "boolean" ||
    typeof grant.isOverride !== "boolean"
  ) {
    throw invalidRequest();
  }
  // Copy only the owner approval fields. Dataset binding is resolved by the server.
  return {
    formatVersion: 1,
    expectedGeneration: input.expectedGeneration,
    policyRecordId: input.policyRecordId,
    policyVersionId: input.policyVersionId,
    approvalEpoch: input.approvalEpoch,
    grant: {
      formatVersion: 1,
      teamId: grant.teamId,
      organizationId: grant.organizationId,
      approvedAmount: grant.approvedAmount,
      isApproved: grant.isApproved,
      isSuspended: grant.isSuspended,
      isOverride: grant.isOverride,
    },
  };
}

function strictFields(value: unknown, keys: readonly string[]): void {
  if (!isObject(value)) throw invalidRequest();
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) throw invalidRequest();
  const actual = Reflect.ownKeys(value);
  if (
    actual.length !== keys.length ||
    actual.some((key) => typeof key !== "string" || !keys.includes(key))
  ) {
    throw invalidRequest();
  }
}

function positiveDecimal(value: unknown, maximum: bigint): value is string {
  return typeof value === "string" && /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= maximum;
}

async function mutateRequest(
  action: "reject" | "cancel",
  agentId: string,
  requestId: string,
  expectedGeneration: bigint,
  csrfToken: string,
): Promise<AccessRequest> {
  const path = requestPath(agentId, requestId);
  if (
    typeof expectedGeneration !== "bigint" ||
    expectedGeneration < 1n ||
    expectedGeneration > MAX_UINT64 ||
    !csrfToken
  ) {
    throw invalidRequest();
  }
  const mutation = create(AccessRequestMutationSchema, { formatVersion: 1, expectedGeneration });
  const result = decodeRequest(
    await request(`${path}/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", [OPERATOR_CSRF_HEADER]: csrfToken },
      body: JSON.stringify(toJson(AccessRequestMutationSchema, mutation)),
    }),
    agentId,
    requestId,
  );
  if (result.generation < expectedGeneration) throw unavailable();
  return result;
}

async function request(path: string, init: RequestInit, signal?: AbortSignal): Promise<unknown> {
  let response: Response;
  try {
    const timeout = AbortSignal.timeout(15_000);
    response = await fetch(path, {
      ...init,
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
      signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
    });
  } catch {
    throw unavailable();
  }
  if (response.status === 401) throw new AccessRequestClientError("OPERATOR_UNAUTHORIZED", 401);
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new AccessRequestClientError("PORTAL_UNAVAILABLE", response.ok ? 503 : response.status);
  }
  if (!response.ok) {
    const error = isObject(body) ? body.error : undefined;
    const code = typeof error === "string" ? error : isObject(error) ? error.code : undefined;
    throw new AccessRequestClientError(
      typeof code === "string" && publicErrors.has(code) ? code : "PORTAL_UNAVAILABLE",
      response.status,
      isObject(error) && error.retryable === true,
    );
  }
  return body;
}

function requestPath(agentId: string, requestId: string): string {
  if (!AGENT_ID.test(agentId) || !REQUEST_ID.test(requestId)) throw invalidRequest();
  return `/api/operator/access-requests/${agentId}/${requestId}`;
}

function decodeRequest(body: unknown, agentId: string, requestId: string): AccessRequest {
  try {
    const result = fromJson(AccessRequestSchema, body as JsonValue);
    assertCanonicalJson(body, toJson(AccessRequestSchema, result, { alwaysEmitImplicit: true }));
    validateRequest(result);
    if (result.agentId !== agentId || result.requestId !== requestId) throw unavailable();
    return result;
  } catch {
    throw unavailable();
  }
}

function validateRequest(item: AccessRequest): void {
  if (
    item.formatVersion !== 1 ||
    !AGENT_ID.test(item.agentId) ||
    !REQUEST_ID.test(item.requestId) ||
    !item.taskId ||
    !item.purpose ||
    !item.action ||
    !item.datasetId ||
    item.datasetRevision < 1n ||
    item.generation < 1n ||
    item.state === AccessRequestState.UNSPECIFIED ||
    typeof AccessRequestState[item.state] !== "string" ||
    typeof ProofJobState[item.proofJobState] !== "string" ||
    item.createdAtUnixSeconds < 0n ||
    item.updatedAtUnixSeconds < item.createdAtUnixSeconds ||
    item.updatedAtUnixSeconds > MAX_TIMESTAMP ||
    item.deadlineUnixSeconds <= item.createdAtUnixSeconds ||
    item.deadlineUnixSeconds > MAX_TIMESTAMP ||
    item.capability.length > 2 * 1024 * 1024 ||
    (item.proposal !== undefined && item.proposal.formatVersion !== 1)
  ) {
    throw unavailable();
  }
}

// The generated serializer defines field names, enum names, decimal strings and base64.
// Check supplied fields only: omitted implicit defaults remain valid ProtoJSON.
function assertCanonicalJson(input: unknown, canonical: JsonValue): void {
  if (Array.isArray(input)) {
    if (!Array.isArray(canonical) || input.length !== canonical.length) throw unavailable();
    input.forEach((value, index) => {
      assertCanonicalJson(value, canonical[index] as JsonValue);
    });
  } else if (isObject(input)) {
    if (!isObject(canonical)) throw unavailable();
    for (const [key, value] of Object.entries(input)) {
      if (!Object.hasOwn(canonical, key)) throw unavailable();
      assertCanonicalJson(value, canonical[key] as JsonValue);
    }
  } else if (input === null || input !== canonical) {
    throw unavailable();
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function object(value: unknown): Record<string, unknown> {
  if (!isObject(value)) throw unavailable();
  return value;
}

function validCursor(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 4096 &&
    Array.from(value).every((character) => {
      const code = character.charCodeAt(0);
      return code >= 32 && code !== 127;
    })
  );
}

function invalidRequest(): AccessRequestClientError {
  return new AccessRequestClientError("INVALID_REQUEST", 400);
}

function unavailable(): AccessRequestClientError {
  return new AccessRequestClientError("PORTAL_UNAVAILABLE", 503, true);
}
