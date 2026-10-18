import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type ApprovedPolicyCatalogue,
  InvalidPolicyCatalogueError,
  parseApprovedPolicyCatalogue,
  recentApprovedPolicies,
} from "../lib/policy-version/catalogue-types.ts";
import { getApprovedPolicyCatalogue } from "../lib/portal-client/policy-catalogue.ts";

const familyId = "a".repeat(32);
const versionId = "b".repeat(32);
const entry = {
  identity: { familyId, versionId, version: 2 },
  recordId: "policy-approved-record",
  policyHash: "c".repeat(64),
  policyName: `family:${familyId}:v2`,
  policyVersion: "2",
  resourceType: `dataset_${versionId}`,
  permission: "query" as const,
  approvalEpoch: "1",
  validFromUnixSeconds: "1790500000",
  validUntilUnixSeconds: "1790586400",
  requiredGrantFields: ["teamId", "isApproved"] as const,
};
const catalogue: ApprovedPolicyCatalogue = { formatVersion: 1, policies: [entry] };

afterEach(() => vi.unstubAllGlobals());

describe("approved policy catalogue display order", () => {
  it("sorts by recent validity start rather than API order, version number, or expiry", () => {
    const older = { ...entry, validFromUnixSeconds: "1790400000", policyVersion: "9" };
    const newest = { ...entry, validFromUnixSeconds: "1790600000", policyVersion: "1" };
    const input = [older, newest, entry];
    expect(recentApprovedPolicies(input)).toEqual([newest, entry, older]);
    expect(input).toEqual([older, newest, entry]);
  });

  it("breaks equal-time ties by version, exact approval epoch, then Version ID", () => {
    const epoch = { ...entry, approvalEpoch: "18446744073709551614" };
    const higherEpoch = { ...epoch, approvalEpoch: "18446744073709551615" };
    const laterVersion = { ...entry, identity: { ...entry.identity, version: 3 } };
    const a = { ...entry, identity: { ...entry.identity, versionId: "a".repeat(32) } };
    const c = { ...entry, identity: { ...entry.identity, versionId: "c".repeat(32) } };
    expect(recentApprovedPolicies([c, epoch, laterVersion, higherEpoch, a])).toEqual([
      laterVersion,
      higherEpoch,
      epoch,
      a,
      c,
    ]);
  });
});

