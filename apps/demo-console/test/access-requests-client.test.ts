import { create, toJson } from "@bufbuild/protobuf";
import {
  AccessRequestSchema,
  AccessRequestState,
  ListAccessRequestsResponseSchema,
} from "@raziel/proto-ts";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type AccessRequestApproval,
  approveAccessRequest,
  cancelAccessRequest,
  getAccessRequest,
  listAccessRequests,
  rejectAccessRequest,
} from "../lib/portal-client/access-requests.ts";

const AGENT = "12".repeat(16);
const OTHER_AGENT = "34".repeat(16);
const REQUEST_ID = "12345678-1234-4123-8123-123456789abc";
const REQUEST = {
  formatVersion: 1,
  requestId: REQUEST_ID,
  agentId: AGENT,
  taskId: "reconcile-september",
  purpose: "payment-reconciliation",
  action: "query",
  datasetId: "processor-ledger",
  datasetRevision: "1",
  state: "ACCESS_REQUEST_STATE_PENDING_REVIEW",
  generation: "1",
  createdAtUnixSeconds: "1780000000",
  updatedAtUnixSeconds: "1780000000",
  deadlineUnixSeconds: "1780086400",
};
const PATH = `/api/operator/access-requests/${AGENT}/${REQUEST_ID}`;
const APPROVAL: AccessRequestApproval = {
  formatVersion: 1,
  expectedGeneration: "1",
  policyRecordId: "policy-record-1",
  policyVersionId: "ab".repeat(16),
  approvalEpoch: "2",
  grant: {
    formatVersion: 1,
    teamId: "finance-operations",
    organizationId: "nus-fintech-lab",
    approvedAmount: "50000",
    isApproved: true,
    isSuspended: false,
    isOverride: false,
  },
};
const APPROVED = {
  ...REQUEST,
  generation: "2",
  state: "ACCESS_REQUEST_STATE_APPROVED_WAITING",
  policyBinding: {
    policyRecordId: APPROVAL.policyRecordId,
    policyHash: Buffer.alloc(32, 1).toString("base64"),
    resourceType: `dataset_${APPROVAL.policyVersionId}`,
    resourceScope: "processor-ledger",
  },
};

