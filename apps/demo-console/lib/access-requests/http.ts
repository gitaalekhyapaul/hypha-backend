import { create, toJson } from "@bufbuild/protobuf";
import {
  AccessDatasetRevisionSchema,
  type AccessRequest,
  AccessRequestSchema,
  AccessRequestState,
  ApproveAccessRequestRequestSchema,
  CancelAccessRequestRequestSchema,
  CreateAccessRequestRequestSchema,
  GetAccessRequestRequestSchema,
  ListAccessRequestsRequestSchema,
  RejectAccessRequestRequestSchema,
} from "@raziel/proto-ts";
import { RAW_TARGET_HEADER } from "../../server/agent-transport.ts";
import { publicAgentAuthResponse } from "../agent-auth/http.ts";
import { AgentAuthError, type AgentRegistration } from "../agent-auth/types.ts";
import { DatasetClientError, type SyntheticDataset } from "../dataset-client/types.ts";
import { jsonResponse, OperatorAuthError, operatorAuthResponse } from "../operator-auth/index.ts";
import { type AccessErrorCode, type AccessOwnerClient, AccessRequestError } from "./types.ts";
import {
  agentId,
  listOptions,
  parseApproval,
  parseIntent,
  parseMutation,
  readAccessBody,
  requestId,
} from "./validation.ts";

const ROOT = "/api/agent/v1/access-requests";
const STATUS: Record<AccessErrorCode, number> = {
  INVALID_REQUEST: 400,
  NOT_FOUND: 404,
  CONFLICT: 409,
  EXPIRED: 410,
  CAPACITY_EXCEEDED: 429,
  SERVICE_UNAVAILABLE: 503,
};

export interface AccessHttpDependencies {
  authenticate(request: Request, body: Uint8Array): Promise<AgentRegistration>;
  authorizeOperator(request: Request): Promise<void>;
  owner(): AccessOwnerClient;
  dataset(id: string): Promise<SyntheticDataset>;
}

export function createAccessHandlers(deps: AccessHttpDependencies) {
  const guarded = async (operation: () => Promise<Response>) => {
    try {
      return await operation();
    } catch (error) {
      return accessErrorResponse(error);
    }
  };
  return {
    agentCreate: (request: Request) =>
      guarded(async () => {
        target(request, ROOT);
        const bytes = await readAccessBody(request);
        const agent = await deps.authenticate(request, bytes);
        const intent = parseIntent(bytes);
        const client = deps.owner();
        const input = create(CreateAccessRequestRequestSchema, {
          owner: { agentId: agent.agentId, agentPublicKey: Buffer.from(agent.publicKey, "base64") },
          intent,
        });
        try {
          const retained = await client.create(input);
          if (retained.created) throw new AccessRequestError("SERVICE_UNAVAILABLE");
          return recordResponse(retained.request, agent.agentId, intent.requestId);
        } catch (error) {
          if (!(error instanceof AccessRequestError) || error.code !== "NOT_FOUND") throw error;
        }
        const dataset = await deps.dataset(intent.datasetId);
        if (
          dataset.status !== "FINALIZED" ||
          BigInt(dataset.revision) !== intent.datasetRevision ||
          !dataset.finalization
        )
          throw new AccessRequestError("NOT_FOUND");
        const result = await client.create(
          create(CreateAccessRequestRequestSchema, {
            owner: {
              agentId: agent.agentId,
              agentPublicKey: Buffer.from(agent.publicKey, "base64"),
            },
            intent,
            dataset: {
              datasetId: dataset.datasetId,
              revision: intent.datasetRevision,
              checksum: Buffer.from(dataset.finalization.checksum, "hex"),
            },
          }),
        );
        // Idempotent retries return the same public record, not an echoed private intent.
        return jsonResponse(
          toJson(AccessRequestSchema, checked(result.request, agent.agentId, intent.requestId)),
          result.created ? 201 : 200,
        );
      }),
    agentList: (request: Request) =>
      guarded(async () => {
        const path = target(request, ROOT, true);
        const agent = await deps.authenticate(request, new Uint8Array());
        const options = listOptions(path, false);
        const result = await deps
          .owner()
          .list(create(ListAccessRequestsRequestSchema, { ...options, agentId: agent.agentId }));
        return listResponse(result, options.limit, agent.agentId);
      }),
    agentGet: (request: Request, id: string) =>
      guarded(async () => {
        requestId(id);
        target(request, `${ROOT}/${id}`);
        const agent = await deps.authenticate(request, new Uint8Array());
        const result = await deps
          .owner()
          .get(create(GetAccessRequestRequestSchema, { agentId: agent.agentId, requestId: id }));
        return recordResponse(result.request, agent.agentId, id);
      }),
    agentCancel: (request: Request, id: string) =>
      guarded(async () => {
        requestId(id);
        target(request, `${ROOT}/${id}/cancel`);
        const bytes = await readAccessBody(request);
        const agent = await deps.authenticate(request, bytes);
        const mutation = parseMutation(bytes);
        const result = await deps.owner().cancel(
          create(CancelAccessRequestRequestSchema, {
            agentId: agent.agentId,
            requestId: id,
            expectedGeneration: mutation.expectedGeneration,
          }),
        );
        const row = checked(result.request, agent.agentId, id);
        return jsonResponse(
          toJson(AccessRequestSchema, row),
          row.state === AccessRequestState.CANCELLING ? 202 : 200,
        );
      }),
    operatorList: (request: Request) =>
      guarded(async () => {
        await deps.authorizeOperator(request);
        const options = listOptions(request.url, true);
        return listResponse(
          await deps.owner().list(create(ListAccessRequestsRequestSchema, options)),
          options.limit,
          options.agentId,
        );
      }),
    operatorGet: (request: Request, owner: string, id: string) =>
      guarded(async () => {
        await deps.authorizeOperator(request);
        agentId(owner);
        requestId(id);
        const result = await deps
          .owner()
          .get(create(GetAccessRequestRequestSchema, { agentId: owner, requestId: id }));
        return recordResponse(result.request, owner, id);
      }),
    operatorApprove: (request: Request, owner: string, id: string) =>
      guarded(async () => {
        await deps.authorizeOperator(request);
        agentId(owner);
        requestId(id);
        const approval = parseApproval(await readAccessBody(request));
        const client = deps.owner();
        const input = create(ApproveAccessRequestRequestSchema, {
          ...approval,
          agentId: owner,
          requestId: id,
        });
        // A lost response can be recovered without fetching the dataset again.
        try {
          const retained = await client.approve(input);
          return recordResponse(retained.request, owner, id);
        } catch (error) {
          if (!(error instanceof AccessRequestError) || error.code !== "NOT_FOUND") throw error;
        }
        const result = await client.get(
          create(GetAccessRequestRequestSchema, { agentId: owner, requestId: id }),
        );
        const row = checked(result.request, owner, id);
        if (
          row.state !== AccessRequestState.PENDING_REVIEW ||
          row.generation !== input.expectedGeneration
        )
          throw new AccessRequestError("CONFLICT");
        const dataset = await deps.dataset(row.datasetId);
        if (
          dataset.status !== "FINALIZED" ||
          BigInt(dataset.revision) !== row.datasetRevision ||
          !dataset.finalization
        )
          throw new AccessRequestError("CONFLICT");
        const approved = await client.approve(
          create(ApproveAccessRequestRequestSchema, {
            ...input,
            dataset: create(AccessDatasetRevisionSchema, {
              datasetId: dataset.datasetId,
              revision: row.datasetRevision,
              checksum: Buffer.from(dataset.finalization.checksum, "hex"),
            }),
          }),
        );
        return recordResponse(approved.request, owner, id);
      }),
    operatorMutation: (
      request: Request,
      owner: string,
      id: string,
      operation: "reject" | "cancel",
    ) =>
      guarded(async () => {
        await deps.authorizeOperator(request);
        agentId(owner);
        requestId(id);
        const mutation = parseMutation(await readAccessBody(request));
        const input = {
          agentId: owner,
          requestId: id,
          expectedGeneration: mutation.expectedGeneration,
        };
        const client = deps.owner();
        const result =
          operation === "reject"
            ? await client.reject(create(RejectAccessRequestRequestSchema, input))
            : await client.cancel(create(CancelAccessRequestRequestSchema, input));
        return recordResponse(result.request, owner, id);
      }),
  };
}

