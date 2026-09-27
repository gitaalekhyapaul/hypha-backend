import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  authorize: vi.fn(),
  activate: vi.fn(),
  graph: vi.fn(),
  publish: vi.fn(),
  session: vi.fn(),
  capability: vi.fn(),
}));
vi.mock("../lib/operator-auth/index.ts", () => ({
  operatorAuthService: async () => ({ authorizeMutation: mocks.authorize }),
  operatorAuthResponse: () => new Response(null, { status: 403 }),
}));
vi.mock("../lib/policy-activation/index.ts", () => ({
  activatePolicyCandidate: mocks.activate,
  loadPolicyActivationConfig: () => ({}),
  policyActivationResponse: () => new Response(null, { status: 503 }),
}));
vi.mock("../lib/access-graph/index.ts", () => ({
  publishAccessGraph: mocks.graph,
  loadAccessGraphConfig: () => ({}),
  accessGraphResponse: () => new Response(null, { status: 503 }),
}));
vi.mock("../lib/policy-version/publication.ts", () => ({
  publishPolicyVersion: mocks.publish,
}));
vi.mock("../lib/agent-capability/session-store.ts", () => ({
  createOperatorAgentSession: mocks.session,
}));
vi.mock("../lib/agent-capability/lifecycle.ts", () => ({
  createCapability: mocks.capability,
}));

import { POST as graph } from "../app/api/access-graphs/route.ts";
import { POST as session } from "../app/api/agent-sessions/route.ts";
import { POST as capability } from "../app/api/capabilities/route.ts";
import { POST as activate } from "../app/api/policies/candidates/[candidateId]/activate/route.ts";
import { POST as publish } from "../app/api/policies/versions/[candidateId]/publish/route.ts";
import { policyWorkflow, policyWorkflowResponse } from "../lib/policy-workflow.ts";

beforeEach(() => {
  vi.resetAllMocks();
  mocks.authorize.mockResolvedValue({ operatorId: "operator-1" });
  mocks.publish.mockResolvedValue({ identity: {}, publication: {} });
});
afterEach(() => vi.unstubAllEnvs());

describe("exclusive policy workflow", () => {
  it("defaults to legacy and rejects unknown deployment modes", () => {
    expect(policyWorkflow({ NODE_ENV: "test" })).toBe("legacy");
    const environment = { NODE_ENV: "test" as const, RAZIEL_POLICY_WORKFLOW: "typo" };
    expect(() => policyWorkflow(environment)).toThrow();
    expect(policyWorkflowResponse("versioned", environment)?.status).toBe(503);
  });

  it("does not admit legacy owner writes in versioned mode", async () => {
    vi.stubEnv("RAZIEL_POLICY_WORKFLOW", "versioned");
    for (const route of [graph, activate, session, capability]) {
      const response = await route(request(), context());
      expect(response.status).toBe(409);
      expect(response.headers.get("Cache-Control")).toBe("no-store");
      expect(await response.json()).toEqual({ error: "POLICY_WORKFLOW_UNAVAILABLE" });
    }
    expect(mocks.activate).not.toHaveBeenCalled();
    expect(mocks.graph).not.toHaveBeenCalled();
    expect(mocks.session).not.toHaveBeenCalled();
    expect(mocks.capability).not.toHaveBeenCalled();
  });

  it("rejects version publication in legacy mode", async () => {
    vi.stubEnv("RAZIEL_POLICY_WORKFLOW", "legacy");
    expect((await publish(request(), context())).status).toBe(409);
    expect(mocks.publish).not.toHaveBeenCalled();
  });

  it("publishes through the explicitly selected versioned owner", async () => {
    vi.stubEnv("RAZIEL_POLICY_WORKFLOW", "versioned");
    expect((await publish(request(), context())).status).toBe(200);
    expect(mocks.publish).toHaveBeenCalledWith("candidate-1", "operator-1");
  });

  it.each([graph, activate, publish])(
    "authenticates before checking deployment mode",
    async (route) => {
      vi.stubEnv("RAZIEL_POLICY_WORKFLOW", "typo");
      mocks.authorize.mockRejectedValue(new Error("unauthorized"));
      expect((await route(request(), context())).status).toBe(403);
      expect(mocks.activate).not.toHaveBeenCalled();
      expect(mocks.graph).not.toHaveBeenCalled();
      expect(mocks.publish).not.toHaveBeenCalled();
    },
  );
});

function request() {
  return new Request("http://127.0.0.1/api/operator-test", { method: "POST" });
}

function context() {
  return { params: Promise.resolve({ candidateId: "candidate-1" }) };
}
