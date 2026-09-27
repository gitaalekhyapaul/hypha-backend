import type {
  ApproveAccessRequestRequest,
  ApproveAccessRequestResponse,
  CancelAccessRequestRequest,
  CancelAccessRequestResponse,
  CreateAccessRequestRequest,
  CreateAccessRequestResponse,
  GetAccessRequestRequest,
  GetAccessRequestResponse,
  ListAccessRequestsRequest,
  ListAccessRequestsResponse,
  RejectAccessRequestRequest,
  RejectAccessRequestResponse,
} from "@raziel/proto-ts";

export type AccessErrorCode =
  | "INVALID_REQUEST"
  | "NOT_FOUND"
  | "CONFLICT"
  | "EXPIRED"
  | "CAPACITY_EXCEEDED"
  | "SERVICE_UNAVAILABLE";

export class AccessRequestError extends Error {
  constructor(readonly code: AccessErrorCode) {
    super(code);
    this.name = "AccessRequestError";
  }
}

export interface AccessOwnerClient {
  approve(input: ApproveAccessRequestRequest): Promise<ApproveAccessRequestResponse>;
  create(input: CreateAccessRequestRequest): Promise<CreateAccessRequestResponse>;
  get(input: GetAccessRequestRequest): Promise<GetAccessRequestResponse>;
  list(input: ListAccessRequestsRequest): Promise<ListAccessRequestsResponse>;
  reject(input: RejectAccessRequestRequest): Promise<RejectAccessRequestResponse>;
  cancel(input: CancelAccessRequestRequest): Promise<CancelAccessRequestResponse>;
}