function target(request: Request, path: string, query = false): string {
  const raw = request.headers.get(RAW_TARGET_HEADER);
  if (!raw) throw new AccessRequestError("SERVICE_UNAVAILABLE");
  if (raw !== path && !(query && raw.startsWith(`${path}?`) && raw.length > path.length + 1))
    throw new AccessRequestError("INVALID_REQUEST");
  return raw;
}

function checked(row: AccessRequest | undefined, owner?: string, id?: string): AccessRequest {
  if (
    row?.formatVersion !== 1 ||
    row.generation < 1n ||
    row.state < AccessRequestState.PENDING_REVIEW ||
    row.state > AccessRequestState.FAILED ||
    (owner !== undefined && row.agentId !== owner) ||
    (id !== undefined && row.requestId !== id) ||
    row.capability.length > 2 * 1024 * 1024
  )
    throw new AccessRequestError("SERVICE_UNAVAILABLE");
  return row;
}

function recordResponse(row: AccessRequest | undefined, owner: string, id: string) {
  return jsonResponse(toJson(AccessRequestSchema, checked(row, owner, id)));
}

function listResponse(
  result: Awaited<ReturnType<AccessOwnerClient["list"]>>,
  limit: number,
  owner?: string,
) {
  if (
    result.formatVersion !== 1 ||
    result.requests.length > limit ||
    (result.nextCursor !== undefined && !/^[A-Za-z0-9_-]{1,512}$/.test(result.nextCursor))
  )
    throw new AccessRequestError("SERVICE_UNAVAILABLE");
  for (const row of result.requests) {
    checked(row, owner);
    if (row.capability.length !== 0) throw new AccessRequestError("SERVICE_UNAVAILABLE");
  }
  return jsonResponse({
    formatVersion: result.formatVersion,
    requests: result.requests.map((row) => toJson(AccessRequestSchema, row)),
    ...(result.nextCursor === undefined ? {} : { nextCursor: result.nextCursor }),
  });
}

export function accessErrorResponse(error: unknown): Response {
  if (error instanceof OperatorAuthError) return operatorAuthResponse(error);
  if (error instanceof AgentAuthError) return publicAgentAuthResponse(error);
  const code =
    error instanceof AccessRequestError
      ? error.code
      : error instanceof DatasetClientError && error.status === 404
        ? "NOT_FOUND"
        : "SERVICE_UNAVAILABLE";
  const response = jsonResponse(
    { error: { code, retryable: code === "SERVICE_UNAVAILABLE" || code === "CAPACITY_EXCEEDED" } },
    STATUS[code],
  );
  if (code === "CAPACITY_EXCEEDED") response.headers.set("Retry-After", "30");
  return response;
}
