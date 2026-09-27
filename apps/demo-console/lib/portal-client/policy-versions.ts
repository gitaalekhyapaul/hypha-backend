import { OPERATOR_CSRF_HEADER } from "../operator-auth/types.ts";
import type { PolicyActivationProjection } from "../policy-activation/types.ts";
import type { PolicyCandidateProjection, PublicTimestamp } from "../policy-candidate/types.ts";
import type { PolicyVersionIdentity } from "../policy-draft/types.ts";
import { validatePolicyVersionIdentity } from "../policy-draft/validation.ts";
import type {
  PolicyVersionCandidate,
  PreparePolicyVersionRequest,
} from "../policy-version/types.ts";
import { PortalRequestError } from "./browser.ts";

export interface PolicyVersionPublication {
  readonly identity: PolicyVersionIdentity;
  readonly publication: PolicyActivationProjection;
}

export async function preparePolicyVersion(
  request: PreparePolicyVersionRequest,
  csrfToken: string,
): Promise<PolicyVersionCandidate> {
  const result = candidate(
    await requestVersion("/api/policies/versions", mutation(csrfToken, request)),
    request.draft.candidateId,
  );
  if (request.familyId !== undefined && result.identity.familyId !== request.familyId)
    throw unavailable();
  return result;
}

export async function loadPolicyVersion(candidateId: string): Promise<PolicyVersionCandidate> {
  return candidate(await requestVersion(versionPath(candidateId)), candidateId);
}

export async function publishPolicyVersion(
  prepared: PolicyVersionCandidate,
  csrfToken: string,
): Promise<PolicyVersionPublication> {
  const value = object(
    await requestVersion(
      `${versionPath(prepared.candidate.candidate_id)}/publish`,
      mutation(csrfToken),
    ),
  );
  const identity = versionIdentity(value.identity);
  const data = object(value.publication);
  const expected = prepared.candidate;
  if (
    identity.familyId !== prepared.identity.familyId ||
    identity.versionId !== prepared.identity.versionId ||
    identity.version !== prepared.identity.version ||
    data.status !== "APPROVED" ||
    data.record_id !== expected.record_id ||
    data.policy_name !== expected.policy_name ||
    data.policy_version !== expected.policy_version ||
    data.policy_hash !== expected.policy_hash ||
    data.approval_epoch !== expected.expected_approval_epoch
  )
    throw unavailable();
  return {
    identity,
    publication: {
      record_id: expected.record_id,
      policy_name: expected.policy_name,
      policy_version: expected.policy_version,
      policy_hash: expected.policy_hash,
      approval_epoch: expected.expected_approval_epoch,
      status: "APPROVED",
      registered_at: timestamp(data.registered_at),
      valid_from: timestamp(data.valid_from),
      valid_until: timestamp(data.valid_until),
      registry_generation: decimal(data.registry_generation),
    },
  };
}

function candidate(input: unknown, candidateId: string): PolicyVersionCandidate {
  const value = object(input);
  const identity = versionIdentity(value.identity);
  const data = object(value.candidate);
  if (
    data.candidate_id !== candidateId ||
    data.status !== "COMPILED" ||
    data.permission !== "query" ||
    data.policy_name !== `family:${identity.familyId}:v${identity.version}` ||
    data.policy_version !== String(identity.version) ||
    data.resource_type !== `dataset_${identity.versionId}` ||
    data.expected_approval_epoch !== "1"
  )
    throw unavailable();
  // Keep only the existing safe projection. Never retain arbitrary response fields.
  const projection: PolicyCandidateProjection = {
    candidate_id: candidateId,
    status: "COMPILED",
    permission: "query",
    policy_name: data.policy_name as string,
    policy_version: String(identity.version),
    resource_type: data.resource_type as string,
    expected_approval_epoch: "1",
    compiler_version: text(data.compiler_version),
    record_id: text(data.record_id),
    rule_summary: text(data.rule_summary),
    policy_hash: digest(data.policy_hash),
    source_schema_digest: digest(data.source_schema_digest),
    valid_from: timestamp(data.valid_from),
    valid_until: timestamp(data.valid_until),
    created_at: timestamp(data.created_at),
    expires_at: timestamp(data.expires_at),
  };
  return { identity, candidate: projection };
}

function versionIdentity(value: unknown): PolicyVersionIdentity {
  try {
    return validatePolicyVersionIdentity(value);
  } catch {
    throw unavailable();
  }
}

function versionPath(id: string): string {
  return `/api/policies/versions/${encodeURIComponent(id)}`;
}

function mutation(csrfToken: string, body?: unknown): RequestInit {
  return {
    method: "POST",
    headers: {
      [OPERATOR_CSRF_HEADER]: csrfToken,
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  };
}

async function requestVersion(path: string, init: RequestInit = {}): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(path, {
      ...init,
      cache: "no-store",
      credentials: "same-origin",
      signal: AbortSignal.timeout(150_000),
    });
  } catch {
    throw unavailable();
  }
  if (response.status === 401) throw new PortalRequestError("OPERATOR_UNAUTHORIZED", 401);
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw unavailable();
  }
  if (!response.ok) {
    const code = typeof body === "object" && body !== null ? Reflect.get(body, "error") : undefined;
    throw new PortalRequestError(
      typeof code === "string" ? code : "PORTAL_UNAVAILABLE",
      response.status,
    );
  }
  return body;
}

function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw unavailable();
  return value as Record<string, unknown>;
}

function text(value: unknown): string {
  if (typeof value !== "string" || value.length === 0 || value.length > 4096) throw unavailable();
  return value;
}

function decimal(value: unknown): string {
  if (typeof value !== "string" || !/^[1-9][0-9]{0,19}$/.test(value)) throw unavailable();
  return value;
}

function digest(value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw unavailable();
  return value;
}

function timestamp(value: unknown): PublicTimestamp {
  const data = object(value);
  if (
    typeof data.seconds !== "string" ||
    !/^(0|[1-9][0-9]{0,11})$/.test(data.seconds) ||
    Number(data.seconds) > 253_402_300_799 ||
    data.nanos !== 0
  )
    throw unavailable();
  return { seconds: data.seconds, nanos: 0 };
}

function unavailable(): PortalRequestError {
  return new PortalRequestError("PORTAL_UNAVAILABLE", 503);
}
