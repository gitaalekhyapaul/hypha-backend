import { spawn } from "node:child_process";
import { type DescMessage, fromBinary, type MessageShape, toBinary } from "@bufbuild/protobuf";
import {
  ApproveAccessRequestRequestSchema,
  ApproveAccessRequestResponseSchema,
  CancelAccessRequestRequestSchema,
  CancelAccessRequestResponseSchema,
  CreateAccessRequestRequestSchema,
  CreateAccessRequestResponseSchema,
  GetAccessRequestRequestSchema,
  GetAccessRequestResponseSchema,
  ListAccessRequestsRequestSchema,
  ListAccessRequestsResponseSchema,
  RejectAccessRequestRequestSchema,
  RejectAccessRequestResponseSchema,
  ResumeAccessRequestRequestSchema,
  ResumeAccessRequestResponseSchema,
  SubmitAccessIssuanceRequestSchema,
  SubmitAccessIssuanceResponseSchema,
} from "@raziel/proto-ts";
import { type AccessErrorCode, type AccessOwnerClient, AccessRequestError } from "./types.ts";

const EXIT_ERRORS: Record<number, AccessErrorCode> = {
  2: "SERVICE_UNAVAILABLE",
  3: "INVALID_REQUEST",
  4: "NOT_FOUND",
  5: "CONFLICT",
  6: "EXPIRED",
  7: "CAPACITY_EXCEEDED",
};

export function rustAccessOwner(executable: string, endpoint: string): AccessOwnerClient {
  async function call<I extends DescMessage, O extends DescMessage>(
    operation: string,
    inputSchema: I,
    outputSchema: O,
    input: MessageShape<I>,
  ): Promise<MessageShape<O>> {
    const request = toBinary(inputSchema, input);
    if (request.length > 32 * 1024) throw new AccessRequestError("INVALID_REQUEST");
    const output = await invoke(
      executable,
      endpoint,
      JSON.stringify({ operation, request_proto_hex: Buffer.from(request).toString("hex") }),
    );
    try {
      if (!/^(?:[0-9a-f]{2})+$/.test(output)) throw new Error();
      return fromBinary(outputSchema, Buffer.from(output, "hex"), { readUnknownFields: false });
    } catch {
      throw new AccessRequestError("SERVICE_UNAVAILABLE");
    }
  }
  return {
    approve: (input) =>
      call("approve", ApproveAccessRequestRequestSchema, ApproveAccessRequestResponseSchema, input),
    create: (input) =>
      call("create", CreateAccessRequestRequestSchema, CreateAccessRequestResponseSchema, input),
    get: (input) =>
      call("get", GetAccessRequestRequestSchema, GetAccessRequestResponseSchema, input),
    list: (input) =>
      call("list", ListAccessRequestsRequestSchema, ListAccessRequestsResponseSchema, input),
    reject: (input) =>
      call("reject", RejectAccessRequestRequestSchema, RejectAccessRequestResponseSchema, input),
    cancel: (input) =>
      call("cancel", CancelAccessRequestRequestSchema, CancelAccessRequestResponseSchema, input),
    resume: (input) =>
      call("resume", ResumeAccessRequestRequestSchema, ResumeAccessRequestResponseSchema, input),
    submitIssuance: (input) =>
      call(
        "submit_issuance",
        SubmitAccessIssuanceRequestSchema,
        SubmitAccessIssuanceResponseSchema,
        input,
      ),
  };
}

function invoke(executable: string, endpoint: string, input: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, [], {
      shell: false,
      env: {
        NODE_ENV: "production",
        PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
        RAZIEL_ACCESS_ENDPOINT: endpoint,
      },
      stdio: ["pipe", "pipe", "pipe"] as const,
    });
    const chunks: Buffer[] = [];
    let size = 0;
    let failed = false;
    const timeout = setTimeout(() => {
      failed = true;
      child.kill("SIGKILL");
    }, 10_000);
    child.stdout.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > 8 * 1024 * 1024) {
        failed = true;
        child.kill("SIGKILL");
      } else chunks.push(chunk);
    });
    child.stderr.on("data", () => {
      failed = true;
      child.kill("SIGKILL");
    });
    child.stdin.on("error", () => {
      failed = true;
    });
    child.on("error", () => {
      clearTimeout(timeout);
      reject(new AccessRequestError("SERVICE_UNAVAILABLE"));
    });
    child.on("close", (code) => {
      clearTimeout(timeout);
      if (failed || code !== 0)
        reject(
          new AccessRequestError(
            failed ? "SERVICE_UNAVAILABLE" : (EXIT_ERRORS[code ?? 2] ?? "SERVICE_UNAVAILABLE"),
          ),
        );
      else resolve(Buffer.concat(chunks).toString("ascii"));
    });
    child.stdin.end(input);
  });
}
