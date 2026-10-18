import { spawn } from "node:child_process";
import { AgentAuthError, type AgentRegistration } from "../agent-auth/types.ts";
import { PolicyActivationError } from "../policy-activation/types.ts";
import { loadPolicyActivationConfig } from "../policy-activation/validation.ts";
import type { AgentRolesProjection, AgentRolesRequest, Role, RoleAssignment } from "./types.ts";

const ROLES: readonly Role[] = ["viewer", "approver", "auditor", "owner", "team_member"];
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const MAX_INPUT_BYTES = 4096;
const MAX_OUTPUT_BYTES = 64 * 1024;
const DEADLINE_MS = 125_000;

export function parseAgentRolesRequest(input: unknown, mutation: boolean): AgentRolesRequest {
  const value = object(
    input,
    ["formatVersion", "operation", "policyVersionId", "approvalEpoch", "datasetId"],
    ["role", "teamId"],
  );
  if (
    value.formatVersion !== 1 ||
    typeof value.policyVersionId !== "string" ||
    value.policyVersionId.length !== 32 ||
    !/^[0-9a-f]{32}$/.test(value.policyVersionId) ||
    !positiveU64(value.approvalEpoch) ||
    !safeId(value.datasetId) ||
    !(mutation
      ? value.operation === "enable" || value.operation === "add" || value.operation === "remove"
      : value.operation === "list")
  ) {
    throw invalid();
  }
  const assignment =
    value.operation === "add" || value.operation === "remove"
      ? roleAssignment({
          role: value.role,
          ...(Object.hasOwn(value, "teamId") ? { teamId: value.teamId } : {}),
        })
      : undefined;
  if (!assignment && (Object.hasOwn(value, "role") || Object.hasOwn(value, "teamId"))) {
    throw invalid();
  }
  return {
    formatVersion: 1,
    operation: value.operation as AgentRolesRequest["operation"],
    policyVersionId: value.policyVersionId,
    approvalEpoch: value.approvalEpoch,
    datasetId: value.datasetId,
    ...assignment,
  };
}

export async function executeAgentRoles(
  agent: AgentRegistration,
  input: AgentRolesRequest,
  environment: NodeJS.ProcessEnv = process.env,
): Promise<AgentRolesProjection> {
  const request = parseAgentRolesRequest(input, input.operation !== "list");
  const key = Buffer.from(agent.keyThumbprint, "base64");
  if (key.length !== 32 || key.toString("base64") !== agent.keyThumbprint) throw unavailable();
  const bound = { ...request, agentKeyThumbprint: key.toString("hex") };
  const body = Buffer.from(JSON.stringify(bound), "utf8");
  if (body.length > MAX_INPUT_BYTES) throw invalid();
  const config = loadPolicyActivationConfig({
    ...environment,
    RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: environment.RAZIEL_POLICY_PUBLICATION_SOCKET_PATH,
  });
  const output = await new Promise<Buffer>((resolve, reject) => {
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    let bytes = 0;
    let failed = false;
    const child = spawn(config.executable, ["--agent-roles"], {
      env: {
        NODE_ENV: environment.NODE_ENV ?? "production",
        PATH: environment.PATH ?? "/usr/local/bin:/usr/bin:/bin",
        RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: config.socketPath,
        RAZIEL_POLICY_PUBLICATION_MODE: "composed",
      },
      shell: false,
      stdio: ["pipe", "pipe", "pipe"],
    });
    const fail = () => {
      failed = true;
      child.kill("SIGKILL");
    };
    const collect = (chunks: Buffer[], chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > MAX_OUTPUT_BYTES) fail();
      else if (!failed) chunks.push(chunk);
    };
    child.stdout.on("data", (chunk: Buffer) => collect(stdout, chunk));
    child.stderr.on("data", (chunk: Buffer) => collect(stderr, chunk));
    child.stdout.on("error", fail);
    child.stderr.on("error", fail);
    child.stdin.on("error", fail);
    child.on("error", () => {
      failed = true;
    });
    const timer = setTimeout(fail, DEADLINE_MS);
    timer.unref();
    // Wait for close after a kill or transport error, so the child is not left running.
    child.on("close", (code) => {
      clearTimeout(timer);
      if (failed) reject(unavailable());
      else if (code !== 0) reject(childFailure(Buffer.concat(stderr)));
      else resolve(Buffer.concat(stdout));
    });
    child.stdin.end(body);
  });
  try {
    const value = object(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(output)), [
      "formatVersion",
      "policyVersionId",
      "approvalEpoch",
      "datasetId",
      "agentKeyThumbprint",
      "managed",
      "canEnable",
      "availableRoles",
      "assignments",
    ]);
    if (
      value.formatVersion !== 1 ||
      value.policyVersionId !== bound.policyVersionId ||
      value.approvalEpoch !== bound.approvalEpoch ||
      value.datasetId !== bound.datasetId ||
      value.agentKeyThumbprint !== bound.agentKeyThumbprint ||
      typeof value.managed !== "boolean" ||
      typeof value.canEnable !== "boolean" ||
      !Array.isArray(value.availableRoles) ||
      value.availableRoles.length > ROLES.length ||
      !value.availableRoles.every(isRole) ||
      new Set(value.availableRoles).size !== value.availableRoles.length ||
      !Array.isArray(value.assignments)
    ) {
      throw unavailable();
    }
    const assignments = value.assignments.map(roleAssignment);
    if (new Set(assignments.map((entry) => JSON.stringify(entry))).size !== assignments.length) {
      throw unavailable();
    }
    return {
      formatVersion: 1,
      policyVersionId: bound.policyVersionId,
      approvalEpoch: bound.approvalEpoch,
      datasetId: bound.datasetId,
      agentKeyThumbprint: bound.agentKeyThumbprint,
      managed: value.managed,
      canEnable: value.canEnable,
      availableRoles: value.availableRoles,
      assignments,
    };
  } catch {
    throw unavailable();
  }
}

