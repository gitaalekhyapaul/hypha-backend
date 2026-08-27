import { jsonResponse } from "../operator-auth/index.ts";
import { PolicyCandidateError, type PolicyCandidateFailureCode } from "./types.ts";

const MAX_CANDIDATE_BODY_BYTES = 32 * 1024;

export async function readCandidateJson(request: Request): Promise<unknown> {
  const contentLength = request.headers.get("content-length");
  if (contentLength !== null) {
    const parsed = Number(contentLength);
    if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > MAX_CANDIDATE_BODY_BYTES) {
      throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
    }
  }
  const raw = await request.text();
  if (Buffer.byteLength(raw, "utf8") > MAX_CANDIDATE_BODY_BYTES) {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    throw new PolicyCandidateError("INVALID_CANDIDATE_REQUEST");
  }
}

export function policyCandidateResponse(error: unknown): Response {
  const code: PolicyCandidateFailureCode =
    error instanceof PolicyCandidateError ? error.code : "CANDIDATE_STORE_UNAVAILABLE";
  return jsonResponse({ error: code }, statusFor(code));
}

function statusFor(code: PolicyCandidateFailureCode): number {
  switch (code) {
    case "INVALID_CANDIDATE_REQUEST":
      return 400;
    case "CANDIDATE_NOT_FOUND":
      return 404;
    case "CANDIDATE_CONFLICT":
    case "CANDIDATE_NOT_READY":
      return 409;
    case "CANDIDATE_EXPIRED":
      return 410;
    case "COMPILER_FAILED":
    case "INSPECTION_FAILED":
      return 422;
    case "CANDIDATE_STORE_UNAVAILABLE":
    case "INVALID_OWNER_CONFIGURATION":
      return 503;
  }
}