describe("approved policy catalogue contract", () => {
  it("projects an exact safe catalogue without retaining input references", () => {
    const result = parseApprovedPolicyCatalogue(catalogue);
    expect(result).toEqual(catalogue);
    expect(result).not.toBe(catalogue);
    expect(result.policies[0]).not.toBe(entry);
    expect(result.policies[0]?.identity).not.toBe(entry.identity);
    expect(result.policies[0]?.requiredGrantFields).not.toBe(entry.requiredGrantFields);
    expect(parseApprovedPolicyCatalogue({ formatVersion: 1, policies: [] })).toEqual({
      formatVersion: 1,
      policies: [],
    });
  });

  it("accepts 32 distinct installed versions and rejects larger catalogues", () => {
    const policies = Array.from({ length: 33 }, (_, index) => {
      const id = index.toString(16).padStart(32, "0");
      return {
        ...entry,
        identity: { ...entry.identity, versionId: id },
        resourceType: `dataset_${id}`,
      };
    });
    expect(
      parseApprovedPolicyCatalogue({ formatVersion: 1, policies: policies.slice(0, 32) }).policies,
    ).toHaveLength(32);
    expect(() => parseApprovedPolicyCatalogue({ formatVersion: 1, policies })).toThrow(
      InvalidPolicyCatalogueError,
    );
    expect(() =>
      parseApprovedPolicyCatalogue({ formatVersion: 1, policies: [entry, entry] }),
    ).toThrow(InvalidPolicyCatalogueError);
  });

  it("accepts the exact integer boundaries without numeric rounding", () => {
    const version = Number.MAX_SAFE_INTEGER;
    const policies = [
      {
        ...entry,
        identity: { ...entry.identity, version },
        policyName: `family:${familyId}:v${version}`,
        policyVersion: String(version),
        approvalEpoch: "18446744073709551615",
        validUntilUnixSeconds: "253402300799",
      },
    ];
    expect(parseApprovedPolicyCatalogue({ formatVersion: 1, policies }).policies).toEqual(policies);
  });

  it.each([
    { ...catalogue, schema: "private" },
    { ...catalogue, policies: [{ ...entry, policyIR: "private" }] },
    { ...catalogue, policies: [{ ...entry, identity: { ...entry.identity, path: "private" } }] },
    { ...catalogue, formatVersion: 2 },
    { ...catalogue, policies: {} },
    null,
    [],
    Object.create(catalogue),
  ])("rejects unsupported fields and shapes without exposing their content", (value) => {
    expect(() => parseApprovedPolicyCatalogue(value)).toThrow("Invalid approved policy catalogue");
  });

  it.each([
    ["recordId", ""],
    ["recordId", "x".repeat(257)],
    ["policyHash", "C".repeat(64)],
    ["policyHash", "c".repeat(63)],
    ["policyName", `family:${familyId}:v3`],
    ["policyVersion", "02"],
    ["resourceType", "dataset"],
    ["permission", "write"],
    ["approvalEpoch", "01"],
    ["approvalEpoch", "0"],
    ["approvalEpoch", "1.0"],
    ["approvalEpoch", "18446744073709551616"],
    ["validFromUnixSeconds", "0"],
    ["validFromUnixSeconds", "-1"],
    ["validUntilUnixSeconds", entry.validFromUnixSeconds],
    ["validUntilUnixSeconds", "1"],
    ["validUntilUnixSeconds", "253402300800"],
    ["validUntilUnixSeconds", "01790586400"],
  ])("rejects invalid %s=%j", (field, value) => {
    expect(() =>
      parseApprovedPolicyCatalogue({
        formatVersion: 1,
        policies: [{ ...entry, [field as string]: value }],
      }),
    ).toThrow(InvalidPolicyCatalogueError);
  });

  it.each([
    { ...entry.identity, familyId: "A".repeat(32) },
    { ...entry.identity, versionId: "b".repeat(31) },
    { ...entry.identity, version: 0 },
    { ...entry.identity, version: 1.5 },
    { ...entry.identity, version: "2" },
    { ...entry.identity, version: Number.MAX_SAFE_INTEGER + 1 },
  ])("rejects invalid identities", (identity) => {
    expect(() =>
      parseApprovedPolicyCatalogue({
        formatVersion: 1,
        policies: [{ ...entry, identity }],
      }),
    ).toThrow(InvalidPolicyCatalogueError);
  });

  it("requires every field rather than accepting partial approval records", () => {
    const { recordId: _recordId, ...partial } = entry;
    expect(() => parseApprovedPolicyCatalogue({ formatVersion: 1, policies: [partial] })).toThrow(
      InvalidPolicyCatalogueError,
    );
  });

  it.each(
    [
      [],
      ["teamId"],
      ["organizationId"],
      ["approvedAmount", "isApproved", "isSuspended", "isOverride"],
      ["teamId", "organizationId", "approvedAmount", "isApproved", "isSuspended", "isOverride"],
    ].map((requiredGrantFields) => ({ requiredGrantFields })),
  )("accepts bounded operator input names $requiredGrantFields", ({ requiredGrantFields }) => {
    const result = parseApprovedPolicyCatalogue({
      formatVersion: 1,
      policies: [{ ...entry, requiredGrantFields }],
    });
    expect(result.policies[0]?.requiredGrantFields).toEqual(requiredGrantFields);
  });

  it.each(
    [
      undefined,
      null,
      "teamId",
      {},
      ["unknown"],
      ["approved_amount"],
      ["TeamId"],
      ["teamId", "teamId"],
      ["isApproved", "isApproved"],
      ["teamId", 1],
      [{ name: "teamId", value: "private" }],
      Array(1),
      Array(7).fill("teamId"),
    ].map((requiredGrantFields) => ({ requiredGrantFields })),
  )("rejects invalid required field projection $requiredGrantFields", ({ requiredGrantFields }) => {
    expect(() =>
      parseApprovedPolicyCatalogue({
        formatVersion: 1,
        policies: [{ ...entry, requiredGrantFields }],
      }),
    ).toThrow(InvalidPolicyCatalogueError);
  });
});

describe("approved policy catalogue browser client", () => {
  it("loads the operator catalogue without caching or a mutation body", async () => {
    const fetch = vi.fn().mockResolvedValue(Response.json(catalogue));
    vi.stubGlobal("fetch", fetch);
    await expect(getApprovedPolicyCatalogue()).resolves.toEqual(catalogue);
    expect(fetch).toHaveBeenCalledWith("/api/policies/versions/approved", {
      method: "GET",
      cache: "no-store",
      credentials: "same-origin",
      signal: expect.any(AbortSignal),
    });
  });

  it("allows an in-flight refresh to be abandoned", async () => {
    const controller = new AbortController();
    const fetch = vi.fn().mockResolvedValue(Response.json(catalogue));
    vi.stubGlobal("fetch", fetch);
    await getApprovedPolicyCatalogue(controller.signal);
    const signal = fetch.mock.calls[0]?.[1].signal as AbortSignal;
    expect(signal.aborted).toBe(false);
    controller.abort();
    expect(signal.aborted).toBe(true);
  });

  it("preserves session expiry from a non-JSON response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("expired", { status: 401 })));
    await expect(getApprovedPolicyCatalogue()).rejects.toMatchObject({
      code: "OPERATOR_UNAUTHORIZED",
      status: 401,
    });
  });

  it.each([
    () => new Response("private infrastructure diagnostic", { status: 503 }),
    () => Response.json({ error: "PRIVATE_OWNER_ERROR", path: "/private" }, { status: 500 }),
    () => new Response("not-json"),
    () => Response.json({ ...catalogue, schema: "private" }),
  ])("returns a generic unavailable result for bad server responses", async (response) => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(response()));
    await expect(getApprovedPolicyCatalogue()).rejects.toMatchObject({
      code: "PORTAL_UNAVAILABLE",
      message: "PORTAL_UNAVAILABLE",
      status: 503,
    });
  });

  it("returns unavailable after a network failure", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("private endpoint diagnostic")));
    await expect(getApprovedPolicyCatalogue()).rejects.toMatchObject({
      code: "PORTAL_UNAVAILABLE",
      status: 503,
    });
  });
});
