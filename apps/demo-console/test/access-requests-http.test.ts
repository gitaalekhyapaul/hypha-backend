import { create } from "@bufbuild/protobuf";
import {
  AccessPolicyBindingSchema,
  AccessRequestSchema,
  AccessRequestState,
  type ApproveAccessRequestRequest,
  ApproveAccessRequestResponseSchema,
  CancelAccessRequestResponseSchema,
  type CreateAccessRequestRequest,
  CreateAccessRequestResponseSchema,
  GetAccessRequestResponseSchema,
  ListAccessRequestsResponseSchema,
  RejectAccessRequestResponseSchema,
} from "@raziel/proto-ts";
import { describe, expect, it, vi } from "vitest";
import { createAccessHandlers } from "../lib/access-requests/http.ts";
import { AccessRequestError } from "../lib/access-requests/types.ts";
import { parseIntent } from "../lib/access-requests/validation.ts";
import { AgentAuthError, type AgentRegistration } from "../lib/agent-auth/types.ts";
import type { SyntheticDataset } from "../lib/dataset-client/types.ts";
import { OperatorAuthError } from "../lib/operator-auth/types.ts";
import { RAW_TARGET_HEADER } from "../server/agent-transport.ts";

const OWNER = "a".repeat(32);
const ID = "11111111-1111-4111-8111-111111111111";
const ROOT = "/api/agent/v1/access-requests";
const intent = {
  formatVersion: 1,
  requestId: ID,
  taskId: "reconcile",
  purpose: "payment_reconciliation",
  action: "query",
  datasetId: "processor",
  datasetRevision: "2",
  requestContext: [{ name: "threshold", intValue: "3" }],
};
const agent: AgentRegistration = {
  formatVersion: 1,
  agentId: OWNER,
  displayName: "Processor",
  publicKey: Buffer.alloc(32, 7).toString("base64"),
  keyThumbprint: Buffer.alloc(32, 9).toString("base64"),
  createdAtUnixSeconds: "1800000000",
};
const dataset: SyntheticDataset = {
  formatVersion: 1,
  datasetId: "processor",
  revision: 2,
  source: { classification: "SYNTHETIC", sourceId: "test", sourceLabel: "Synthetic test" },
  status: "FINALIZED",
  rows: [],
  createdAt: "2026-09-25T00:00:00Z",
  updatedAt: "2026-09-25T00:00:00Z",
  finalization: {
    checksum: "b".repeat(64),
    schemaFingerprint: "c".repeat(64),
    rowCount: 0,
    finalizedAt: "2026-09-25T00:00:00Z",
  },
};

function fixture() {
  const row = create(AccessRequestSchema, {
    formatVersion: 1,
    requestId: ID,
    agentId: OWNER,
    taskId: intent.taskId,
    purpose: intent.purpose,
    action: "query",
    datasetId: "processor",
    datasetRevision: 2n,
    state: AccessRequestState.PENDING_REVIEW,
    generation: 1n,
    createdAtUnixSeconds: 1800000000n,
    updatedAtUnixSeconds: 1800000000n,
    deadlineUnixSeconds: 1800086400n,
  });
  const owner = {
    approve: vi.fn(async (input: ApproveAccessRequestRequest) => {
      if (!input.dataset) throw new AccessRequestError("NOT_FOUND");
      return create(ApproveAccessRequestResponseSchema, {
        request: {
          ...row,
          state: AccessRequestState.APPROVED_WAITING,
          generation: 2n,
          policyBinding: create(AccessPolicyBindingSchema, {
            policyRecordId: input.policyRecordId,
            policyHash: Buffer.alloc(32, 8),
            resourceType: `dataset_${input.policyVersionId}`,
            resourceScope: row.datasetId,
          }),
        },
      });
    }),
    create: vi.fn(async (input: CreateAccessRequestRequest) => {
      if (!input.dataset) throw new AccessRequestError("NOT_FOUND");
      return create(CreateAccessRequestResponseSchema, {
        formatVersion: 1,
        request: row,
        created: true,
      });
    }),
    get: vi.fn(async () => create(GetAccessRequestResponseSchema, { request: row })),
    list: vi.fn(async () =>
      create(ListAccessRequestsResponseSchema, { formatVersion: 1, requests: [row] }),
    ),
    reject: vi.fn(async () =>
      create(RejectAccessRequestResponseSchema, {
        request: { ...row, state: AccessRequestState.REJECTED, generation: 2n },
      }),
    ),
    cancel: vi.fn(async () =>
      create(CancelAccessRequestResponseSchema, {
        request: { ...row, state: AccessRequestState.CANCELED, generation: 2n },
      }),
    ),
  };
  const deps = {
    authenticate: vi.fn(async () => agent),
    authorizeOperator: vi.fn(async () => {}),
    dataset: vi.fn(async () => dataset),
    owner: () => owner,
  };
  return { row, owner, deps, handlers: createAccessHandlers(deps) };
}