function response(body: unknown, status = 200) {
  const fetch = vi.fn().mockResolvedValue(Response.json(body, { status }));
  vi.stubGlobal("fetch", fetch);
  return fetch;
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("operator access-request reads", () => {
  it("decodes generated safe DTOs and defaults to 25 requests", async () => {
    const fetch = response({ formatVersion: 1, requests: [REQUEST] });
    const result = await listAccessRequests();
    expect(result.requests[0]).toMatchObject({
      $typeName: "raziel.v1.AccessRequest",
      agentId: AGENT,
      datasetRevision: 1n,
      generation: 1n,
      state: AccessRequestState.PENDING_REVIEW,
      capability: new Uint8Array(),
    });
    expect(fetch).toHaveBeenCalledWith(
      "/api/operator/access-requests?limit=25",
      expect.objectContaining({
        credentials: "same-origin",
        cache: "no-store",
        redirect: "error",
      }),
    );
    expect(fetch.mock.calls[0]?.[1]).not.toHaveProperty("body");
    expect(fetch.mock.calls[0]?.[1]).not.toHaveProperty("headers");
  });

  it("accepts server-generated implicit defaults and an empty list", async () => {
    response(
      toJson(
        ListAccessRequestsResponseSchema,
        create(ListAccessRequestsResponseSchema, { formatVersion: 1 }),
        {
          alwaysEmitImplicit: true,
        },
      ),
    );
    expect((await listAccessRequests()).requests).toEqual([]);
    const request = create(AccessRequestSchema, {
      formatVersion: 1,
      requestId: REQUEST_ID,
      agentId: AGENT,
      taskId: "task",
      purpose: "reconcile",
      action: "query",
      datasetId: "ledger",
      datasetRevision: 1n,
      generation: 1n,
      state: AccessRequestState.PENDING_REVIEW,
      createdAtUnixSeconds: 1780000000n,
      updatedAtUnixSeconds: 1780000000n,
      deadlineUnixSeconds: 1780086400n,
    });
    response(toJson(AccessRequestSchema, request, { alwaysEmitImplicit: true }));
    expect(await getAccessRequest(AGENT, REQUEST_ID)).toEqual(request);
  });

  it("encodes opaque cursors and supports an exact agent filter", async () => {
    const fetch = response({ formatVersion: 1, requests: [REQUEST], nextCursor: "next-page" });
    const result = await listAccessRequests({
      limit: 50,
      cursor: "opaque+/=&? value",
      agentId: AGENT,
    });
    expect(fetch.mock.calls[0]?.[0]).toBe(
      `/api/operator/access-requests?limit=50&cursor=opaque%2B%2F%3D%26%3F+value&agentId=${AGENT}`,
    );
    expect(result.nextCursor).toBe("next-page");
  });

  it("reads direct request responses under both owner and request IDs", async () => {
    const fetch = response(REQUEST);
    await getAccessRequest(AGENT, REQUEST_ID);
    expect(fetch.mock.calls[0]?.[0]).toBe(PATH);
  });

  it("keeps identical request UUIDs from different agents separate", async () => {
    response({ formatVersion: 1, requests: [REQUEST, { ...REQUEST, agentId: OTHER_AGENT }] });
    expect((await listAccessRequests()).requests.map((item) => item.agentId)).toEqual([
      AGENT,
      OTHER_AGENT,
    ]);
  });

  it.each([{ agentId: OTHER_AGENT }, { requestId: "87654321-1234-4123-8123-123456789abc" }])(
    "rejects detail identity mismatches %j",
    async (change) => {
      response({ ...REQUEST, ...change });
      await expect(getAccessRequest(AGENT, REQUEST_ID)).rejects.toMatchObject({ status: 503 });
    },
  );

  it.each([
    { limit: 0 },
    { limit: 51 },
    { limit: 1.5 },
    { limit: Number.NaN },
    { cursor: "" },
    { cursor: "a".repeat(4097) },
    { cursor: "\n" },
    { agentId: "../agent" },
    { agentId: "AA".repeat(16) },
  ])("rejects malformed list options before sending %j", async (options) => {
    const fetch = response({ formatVersion: 1, requests: [] });
    await expect(listAccessRequests(options)).rejects.toMatchObject({
      code: "INVALID_REQUEST",
      status: 400,
    });
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each([
    ["../operator", REQUEST_ID],
    [AGENT, "../reject"],
    [AGENT, "not-a-uuid"],
  ])("rejects invalid path identities %s %s", async (agentId, requestId) => {
    const fetch = response(REQUEST);
    await expect(getAccessRequest(agentId, requestId)).rejects.toMatchObject({ status: 400 });
    expect(fetch).not.toHaveBeenCalled();
  });

  it("forwards abort signals through the bounded request", async () => {
    const fetch = response(REQUEST);
    const controller = new AbortController();
    await getAccessRequest(AGENT, REQUEST_ID, controller.signal);
    const signal = fetch.mock.calls[0]?.[1]?.signal as AbortSignal;
    expect(signal.aborted).toBe(false);
    controller.abort();
    expect(signal.aborted).toBe(true);
  });
});

describe("strict generated ProtoJSON validation", () => {
  it.each([
    { formatVersion: 2 },
    { formatVersion: "1" },
    { state: "PENDING_REVIEW" },
    { state: "ACCESS_REQUEST_STATE_UNKNOWN" },
    { state: "ACCESS_REQUEST_STATE_UNSPECIFIED" },
    { state: 1 },
    { state: 99 },
    { proofJobState: 99 },
    { generation: 1 },
    { generation: "0" },
    { generation: "01" },
    { generation: "18446744073709551616" },
    { datasetRevision: 1 },
    { datasetRevision: "0" },
    { createdAtUnixSeconds: 1780000000 },
    { createdAtUnixSeconds: "-1" },
    { updatedAtUnixSeconds: "1779999999" },
    { deadlineUnixSeconds: "253402300800" },
    { deadlineUnixSeconds: "1780000000" },
    { purpose: "" },
    { taskId: null },
    { requestContext: [{ name: "private", stringValue: "secret" }] },
    { policyIr: "private" },
    { grantRecipe: "private" },
    { policyBinding: { policyRecordId: "id", privateDiagnostic: "secret" } },
    { proposal: { formatVersion: 2 } },
    { metrics: { executionCycleCount: 42 } },
    { capability: "AQ" },
  ])("fails closed without echoing malformed or private fields %j", async (change) => {
    response({ ...REQUEST, ...change });
    await expect(getAccessRequest(AGENT, REQUEST_ID)).rejects.toMatchObject({
      status: 503,
      code: "PORTAL_UNAVAILABLE",
      message: "PORTAL_UNAVAILABLE",
    });
  });

  it("rejects snake-case fields accepted by the permissive protobuf reader", async () => {
    const { taskId, ...rest } = REQUEST;
    response({ ...rest, task_id: taskId });
    await expect(getAccessRequest(AGENT, REQUEST_ID)).rejects.toMatchObject({ status: 503 });
  });

  it("retains uint64 precision above the safe JavaScript number range", async () => {
    response({
      ...REQUEST,
      generation: "9007199254740993",
      datasetRevision: "18446744073709551615",
    });
    expect(await getAccessRequest(AGENT, REQUEST_ID)).toMatchObject({
      generation: 9007199254740993n,
      datasetRevision: 18446744073709551615n,
    });
  });

  it.each([
    { formatVersion: 2, requests: [] },
    { formatVersion: 1 },
    { formatVersion: 1, requests: null },
    { formatVersion: 1, requests: [REQUEST, REQUEST] },
    { formatVersion: 1, requests: [{ ...REQUEST, capability: "AQ==" }] },
    { formatVersion: 1, requests: [], nextCursor: "" },
    { formatVersion: 1, requests: [], internalErrorRef: "private" },
  ])("rejects malformed or unsafe lists %j", async (body) => {
    response(body);
    await expect(listAccessRequests()).rejects.toMatchObject({ status: 503 });
  });

  it("rejects excess page size, wrong filter ownership, and cursor loops", async () => {
    response({ formatVersion: 1, requests: [REQUEST, { ...REQUEST, agentId: OTHER_AGENT }] });
    await expect(listAccessRequests({ limit: 1 })).rejects.toMatchObject({ status: 503 });
    await expect(listAccessRequests({ agentId: AGENT })).rejects.toMatchObject({ status: 503 });
    response({ formatVersion: 1, requests: [], nextCursor: "same" });
    await expect(listAccessRequests({ cursor: "same" })).rejects.toMatchObject({ status: 503 });
  });
});

describe("generation-checked operator mutations", () => {
  it.each([
    ["reject", rejectAccessRequest, "ACCESS_REQUEST_STATE_REJECTED"],
    ["cancel", cancelAccessRequest, "ACCESS_REQUEST_STATE_CANCELLING"],
  ] as const)(
    "posts %s with CSRF and generated decimal-string fields",
    async (action, mutate, state) => {
      const fetch = response({ ...REQUEST, state, generation: "2" });
      expect((await mutate(AGENT, REQUEST_ID, 1n, "test-csrf")).generation).toBe(2n);
      expect(fetch).toHaveBeenCalledWith(
        `${PATH}/${action}`,
        expect.objectContaining({
          method: "POST",
          credentials: "same-origin",
          cache: "no-store",
          headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "test-csrf" },
          body: '{"formatVersion":1,"expectedGeneration":"1"}',
        }),
      );
    },
  );

  it("preserves a READY response from a cancellation race", async () => {
    response({
      ...REQUEST,
      state: "ACCESS_REQUEST_STATE_READY",
      generation: "3",
      capability: "AQ==",
    });
    const result = await cancelAccessRequest(AGENT, REQUEST_ID, 2n, "csrf");
    expect(result.state).toBe(AccessRequestState.READY);
    expect(result.capability).toEqual(new Uint8Array([1]));
  });

  it.each([0n, -1n, 18446744073709551616n])(
    "rejects invalid generation %s without sending",
    async (generation) => {
      const fetch = response(REQUEST);
      await expect(
        rejectAccessRequest(AGENT, REQUEST_ID, generation, "csrf"),
      ).rejects.toMatchObject({ status: 400 });
      expect(fetch).not.toHaveBeenCalled();
    },
  );

  it("rejects missing CSRF without sending", async () => {
    const fetch = response(REQUEST);
    await expect(cancelAccessRequest(AGENT, REQUEST_ID, 1n, "")).rejects.toMatchObject({
      status: 400,
    });
    expect(fetch).not.toHaveBeenCalled();
  });

  it("rejects a generation rollback in a mutation response", async () => {
    response(REQUEST);
    await expect(rejectAccessRequest(AGENT, REQUEST_ID, 2n, "csrf")).rejects.toMatchObject({
      status: 503,
    });
  });

  it("exposes a conflict for UI refetch without retrying the mutation", async () => {
    const fetch = response({ error: { code: "CONFLICT", retryable: false } }, 409);
    await expect(rejectAccessRequest(AGENT, REQUEST_ID, 1n, "csrf")).rejects.toMatchObject({
      code: "CONFLICT",
      status: 409,
      retryable: false,
    });
    expect(fetch).toHaveBeenCalledTimes(1);
  });
});

describe("safe failure handling", () => {
  it.each([
    { error: "OPERATOR_CSRF_REJECTED" },
    { error: { code: "OPERATOR_CSRF_REJECTED", retryable: false } },
  ])("supports flat operator and workflow failures %j", async (body) => {
    response(body, 403);
    await expect(rejectAccessRequest(AGENT, REQUEST_ID, 1n, "csrf")).rejects.toMatchObject({
      status: 403,
      code: "OPERATOR_CSRF_REJECTED",
      retryable: false,
    });
  });

  it("retains retryability without retaining private messages", async () => {
    response(
      { error: { code: "SERVICE_UNAVAILABLE", retryable: true, message: "private witness" } },
      503,
    );
    await expect(listAccessRequests()).rejects.toMatchObject({
      status: 503,
      retryable: true,
      message: "SERVICE_UNAVAILABLE",
    });
  });

  it("does not echo unknown error codes", async () => {
    response({ error: "private relationship value" }, 500);
    await expect(listAccessRequests()).rejects.toMatchObject({ message: "PORTAL_UNAVAILABLE" });
  });

  it("preserves session expiry for a non-JSON 401", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("Expired", { status: 401 })));
    await expect(listAccessRequests()).rejects.toMatchObject({
      status: 401,
      code: "OPERATOR_UNAUTHORIZED",
    });
  });

  it("maps invalid JSON and network failure to safe errors", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("not JSON")));
    await expect(listAccessRequests()).rejects.toMatchObject({ status: 503 });
    const fetch = vi.fn().mockRejectedValue(new Error("private network details"));
    vi.stubGlobal("fetch", fetch);
    await expect(cancelAccessRequest(AGENT, REQUEST_ID, 1n, "csrf")).rejects.toMatchObject({
      status: 503,
      message: "PORTAL_UNAVAILABLE",
    });
    expect(fetch).toHaveBeenCalledTimes(1);
  });
});

