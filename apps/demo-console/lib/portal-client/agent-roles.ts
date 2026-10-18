import { OPERATOR_CSRF_HEADER } from "../operator-auth/types.ts";
import { PortalRequestError } from "./browser.ts";

export type AgentRole = "viewer" | "approver" | "auditor" | "owner" | "team_member";

export interface AgentRoleScope {
  readonly policyVersionId: string;
  readonly approvalEpoch: string;
  readonly datasetId: string;
}

export interface AgentRoleAssignment {
  readonly role: AgentRole;
  readonly teamId?: string;
}

export type AgentRoleChange =
  | { readonly operation: "enable" }
  | ({ readonly operation: "add" | "remove" } & AgentRoleAssignment);

export interface AgentRoles extends AgentRoleScope {
  readonly formatVersion: 1;
  readonly agentKeyThumbprint: string;
  readonly managed: boolean;
  readonly canEnable: boolean;
  readonly availableRoles: readonly AgentRole[];
  readonly assignments: readonly AgentRoleAssignment[];
}

export function isAgentRoleId(value: string): boolean {
  return /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(value);
}

export async function getAgentRoles(
  agentId: string,
  scope: AgentRoleScope,
  signal?: AbortSignal,
): Promise<AgentRoles> {
  validateScope(scope);
  const query = new URLSearchParams({
    policyVersionId: scope.policyVersionId,
    approvalEpoch: scope.approvalEpoch,
    datasetId: scope.datasetId,
  });
  return projection(await request(`${rolePath(agentId)}?${query}`, {}, signal), scope);
}

export const listAgentRoles = getAgentRoles;

export async function updateAgentRoles(
  agentId: string,
  scope: AgentRoleScope,
  change: AgentRoleChange,
  csrfToken: string,
  signal?: AbortSignal,
): Promise<AgentRoles> {
  validateScope(scope);
  if (
    !["enable", "add", "remove"].includes(change.operation) ||
    (change.operation !== "enable" && !validAssignment(change))
  ) {
    throw new PortalRequestError("INVALID_ROLE_CHANGE", 400);
  }
  return projection(
    await request(
      rolePath(agentId),
      {
        method: "POST",
        headers: { "Content-Type": "application/json", [OPERATOR_CSRF_HEADER]: csrfToken },
        body: JSON.stringify({
          formatVersion: 1,
          policyVersionId: scope.policyVersionId,
          approvalEpoch: scope.approvalEpoch,
          datasetId: scope.datasetId,
          operation: change.operation,
          ...(change.operation === "enable"
            ? {}
            : { role: change.role, ...(change.teamId ? { teamId: change.teamId } : {}) }),
        }),
      },
      signal,
    ),
    scope,
  );
}

function rolePath(agentId: string): string {
  if (!agentId) throw new PortalRequestError("INVALID_AGENT_ID", 400);
  return `/api/operator/agents/${encodeURIComponent(agentId)}/roles`;
}

function validateScope(scope: AgentRoleScope) {
  if (
    !/^[0-9a-f]{32}$/.test(scope.policyVersionId) ||
    !/^[1-9][0-9]{0,19}$/.test(scope.approvalEpoch) ||
    BigInt(scope.approvalEpoch) > 18_446_744_073_709_551_615n ||
    !isAgentRoleId(scope.datasetId)
  ) {
    throw new PortalRequestError("INVALID_ROLE_SCOPE", 400);
  }
}

async function request(path: string, init: RequestInit, signal?: AbortSignal): Promise<unknown> {
  let response: Response;
  try {
    const timeout = AbortSignal.timeout(130_000);
    response = await fetch(path, {
      ...init,
      cache: "no-store",
      credentials: "same-origin",
      signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
    });
  } catch {
    throw unavailable();
  }
  if (response.status === 401) throw new PortalRequestError("OPERATOR_UNAUTHORIZED", 401);
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new PortalRequestError("PORTAL_UNAVAILABLE", response.ok ? 503 : response.status);
  }
  if (!response.ok) {
    const error = typeof body === "object" && body !== null ? Reflect.get(body, "error") : null;
    const code = typeof error === "string" ? error : error?.code;
    throw new PortalRequestError(
      typeof code === "string" ? code : "PORTAL_UNAVAILABLE",
      response.status,
    );
  }
  return body;
}

function projection(input: unknown, scope: AgentRoleScope): AgentRoles {
  const value = object(input);
  if (
    value.formatVersion !== 1 ||
    value.policyVersionId !== scope.policyVersionId ||
    value.approvalEpoch !== scope.approvalEpoch ||
    value.datasetId !== scope.datasetId ||
    typeof value.agentKeyThumbprint !== "string" ||
    !/^[0-9a-f]{64}$/.test(value.agentKeyThumbprint) ||
    typeof value.managed !== "boolean" ||
    typeof value.canEnable !== "boolean" ||
    !Array.isArray(value.availableRoles) ||
    !value.availableRoles.every(isRole) ||
    new Set(value.availableRoles).size !== value.availableRoles.length ||
    !Array.isArray(value.assignments)
  ) {
    throw unavailable();
  }
  const availableRoles: AgentRole[] = value.availableRoles;
  const assignments = value.assignments.map((input: unknown) => {
    const assignment = object(input);
    if (!validAssignment(assignment) || !availableRoles.includes(assignment.role))
      throw unavailable();
    return {
      role: assignment.role,
      ...(assignment.teamId === undefined ? {} : { teamId: assignment.teamId }),
    };
  });
  if (
    new Set(assignments.map((item) => `${item.role}:${item.teamId ?? ""}`)).size !==
    assignments.length
  )
    throw unavailable();
  return {
    formatVersion: 1,
    policyVersionId: scope.policyVersionId,
    approvalEpoch: scope.approvalEpoch,
    datasetId: scope.datasetId,
    agentKeyThumbprint: value.agentKeyThumbprint,
    managed: value.managed,
    canEnable: value.canEnable,
    availableRoles,
    assignments,
  };
}

function isRole(value: unknown): value is AgentRole {
  return (
    value === "viewer" ||
    value === "approver" ||
    value === "auditor" ||
    value === "owner" ||
    value === "team_member"
  );
}

function validAssignment(value: {
  readonly role?: unknown;
  readonly teamId?: unknown;
}): value is AgentRoleAssignment {
  return (
    isRole(value.role) &&
    (value.role === "team_member"
      ? typeof value.teamId === "string" && isAgentRoleId(value.teamId)
      : value.teamId === undefined)
  );
}

function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw unavailable();
  return value as Record<string, unknown>;
}

function unavailable(): PortalRequestError {
  return new PortalRequestError("PORTAL_UNAVAILABLE", 503);
}