function request(path = ROOT, body?: unknown) {
  return new Request(`http://localhost${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: {
      [RAW_TARGET_HEADER]: path,
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
}

const approval = {
  formatVersion: 1,
  expectedGeneration: "1",
  policyRecordId: "policy-selected",
  policyVersionId: "d".repeat(32),
  approvalEpoch: "1",
  grant: {
    formatVersion: 1,
    teamId: "finance",
    organizationId: "lab",
    approvedAmount: "50000",
    isApproved: true,
    isSuspended: false,
    isOverride: false,
  },
};

describe("operator access approval", () => {
  const path = `/api/operator/access-requests/${OWNER}/${ID}/approve`;

  it("pins server-read finalized metadata and returns no private grant inputs", async () => {
    const f = fixture();
    const response = await f.handlers.operatorApprove(request(path, approval), OWNER, ID);
    expect(response.status).toBe(200);
    expect(f.owner.approve).toHaveBeenCalledTimes(2);
    expect(f.owner.approve.mock.calls[0]?.[0].dataset).toBeUndefined();
    expect(f.owner.approve.mock.calls[1]?.[0]).toMatchObject({
      agentId: OWNER,
      requestId: ID,
      expectedGeneration: 1n,
      dataset: {
        datasetId: "processor",
        revision: 2n,
        checksum: Buffer.from("b".repeat(64), "hex"),
      },
      grant: { approvedAmount: 50000n, isApproved: true },
    });
    const payload = await response.json();
    expect(payload.state).toBe("ACCESS_REQUEST_STATE_APPROVED_WAITING");
    expect(payload.policyBinding.policyRecordId).toBe("policy-selected");
    expect(payload).not.toHaveProperty("grant");
    expect(JSON.stringify(payload)).not.toContain("approvedAmount");
  });

  it("recovers exact approval retries without a dataset read", async () => {
    const f = fixture();
    f.owner.approve.mockResolvedValueOnce(
      create(ApproveAccessRequestResponseSchema, {
        request: { ...f.row, state: AccessRequestState.APPROVED_WAITING, generation: 2n },
      }),
    );
    f.deps.dataset.mockRejectedValue(new Error("offline"));
    expect((await f.handlers.operatorApprove(request(path, approval), OWNER, ID)).status).toBe(200);
    expect(f.deps.dataset).not.toHaveBeenCalled();
    expect(f.owner.get).not.toHaveBeenCalled();
  });

  it("checks operator admission first and rejects caller dataset or policy body overrides", async () => {
    const f = fixture();
    f.deps.authorizeOperator.mockRejectedValueOnce(new OperatorAuthError("OPERATOR_UNAUTHORIZED"));
    expect((await f.handlers.operatorApprove(request(path, approval), OWNER, ID)).status).toBe(401);
    for (const patch of [
      { dataset: { checksum: "fake" } },
      { resourceScope: "other" },
      { policyHash: "fake" },
      { approvalEpoch: "0" },
      { expectedGeneration: "01" },
      { grant: { ...approval.grant, approvedAmount: 50000 } },
      { grant: { ...approval.grant, isApproved: "true" } },
      { grant: { ...approval.grant, approvedAmount: "9007199254740992" } },
    ]) {
      expect(
        (await f.handlers.operatorApprove(request(path, { ...approval, ...patch }), OWNER, ID))
          .status,
      ).toBe(400);
    }
    expect(f.owner.approve).not.toHaveBeenCalled();
    expect(f.deps.dataset).not.toHaveBeenCalled();
  });

  it("does not approve a stale generation, changed dataset, or terminal request", async () => {
    for (const patch of [{ generation: 2n }, { state: AccessRequestState.CANCELED }]) {
      const f = fixture();
      f.owner.get.mockResolvedValueOnce(
        create(GetAccessRequestResponseSchema, { request: { ...f.row, ...patch } }),
      );
      expect((await f.handlers.operatorApprove(request(path, approval), OWNER, ID)).status).toBe(
        409,
      );
      expect(f.owner.approve).toHaveBeenCalledTimes(1);
      expect(f.deps.dataset).not.toHaveBeenCalled();
    }
    const f = fixture();
    f.deps.dataset.mockResolvedValueOnce({ ...dataset, revision: 3 });
    expect((await f.handlers.operatorApprove(request(path, approval), OWNER, ID)).status).toBe(409);
    expect(f.owner.approve).toHaveBeenCalledTimes(1);
  });
});

describe("external access-request facade", () => {
  it("authenticates exact bytes and submits registered identity plus trusted finalized revision", async () => {
    const f = fixture();
    const response = await f.handlers.agentCreate(request(ROOT, intent));
    expect(response.status).toBe(201);
    expect(f.deps.authenticate).toHaveBeenCalledWith(
      expect.any(Request),
      Buffer.from(JSON.stringify(intent)),
    );
    expect(f.owner.create).toHaveBeenCalledWith(
      expect.objectContaining({
        owner: expect.objectContaining({ agentId: OWNER, agentPublicKey: Buffer.alloc(32, 7) }),
        dataset: expect.objectContaining({
          datasetId: "processor",
          revision: 2n,
          checksum: Buffer.from("b".repeat(64), "hex"),
        }),
      }),
    );
    const payload = await response.json();
    expect(payload.state).toBe("ACCESS_REQUEST_STATE_PENDING_REVIEW");
    expect(payload).not.toHaveProperty("requestContext");
    expect(JSON.stringify(payload)).not.toContain("threshold");
    f.owner.create.mockResolvedValueOnce(
      create(CreateAccessRequestResponseSchema, { formatVersion: 1, request: f.row }),
    );
    expect((await f.handlers.agentCreate(request(ROOT, intent))).status).toBe(200);
  });

  it("rejects identity overrides and never reaches private storage after failed authentication", async () => {
    const f = fixture();
    expect(
      (await f.handlers.agentCreate(request(ROOT, { ...intent, agentId: OWNER }))).status,
    ).toBe(400);
    f.deps.authenticate.mockRejectedValueOnce(new AgentAuthError("UNAUTHENTICATED"));
    expect((await f.handlers.agentCreate(request(ROOT, intent))).status).toBe(401);
    expect(f.owner.create).not.toHaveBeenCalled();
    expect(f.deps.dataset).not.toHaveBeenCalled();
  });

  it("lists and reads only the authenticated owner and rejects unexpected owner results", async () => {
    const f = fixture();
    expect((await f.handlers.agentList(request(`${ROOT}?limit=2`))).status).toBe(200);
    expect(f.owner.list).toHaveBeenCalledWith(
      expect.objectContaining({ agentId: OWNER, limit: 2 }),
    );
    expect((await f.handlers.agentList(request(`${ROOT}?agentId=${"b".repeat(32)}`))).status).toBe(
      400,
    );
    expect((await f.handlers.agentGet(request(`${ROOT}/${ID}`), ID)).status).toBe(200);
    f.owner.get.mockResolvedValueOnce(
      create(GetAccessRequestResponseSchema, { request: { ...f.row, agentId: "b".repeat(32) } }),
    );
    expect((await f.handlers.agentGet(request(`${ROOT}/${ID}`), ID)).status).toBe(503);
  });

  it("does not accept draft or changed dataset revisions", async () => {
    const f = fixture();
    f.deps.dataset.mockResolvedValueOnce({ ...dataset, revision: 3 });
    expect((await f.handlers.agentCreate(request(ROOT, intent))).status).toBe(404);
    const { finalization: _finalization, ...draft } = dataset;
    f.deps.dataset.mockResolvedValueOnce({ ...draft, status: "DRAFT" });
    expect((await f.handlers.agentCreate(request(ROOT, intent))).status).toBe(404);
    expect(f.owner.create.mock.calls.every(([input]) => input.dataset === undefined)).toBe(true);
  });

  it("uses operator session and CSRF admission before review mutations", async () => {
    const f = fixture();
    const path = `/api/operator/access-requests/${OWNER}/${ID}/reject`;
    const mutation = { formatVersion: 1, expectedGeneration: "1" };
    const response = await f.handlers.operatorMutation(
      request(path, mutation),
      OWNER,
      ID,
      "reject",
    );
    expect(response.status).toBe(200);
    expect(f.owner.reject).toHaveBeenCalledWith(
      expect.objectContaining({ agentId: OWNER, requestId: ID, expectedGeneration: 1n }),
    );
    f.deps.authorizeOperator.mockRejectedValueOnce(new OperatorAuthError("OPERATOR_UNAUTHORIZED"));
    expect(
      (await f.handlers.operatorMutation(request(path, mutation), OWNER, ID, "reject")).status,
    ).toBe(401);
    expect(f.owner.reject).toHaveBeenCalledTimes(1);
  });

  it("passes generation and owner to cancellation and keeps capacity failures coarse", async () => {
    const f = fixture();
    expect(
      (
        await f.handlers.agentCancel(
          request(`${ROOT}/${ID}/cancel`, { formatVersion: 1, expectedGeneration: "1" }),
          ID,
        )
      ).status,
    ).toBe(200);
    expect(f.owner.cancel).toHaveBeenCalledWith(
      expect.objectContaining({ agentId: OWNER, expectedGeneration: 1n }),
    );
    f.owner.create.mockRejectedValueOnce(new AccessRequestError("CAPACITY_EXCEEDED"));
    const response = await f.handlers.agentCreate(request(ROOT, intent));
    expect(response.status).toBe(429);
    expect(response.headers.get("retry-after")).toBe("30");
    expect(await response.json()).toEqual({
      error: { code: "CAPACITY_EXCEEDED", retryable: true },
    });
  });

  it("rejects normalized route aliases and oversized streaming bodies", async () => {
    const f = fixture();
    const alias = request(ROOT, intent);
    alias.headers.set(RAW_TARGET_HEADER, "/api/agent/v1/ignored/../access-requests");
    expect((await f.handlers.agentCreate(alias)).status).toBe(400);
    expect(
      (await f.handlers.agentCreate(request(ROOT, { ...intent, purpose: "x".repeat(16384) })))
        .status,
    ).toBe(400);
    expect(f.owner.create).not.toHaveBeenCalled();
  });

  it("enforces strict ProtoJSON types and one typed value per context entry", () => {
    for (const patch of [
      { formatVersion: "1" },
      { datasetRevision: 2 },
      { datasetRevision: "02" },
      { datasetRevision: "0" },
      { requestContext: [{ name: "n", intValue: "-0" }] },
      { requestContext: [{ name: "n", boolValue: false, intValue: "2" }] },
      { requestContext: [{ name: "n", intValue: "9223372036854775808" }] },
    ]) {
      expect(() => parseIntent(Buffer.from(JSON.stringify({ ...intent, ...patch })))).toThrow(
        AccessRequestError,
      );
    }
  });
});
