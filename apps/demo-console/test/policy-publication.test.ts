import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  load: vi.fn(),
  activate: vi.fn(),
  authorize: vi.fn(),
}));

vi.mock("../lib/policy-version/store.ts", () => ({ loadPolicyVersionCandidate: mocks.load }));
vi.mock("../lib/policy-version/validation.ts", () => ({ loadPolicyVersionConfig: () => ({}) }));
vi.mock("../lib/policy-activation/process.ts", () => ({ activatePolicyCandidate: mocks.activate }));
vi.mock("../lib/operator-auth/index.ts", () => ({
  operatorAuthService: async () => ({ authorizeMutation: mocks.authorize }),
  operatorAuthResponse: () => new Response(null, { status: 403 }),
}));

import { POST } from "../app/api/policies/versions/[candidateId]/publish/route.ts";
import { publishPolicyVersion } from "../lib/policy-version/publication.ts";

const identity = { familyId: "a".repeat(32), versionId: "b".repeat(32), version: 1 };
const candidate = {
  record_id: "record-1",
  policy_name: `family:${identity.familyId}:v1`,
  policy_version: "1",
  policy_hash: "c".repeat(64),
  expected_approval_epoch: "1",
};
const publication = { ...candidate, approval_epoch: "1", status: "APPROVED" };
const environment = {
  NODE_ENV: "test" as const,
  RAZIEL_POLICY_ACTIVATION_EXECUTABLE: "/tools/raziel-policy-activate",
  RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: "/run/legacy.sock",
  RAZIEL_POLICY_PUBLICATION_SOCKET_PATH: "/run/composed.sock",
};

beforeEach(() => {
  vi.clearAllMocks();
  mocks.load.mockResolvedValue({ identity, candidate });
  mocks.activate.mockResolvedValue(publication);
  mocks.authorize.mockResolvedValue({ operatorId: "operator-1" });
});

describe("owner version publication facade", () => {
  it("selects composed publication and returns only the safe bound result", async () => {
    await expect(publishPolicyVersion("candidate-1", "operator-1", environment)).resolves.toEqual({
      identity,
      publication,
    });
    expect(mocks.activate).toHaveBeenCalledWith("candidate-1", "operator-1", {
      executable: "/tools/raziel-policy-activate",
      socketPath: "/run/composed.sock",
      publication: true,
    });
  });

  it("does not fall back to legacy activation without a publication socket", async () => {
    await expect(
      publishPolicyVersion("candidate-1", "operator-1", {
        ...environment,
        RAZIEL_POLICY_PUBLICATION_SOCKET_PATH: undefined,
      }),
    ).rejects.toMatchObject({ code: "POLICY_ACTIVATION_UNAVAILABLE" });
    expect(mocks.activate).not.toHaveBeenCalled();
  });

  it.each(["record_id", "policy_name", "policy_version", "policy_hash", "approval_epoch"])(
    "rejects a mismatched publication %s",
    async (key) => {
      mocks.activate.mockResolvedValue({ ...publication, [key]: "changed" });
      await expect(
        publishPolicyVersion("candidate-1", "operator-1", environment),
      ).rejects.toMatchObject({ code: "POLICY_ACTIVATION_CONFLICT" });
    },
  );

  it("checks operator mutation authorization before touching private candidates", async () => {
    mocks.authorize.mockRejectedValue(new Error("not authorized"));
    const response = await POST(
      new Request("http://127.0.0.1/api/policies/versions/candidate-1/publish", {
        method: "POST",
      }),
      { params: Promise.resolve({ candidateId: "candidate-1" }) },
    );
    expect(response.status).toBe(403);
    expect(mocks.load).not.toHaveBeenCalled();
    expect(mocks.activate).not.toHaveBeenCalled();
  });
});
