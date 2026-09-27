import { create, toBinary } from "@bufbuild/protobuf";
import { ProofCapabilitySchema } from "@raziel/proto-ts";
import { describe, expect, it, vi } from "vitest";
import { AgentAuthError } from "../lib/agent-auth/types.ts";
import { createAgentQueryHandler } from "../lib/agent-query/http.ts";
import { RAW_TARGET_HEADER } from "../server/agent-transport.ts";

const DATASET = "processor";
const TARGET = `/api/datasets/${DATASET}/protected-query`;
const KEY = Buffer.alloc(32, 7);
const ID = "a".repeat(64);
const RESOURCE = `dataset_${"b".repeat(32)}`;
const agent = {
  formatVersion: 1 as const,
  agentId: "c".repeat(32),
  displayName: "Processor",
  publicKey: KEY.toString("base64"),
  keyThumbprint: Buffer.alloc(32, 9).toString("base64"),
  createdAtUnixSeconds: "1800000000",
};

function envelope() {
  return {
    format_version: 1,
    capability_proto_hex: Buffer.from(
      toBinary(
        ProofCapabilitySchema,
        create(ProofCapabilitySchema, {
          capabilityId: ID,
          resourceType: RESOURCE,
          resourceScope: DATASET,
        }),
      ),
    ).toString("hex"),
    agent_public_key: KEY.toString("hex"),
    request_method: "POST",
    canonical_request_target: TARGET,
    request_body_base64: "e30=",
    request_body_hash: "d".repeat(64),
    request_nonce: "e".repeat(64),
    request_timestamp_unix_seconds: "1800000000",
    agent_request_signature: "f".repeat(128),
  };
}

function success(path = "VERIFIER") {
  return {
    classification: "SYNTHETIC",
    dataset_id: DATASET,
    verification_path: path,
    row_count: 0,
    rows: [],
    internal_trace: "not-public",
    receipt: {
      format_version: 1,
      receipt_id: "receipt-1",
      request_id: "request-1",
      receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
      task_id: "reconcile",
      capability_id: ID,
      proof_job_id: ID,
      policy_record_id: "policy-1",
      policy_hash: "1".repeat(64),
      snapshot_id: "snapshot-1",
      relationship_snapshot_root: "2".repeat(64),
      public_claims_digest: "3".repeat(64),
      proof_digest: "4".repeat(64),
      occurred_at: "2026-09-27T00:00:00Z",
      action: "query",
      resource_type: RESOURCE,
      resource_scope: DATASET,
      outcome: "AUDIT_OUTCOME_ACCEPTED",
      reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
      private_policy: "not-public",
    },
  };
}

function request(body = JSON.stringify(envelope()), target = TARGET) {
  return new Request(`http://localhost${TARGET}`, {
    method: "POST",
    body,
    headers: {
      "content-type": "application/json",
      [RAW_TARGET_HEADER]: target,
      authorization: "Bearer caller-token",
      "x-raziel-agent-signature": "caller-signature",
    },
  });
}

function fixture() {
  const authenticate = vi.fn(async () => agent);
  const forward = vi.fn<typeof fetch>(async () => Response.json(success()));
  const handle = createAgentQueryHandler({
    authenticate,
    fetch: forward,
    gatewayUrl: () => "http://gateway:3000",
  });
  return { authenticate, forward, handle };
}