function roleAssignment(input: unknown): RoleAssignment {
  const value = object(input, ["role"], ["teamId"]);
  if (!isRole(value.role)) throw invalid();
  if (value.role === "team_member") {
    if (!safeId(value.teamId)) throw invalid();
    return { role: value.role, teamId: value.teamId };
  }
  if (Object.hasOwn(value, "teamId")) throw invalid();
  return { role: value.role };
}

function object(
  input: unknown,
  required: readonly string[],
  optional: readonly string[] = [],
): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) throw invalid();
  const value = input as Record<string, unknown>;
  const prototype = Object.getPrototypeOf(value);
  if (
    (prototype !== Object.prototype && prototype !== null) ||
    required.some((key) => !Object.hasOwn(value, key)) ||
    Reflect.ownKeys(value).some(
      (key) => typeof key !== "string" || (!required.includes(key) && !optional.includes(key)),
    )
  ) {
    throw invalid();
  }
  return value;
}

function positiveU64(input: unknown): input is string {
  return (
    typeof input === "string" &&
    /^[1-9][0-9]{0,19}$/.exec(input)?.[0] === input &&
    BigInt(input) <= 18_446_744_073_709_551_615n
  );
}

function safeId(input: unknown): input is string {
  return typeof input === "string" && SAFE_ID.exec(input)?.[0] === input;
}

function isRole(input: unknown): input is Role {
  return typeof input === "string" && ROLES.includes(input as Role);
}

function invalid(): AgentAuthError {
  return new AgentAuthError("INVALID_REQUEST");
}

function unavailable(): PolicyActivationError {
  return new PolicyActivationError("POLICY_ACTIVATION_UNAVAILABLE");
}

function childFailure(stderr: Buffer): PolicyActivationError {
  switch (stderr.toString("utf8").trim()) {
    case "raziel-policy-activate: invalid policy-activation request":
      return new PolicyActivationError("INVALID_POLICY_ACTIVATION_REQUEST");
    case "raziel-policy-activate: policy activation conflicts with trusted state":
      return new PolicyActivationError("POLICY_ACTIVATION_CONFLICT");
    case "raziel-policy-activate: policy candidate was rejected":
      return new PolicyActivationError("POLICY_ACTIVATION_REJECTED");
    default:
      return unavailable();
  }
}
