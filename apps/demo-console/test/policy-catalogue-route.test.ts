import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ verify: vi.fn(), list: vi.fn() }));
vi.mock("../lib/operator-auth/index.ts", () => ({
  operatorAuthService: async () => ({ verify: mocks.verify }),
  operatorAuthResponse: () => new Response(null, { status: 401 }),
}));
vi.mock("../lib/policy-activation/process.ts", () => ({ listApprovedPolicyVersions: mocks.list }));

import { GET } from "../app/api/policies/versions/approved/route.ts";

const request = () => new Request("http://localhost/api/policies/versions/approved");
beforeEach(() => {
  vi.clearAllMocks();
  mocks.verify.mockResolvedValue({ operatorId: "operator-1" });
  mocks.list.mockResolvedValue({ formatVersion: 1, policies: [] });
  vi.stubEnv("RAZIEL_POLICY_WORKFLOW", "versioned");
  vi.stubEnv("RAZIEL_POLICY_ACTIVATION_EXECUTABLE", "/tools/raziel-policy-activate");
  vi.stubEnv("RAZIEL_POLICY_PUBLICATION_SOCKET_PATH", "/run/composed.sock");
});
afterEach(() => vi.unstubAllEnvs());

describe("operator approved catalogue route", () => {
  it("authenticates first and returns an uncached owner projection", async () => {
    const response = await GET(request());
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
    expect(await response.json()).toEqual({ formatVersion: 1, policies: [] });
    expect(mocks.list).toHaveBeenCalledWith({
      executable: "/tools/raziel-policy-activate",
      socketPath: "/run/composed.sock",
      publication: true,
    });
  });
  it("never contacts the owner for unauthenticated requests or M5 mode", async () => {
    mocks.verify.mockRejectedValue(new Error("session missing"));
    expect((await GET(request())).status).toBe(401);
    expect(mocks.list).not.toHaveBeenCalled();
    mocks.verify.mockResolvedValue({ operatorId: "operator-1" });
    vi.stubEnv("RAZIEL_POLICY_WORKFLOW", "legacy");
    expect((await GET(request())).status).toBe(409);
    expect(mocks.list).not.toHaveBeenCalled();
  });
  it("returns unavailable rather than an empty list or private error on owner failure", async () => {
    mocks.list.mockRejectedValue(new Error("private registry path"));
    const response = await GET(request());
    expect(response.status).toBe(503);
    expect(await response.json()).toEqual({ error: "POLICY_CATALOGUE_UNAVAILABLE" });
  });
});