describe("agent protected-query facade", () => {
  it("forwards the exact envelope only to the fixed protected route without caller credentials", async () => {
    const f = fixture();
    const body = JSON.stringify(envelope(), null, 2);
    const response = await f.handle(request(body), DATASET);
    expect(response.status).toBe(200);
    expect(f.authenticate).toHaveBeenCalledWith(expect.any(Request), Buffer.from(body));
    const [url, init] = f.forward.mock.calls[0] ?? [];
    expect(String(url)).toBe(`http://gateway:3000${TARGET}`);
    expect(init?.body).toEqual(new Uint8Array(Buffer.from(body)));
    expect(init?.headers).toEqual({ "content-type": "application/json" });
    expect(init?.redirect).toBe("error");
    const result = await response.json();
    expect(result.receipt.resource_type).toBe(RESOURCE);
    expect(result.verification_path).toBe("VERIFIER");
    expect(JSON.stringify(result)).not.toContain("not-public");
  });

  it("authenticates again for cached queries and blocks revoked tokens before forwarding", async () => {
    const f = fixture();
    f.forward.mockResolvedValueOnce(Response.json(success("CACHE")));
    expect((await f.handle(request(), DATASET)).status).toBe(200);
    f.authenticate.mockRejectedValueOnce(new AgentAuthError("UNAUTHENTICATED"));
    const denied = await f.handle(request(), DATASET);
    expect(denied.status).toBe(401);
    expect(await denied.json()).toEqual({ error: { code: "NOT_AUTHORIZED", retryable: false } });
    expect(f.authenticate).toHaveBeenCalledTimes(2);
    expect(f.forward).toHaveBeenCalledTimes(1);
  });

  it("rejects another signing key, substituted target, scope, and unknown envelope fields", async () => {
    for (const patch of [
      { agent_public_key: "9".repeat(64) },
      { canonical_request_target: "/api/datasets" },
      { request_method: "GET" },
      { agent_id: "someone-else" },
      { capability_proto_hex: "broken" },
      {
        capability_proto_hex: Buffer.from(
          toBinary(
            ProofCapabilitySchema,
            create(ProofCapabilitySchema, {
              capabilityId: ID,
              resourceType: RESOURCE,
              resourceScope: "bank",
            }),
          ),
        ).toString("hex"),
      },
    ]) {
      const f = fixture();
      const response = await f.handle(
        request(JSON.stringify({ ...envelope(), ...patch })),
        DATASET,
      );
      expect(response.status).toBe(403);
      expect(f.forward).not.toHaveBeenCalled();
    }
  });

  it("rejects raw target aliases without normalizing them to an allowed route", async () => {
    const f = fixture();
    for (const target of [
      `${TARGET}?x=1`,
      "/api/datasets/ignored/../processor/protected-query",
      "/api/datasets/%70rocessor/protected-query",
    ]) {
      expect((await f.handle(request(undefined, target), DATASET)).status).toBe(403);
    }
    expect(f.authenticate).not.toHaveBeenCalled();
    expect(f.forward).not.toHaveBeenCalled();
  });

  it("bounds bodies and never forwards compressed envelopes", async () => {
    const f = fixture();
    expect((await f.handle(request("x".repeat(6 * 1024 * 1024 + 1)), DATASET)).status).toBe(403);
    const compressed = request();
    compressed.headers.set("content-encoding", "gzip");
    expect((await f.handle(compressed, DATASET)).status).toBe(403);
    expect(f.authenticate).not.toHaveBeenCalled();
    expect(f.forward).not.toHaveBeenCalled();
  });

  it("keeps gateway denials coarse and rejects mismatched public receipts or oversized responses", async () => {
    for (const [upstream, status, error] of [
      [Response.json({ error: "private-reason" }, { status: 403 }), 403, "AUTHORIZATION_REJECTED"],
      [Response.json({ error: "private-reason" }, { status: 500 }), 503, undefined],
      [Response.json({ ...success(), dataset_id: "bank" }), 503, "PROTECTED_QUERY_UNAVAILABLE"],
      [
        Response.json({
          ...success(),
          receipt: { ...success().receipt, resource_type: "dataset" },
        }),
        503,
        "PROTECTED_QUERY_UNAVAILABLE",
      ],
      [
        new Response("x".repeat(1024 * 1024 + 1), {
          headers: { "content-type": "application/json" },
        }),
        503,
        "PROTECTED_QUERY_UNAVAILABLE",
      ],
    ] as const) {
      const f = fixture();
      f.forward.mockResolvedValueOnce(upstream);
      const response = await f.handle(request(), DATASET);
      expect(response.status).toBe(status);
      const result = await response.json();
      if (error) expect(result.error).toBe(error);
      expect(JSON.stringify(result)).not.toContain("private-reason");
    }
  });
});
