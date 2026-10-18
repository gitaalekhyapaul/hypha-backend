export type Role = "viewer" | "approver" | "auditor" | "owner" | "team_member";

export interface RoleAssignment {
  readonly role: Role;
  readonly teamId?: string;
}

export interface AgentRolesRequest {
  readonly formatVersion: 1;
  readonly operation: "list" | "enable" | "add" | "remove";
  readonly policyVersionId: string;
  readonly approvalEpoch: string;
  readonly datasetId: string;
  readonly role?: Role;
  readonly teamId?: string;
}

export interface AgentRolesProjection {
  readonly formatVersion: 1;
  readonly policyVersionId: string;
  readonly approvalEpoch: string;
  readonly datasetId: string;
  readonly agentKeyThumbprint: string;
  readonly managed: boolean;
  readonly canEnable: boolean;
  readonly availableRoles: readonly Role[];
  readonly assignments: readonly RoleAssignment[];
}
