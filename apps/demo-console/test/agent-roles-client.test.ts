import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type AgentRole,
  getAgentRoles,
  isAgentRoleId,
  updateAgentRoles,
} from "../lib/portal-client/agent-roles.ts";

const scope = {
  policyVersionId: "a".repeat(32),
  approvalEpoch: "18446744073709551615",
  datasetId: "processor-demo-v1",
};
const roles = {
  formatVersion: 1,
  ...scope,
  agentKeyThumbprint: "b".repeat(64),
  managed: true,
  canEnable: false,
  availableRoles: ["viewer", "team_member"],
  assignments: [{ role: "viewer" }, { role: "team_member", teamId: "reconciliation-team" }],
};

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function mock(body: unknown = roles, status = 200) {
  const fetch = vi.fn().mockResolvedValue(Response.json(body, { status }));
  vi.stubGlobal("fetch", fetch);
  return fetch;
}

describe("operator agent role browser client", () => {
  it("allows 130 seconds for the owner compiler and schema validation", async () => {
    mock();
    const timeout = vi.spyOn(AbortSignal, "timeout");
    await getAgentRoles("agent-one", scope);
    expect(timeout).toHaveBeenCalledWith(130_000);
  });
  it("reads the exact policy epoch and dataset using operator cookies and no cache", async () => {
    const fetch = mock();
    expect(await getAgentRoles("agent/one", scope)).toEqual(roles);
    expect(fetch).toHaveBeenCalledWith(
      `/api/operator/agents/agent%2Fone/roles?policyVersionId=${scope.policyVersionId}&approvalEpoch=${scope.approvalEpoch}&datasetId=processor-demo-v1`,
      expect.objectContaining({
        cache: "no-store",
        credentials: "same-origin",
        signal: expect.any(AbortSignal),
      }),
    );
    expect(fetch.mock.calls[0]?.[1]).not.toHaveProperty("body");
  });

  it.each([
    { operation: "enable" } as const,
    { operation: "add", role: "viewer" } as const,
    { operation: "remove", role: "team_member", teamId: "team:bank.v1" } as const,
  ])("posts the exact $operation body with the current CSRF header", async (change) => {
    const fetch = mock();
    expect(await updateAgentRoles("agent-one", scope, change, "current-csrf")).toEqual(roles);
    expect(fetch).toHaveBeenCalledWith(
      "/api/operator/agents/agent-one/roles",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ formatVersion: 1, ...scope, ...change }),
        headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "current-csrf" },
        cache: "no-store",
        credentials: "same-origin",
      }),
    );
  });

  it("projects only operator role state, not graph or diagnostic fields", async () => {
    mock({ ...roles, graph: "private", assignments: [{ role: "viewer", witness: "private" }] });
    expect(await getAgentRoles("agent-one", scope)).toEqual({
      ...roles,
      assignments: [{ role: "viewer" }],
    });
  });

  it("accepts unsupported families without inventing role options", async () => {
    const unsupported = { ...roles, managed: false, availableRoles: [], assignments: [] };
    mock(unsupported);
    expect(await getAgentRoles("agent-one", scope)).toEqual(unsupported);
  });

  it.each([
    { policyVersionId: "c".repeat(32) },
    { approvalEpoch: "2" },
    { datasetId: "bank-demo-v1" },
    { formatVersion: 2 },
    { agentKeyThumbprint: "base64-thumbprint" },
    { managed: "true" },
    { canEnable: undefined },
    { availableRoles: ["administrator"] },
    { availableRoles: ["viewer", "viewer"] },
    { assignments: [{ role: "team_member" }] },
    { assignments: [{ role: "viewer", teamId: "team-one" }] },
    { assignments: [{ role: "owner" }] },
    { assignments: [{ role: "viewer" }, { role: "viewer" }] },
  ])("rejects malformed or unbound response %#", async (patch) => {
    mock({ ...roles, ...patch });
    await expect(getAgentRoles("agent-one", scope)).rejects.toMatchObject({ status: 503 });
  });

  it.each([
    { policyVersionId: "A".repeat(32) },
    { approvalEpoch: "01" },
    { approvalEpoch: "18446744073709551616" },
    { datasetId: "../bank" },
    { datasetId: "" },
  ])("rejects invalid scope before sending %#", async (patch) => {
    const fetch = mock();
    await expect(getAgentRoles("agent-one", { ...scope, ...patch })).rejects.toMatchObject({
      status: 400,
    });
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each(["", "../team", "team/one", "a".repeat(129)])(
    "rejects unsafe team ID %s",
    async (teamId) => {
      const fetch = mock();
      await expect(
        updateAgentRoles(
          "agent-one",
          scope,
          { operation: "add", role: "team_member", teamId },
          "csrf",
        ),
      ).rejects.toMatchObject({ status: 400 });
      expect(fetch).not.toHaveBeenCalled();
      expect(isAgentRoleId(teamId)).toBe(false);
    },
  );

  it("rejects unknown roles without sending", async () => {
    const fetch = mock();
    await expect(
      updateAgentRoles(
        "agent-one",
        scope,
        { operation: "add", role: "administrator" as AgentRole },
        "csrf",
      ),
    ).rejects.toMatchObject({ status: 400 });
    expect(fetch).not.toHaveBeenCalled();
  });

  it("preserves session expiry from a non-JSON 401", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("Expired", { status: 401 })));
    await expect(getAgentRoles("agent-one", scope)).rejects.toMatchObject({
      status: 401,
      code: "OPERATOR_UNAUTHORIZED",
    });
  });

  it.each([
    { error: "ROLE_CONFLICT" },
    { error: { code: "ROLE_CONFLICT", message: "private detail" } },
  ])("keeps typed conflicts without retaining diagnostics", async (body) => {
    mock(body, 409);
    await expect(
      updateAgentRoles("agent-one", scope, { operation: "enable" }, "csrf"),
    ).rejects.toMatchObject({ status: 409, code: "ROLE_CONFLICT", message: "ROLE_CONFLICT" });
  });

  it("does not retry a mutation after a network failure", async () => {
    const fetch = vi.fn().mockRejectedValue(new Error("private"));
    vi.stubGlobal("fetch", fetch);
    await expect(
      updateAgentRoles("agent-one", scope, { operation: "enable" }, "csrf"),
    ).rejects.toMatchObject({ status: 503, message: "PORTAL_UNAVAILABLE" });
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("forwards cancellation", async () => {
    const fetch = mock();
    const controller = new AbortController();
    await getAgentRoles("agent-one", scope, controller.signal);
    const signal = fetch.mock.calls[0]?.[1]?.signal as AbortSignal;
    expect(signal.aborted).toBe(false);
    controller.abort();
    expect(signal.aborted).toBe(true);
  });
});