describe("operator approval", () => {
  it("sends only the strict approval body with CSRF and decodes the pinned binding", async () => {
    const fetch = response(APPROVED);
    const result = await approveAccessRequest(AGENT, REQUEST_ID, APPROVAL, "test-csrf");
    expect(result).toMatchObject({
      state: AccessRequestState.APPROVED_WAITING,
      generation: 2n,
      policyBinding: {
        policyRecordId: APPROVAL.policyRecordId,
        policyHash: new Uint8Array(32).fill(1),
        resourceType: APPROVED.policyBinding.resourceType,
        resourceScope: "processor-ledger",
      },
    });
    expect(fetch).toHaveBeenCalledExactlyOnceWith(
      `${PATH}/approve`,
      expect.objectContaining({
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
        redirect: "error",
        headers: { "Content-Type": "application/json", "x-raziel-csrf-token": "test-csrf" },
        body: JSON.stringify(APPROVAL),
      }),
    );
  });

  it.each([
    { formatVersion: 2 },
    { expectedGeneration: "0" },
    { expectedGeneration: "01" },
    { expectedGeneration: "18446744073709551616" },
    { expectedGeneration: 1 },
    { policyRecordId: "" },
    { policyRecordId: "x".repeat(257) },
    { policyVersionId: "AB".repeat(16) },
    { policyVersionId: "../version" },
    { approvalEpoch: "0" },
    { approvalEpoch: "01" },
    { approvalEpoch: "18446744073709551616" },
    { approvalEpoch: 2 },
    { dataset: { checksum: "caller-selected" } },
    { datasetId: "caller-selected" },
    { resourceType: "dataset" },
    { resourceScope: "caller-selected" },
    { policyHash: "caller-selected" },
    { agentPrivateKey: "private" },
    { grant: null },
  ])("rejects malformed or extra approval fields %j without sending", async (change) => {
    const fetch = response(APPROVED);
    await expect(
      approveAccessRequest(
        AGENT,
        REQUEST_ID,
        { ...APPROVAL, ...change } as AccessRequestApproval,
        "csrf",
      ),
    ).rejects.toMatchObject({ status: 400 });
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each([
    { formatVersion: 2 },
    { teamId: "../team" },
    { organizationId: "" },
    { organizationId: "x".repeat(129) },
    { approvedAmount: "0" },
    { approvedAmount: "01" },
    { approvedAmount: "1e3" },
    { approvedAmount: "1.5" },
    { approvedAmount: "9007199254740992" },
    { approvedAmount: 50000 },
    { isApproved: "true" },
    { isSuspended: 0 },
    { isOverride: undefined },
    { relationship: "caller-selected" },
  ])("rejects malformed grant fields %j without sending", async (change) => {
    const fetch = response(APPROVED);
    await expect(
      approveAccessRequest(
        AGENT,
        REQUEST_ID,
        { ...APPROVAL, grant: { ...APPROVAL.grant, ...change } } as AccessRequestApproval,
        "csrf",
      ),
    ).rejects.toMatchObject({ status: 400 });
    expect(fetch).not.toHaveBeenCalled();
  });

  it("retains maximum safe amount and uint64 approval precision as decimal strings", async () => {
    const fetch = response(APPROVED);
    const body = {
      ...APPROVAL,
      approvalEpoch: "18446744073709551615",
      grant: { ...APPROVAL.grant, approvedAmount: "9007199254740991" },
    };
    await approveAccessRequest(AGENT, REQUEST_ID, body, "csrf");
    expect(fetch.mock.calls[0]?.[1]?.body).toBe(JSON.stringify(body));
  });

  it("does not send a missing field or an object with inherited serialization", async () => {
    const fetch = response(APPROVED);
    const { approvalEpoch: _epoch, ...missing } = APPROVAL;
    const inherited = Object.assign(
      Object.create({ toJSON: () => ({ privateKey: "secret" }) }),
      APPROVAL,
    );
    for (const body of [missing, inherited, { ...APPROVAL, [Symbol("private")]: "secret" }]) {
      await expect(
        approveAccessRequest(AGENT, REQUEST_ID, body as AccessRequestApproval, "csrf"),
      ).rejects.toMatchObject({ status: 400 });
    }
    expect(fetch).not.toHaveBeenCalled();
  });

  it("rejects bad path identities and missing CSRF before sending", async () => {
    const fetch = response(APPROVED);
    for (const [agentId, requestId, csrf] of [
      ["../agent", REQUEST_ID, "csrf"],
      [AGENT, "../request", "csrf"],
      [AGENT, REQUEST_ID, ""],
    ]) {
      await expect(
        approveAccessRequest(agentId as string, requestId as string, APPROVAL, csrf as string),
      ).rejects.toMatchObject({ status: 400 });
    }
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each([
    { generation: "1" },
    { state: "ACCESS_REQUEST_STATE_PENDING_REVIEW" },
    { state: "ACCESS_REQUEST_STATE_QUEUED" },
    { policyBinding: undefined },
    { policyBinding: { ...APPROVED.policyBinding, policyRecordId: "different-record" } },
    { policyBinding: { ...APPROVED.policyBinding, policyHash: "AQ==" } },
    { policyBinding: { ...APPROVED.policyBinding, resourceType: "dataset" } },
    { policyBinding: { ...APPROVED.policyBinding, resourceScope: "../ledger" } },
    { policyBinding: { ...APPROVED.policyBinding, grant: "private" } },
  ])("rejects inconsistent approval responses %j", async (change) => {
    response({ ...APPROVED, ...change });
    await expect(approveAccessRequest(AGENT, REQUEST_ID, APPROVAL, "csrf")).rejects.toMatchObject({
      status: 503,
    });
  });

  it.each([401, 403, 409, 410, 503])(
    "preserves approval status %s without retrying",
    async (status) => {
      const fetch = response(
        { error: { code: "CONFLICT", retryable: false, message: "private reason" } },
        status,
      );
      await expect(approveAccessRequest(AGENT, REQUEST_ID, APPROVAL, "csrf")).rejects.toMatchObject(
        { status, message: status === 401 ? "OPERATOR_UNAUTHORIZED" : "CONFLICT" },
      );
      expect(fetch).toHaveBeenCalledTimes(1);
    },
  );

  it("allows an explicit byte-identical retry after an uncertain network failure", async () => {
    const fetch = vi
      .fn()
      .mockRejectedValueOnce(new Error("private network detail"))
      .mockResolvedValueOnce(Response.json(APPROVED));
    vi.stubGlobal("fetch", fetch);
    await expect(approveAccessRequest(AGENT, REQUEST_ID, APPROVAL, "csrf")).rejects.toMatchObject({
      status: 503,
      retryable: true,
      message: "PORTAL_UNAVAILABLE",
    });
    expect(fetch).toHaveBeenCalledTimes(1);
    await approveAccessRequest(AGENT, REQUEST_ID, APPROVAL, "csrf");
    expect(fetch.mock.calls[1]?.[1]?.body).toBe(fetch.mock.calls[0]?.[1]?.body);
    expect(APPROVAL.grant.approvedAmount).toBe("50000");
  });
});
