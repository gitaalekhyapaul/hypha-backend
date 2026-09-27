import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultPolicyBuilderState } from "../lib/policy-builder/model.ts";
import { buildPolicyVersionRequest } from "../lib/policy-builder/versioned.ts";
import {
  loadPolicyVersion,
  preparePolicyVersion,
  publishPolicyVersion,
} from "../lib/portal-client/policy-versions.ts";

const identity = { familyId: "a".repeat(32), versionId: "b".repeat(32), version: 2 };
const timestamp = { seconds: "1790500000", nanos: 0 as const };
const prepared = {
  identity,
  candidate: {
    candidate_id: "candidate-test",
    compiler_version: "1.54.0",
    record_id: "record-test",
    policy_name: `family:${identity.familyId}:v2`,
    policy_version: "2",
    resource_type: `dataset_${identity.versionId}`,
    permission: "query" as const,
    rule_summary: "Direct viewer",
    source_schema_digest: "c".repeat(64),
    policy_hash: "d".repeat(64),
    expected_approval_epoch: "1",
    valid_from: timestamp,
    valid_until: timestamp,
    created_at: timestamp,
    expires_at: timestamp,
    status: "COMPILED" as const,
  },
};
const published = {
  identity,
  publication: {
    record_id: prepared.candidate.record_id,
    policy_name: prepared.candidate.policy_name,
    policy_version: "2",
    policy_hash: prepared.candidate.policy_hash,
    approval_epoch: "1",
    status: "APPROVED" as const,
    registered_at: timestamp,
    valid_from: timestamp,
    valid_until: timestamp,
    registry_generation: "3",
  },
};

afterEach(() => vi.unstubAllGlobals());

describe("versioned policy browser client", () => {
  it("posts the exact draft with CSRF, no caching, and same-origin credentials", async () => {
    const fetch = vi.fn().mockResolvedValue(Response.json(prepared));
    vi.stubGlobal("fetch", fetch);
    const request = buildPolicyVersionRequest(
      defaultPolicyBuilderState(),
      "candidate-test",
      identity.familyId,
    );
    await expect(preparePolicyVersion(request, "csrf-test")).resolves.toEqual(prepared);
    expect(fetch).toHaveBeenCalledWith(
      "/api/policies/versions",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "csrf-test" },
        body: JSON.stringify(request),
        cache: "no-store",
        credentials: "same-origin",
        signal: expect.any(AbortSignal),
      }),
    );
  });

  it("refreshes the exact candidate through its version endpoint", async () => {
    const fetch = vi.fn().mockResolvedValue(Response.json(prepared));
    vi.stubGlobal("fetch", fetch);
    await expect(loadPolicyVersion("candidate-test")).resolves.toEqual(prepared);
    expect(fetch).toHaveBeenCalledWith(
      "/api/policies/versions/candidate-test",
      expect.objectContaining({ cache: "no-store" }),
    );
  });

  it("publishes with CSRF and no caller-selected identity body", async () => {
    const fetch = vi.fn().mockResolvedValue(Response.json(published));
    vi.stubGlobal("fetch", fetch);
    await expect(publishPolicyVersion(prepared, "csrf-test")).resolves.toEqual(published);
    const [path, options] = fetch.mock.calls[0] ?? [];
    expect(path).toBe("/api/policies/versions/candidate-test/publish");
    expect(options).toMatchObject({
      method: "POST",
      headers: { "x-raziel-csrf-token": "csrf-test" },
    });
    expect(options.body).toBeUndefined();
  });

  it("discards non-projection fields", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        Response.json({
          ...prepared,
          privateSource: "not-for-state",
          candidate: { ...prepared.candidate, witness: "not-for-state" },
        }),
      ),
    );
    expect(await loadPolicyVersion("candidate-test")).toEqual(prepared);
  });

  it.each([
    "candidate_id",
    "policy_name",
    "resource_type",
    "policy_version",
    "expected_approval_epoch",
    "status",
    "policy_hash",
  ])("rejects an invalid candidate %s", async (field) => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          Response.json({ ...prepared, candidate: { ...prepared.candidate, [field]: "wrong" } }),
        ),
    );
    await expect(loadPolicyVersion("candidate-test")).rejects.toMatchObject({
      code: "PORTAL_UNAVAILABLE",
    });
  });

  it("rejects another family from preparation", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json(prepared)));
    await expect(
      preparePolicyVersion(
        buildPolicyVersionRequest(defaultPolicyBuilderState(), "candidate-test", "e".repeat(32)),
        "csrf",
      ),
    ).rejects.toMatchObject({ status: 503 });
  });

  it.each([
    "record_id",
    "policy_name",
    "policy_version",
    "policy_hash",
    "approval_epoch",
    "status",
  ])("rejects an unbound publication %s", async (field) => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        Response.json({
          ...published,
          publication: { ...published.publication, [field]: "wrong" },
        }),
      ),
    );
    await expect(publishPolicyVersion(prepared, "csrf")).rejects.toMatchObject({ status: 503 });
  });

  it("preserves session expiry with a non-JSON response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("expired", { status: 401 })));
    await expect(loadPolicyVersion("candidate-test")).rejects.toMatchObject({ status: 401 });
  });

  it("preserves typed conflict responses without echoing diagnostics", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          Response.json(
            { error: "CANDIDATE_CONFLICT", message: "private diagnostic" },
            { status: 409 },
          ),
        ),
    );
    await expect(loadPolicyVersion("candidate-test")).rejects.toMatchObject({
      status: 409,
      code: "CANDIDATE_CONFLICT",
      message: "CANDIDATE_CONFLICT",
    });
  });

  it.each(["network", "invalid-json"])("treats %s outcomes as uncertain", async (kind) => {
    const fetch =
      kind === "network"
        ? vi.fn().mockRejectedValue(new Error("offline"))
        : vi.fn().mockResolvedValue(new Response("intermediary", { status: 502 }));
    vi.stubGlobal("fetch", fetch);
    await expect(loadPolicyVersion("candidate-test")).rejects.toMatchObject({ status: 503 });
  });
});
