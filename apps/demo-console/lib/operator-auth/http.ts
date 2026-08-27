import { loadOperatorAuthConfig } from "./secret.ts";
import { OperatorAuthService } from "./service.ts";
import { OperatorAuthError, type OperatorAuthFailureCode } from "./types.ts";

const MAX_LOGIN_BODY_BYTES = 1024;

export async function operatorAuthService(): Promise<OperatorAuthService> {
  return new OperatorAuthService(loadOperatorAuthConfig());
}

export async function readLoginPassword(request: Request): Promise<string> {
  const contentLength = request.headers.get("content-length");
  if (contentLength !== null) {
    const parsed = Number(contentLength);
    if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > MAX_LOGIN_BODY_BYTES) {
      throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
    }
  }
  const raw = await request.text();
  if (Buffer.byteLength(raw, "utf8") > MAX_LOGIN_BODY_BYTES) {
    throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
  }
  let input: unknown;
  try {
    input = JSON.parse(raw) as unknown;
  } catch {
    throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
  }
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
  }
  const object = input as Record<string, unknown>;
  if (
    Object.keys(object).length !== 1 ||
    !("password" in object) ||
    typeof object.password !== "string"
  ) {
    throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
  }
  return object.password;
}

export function operatorAuthResponse(error: unknown): Response {
  const code: OperatorAuthFailureCode =
    error instanceof OperatorAuthError ? error.code : "OPERATOR_UNAVAILABLE";
  const status = statusFor(code);
  return jsonResponse({ error: code }, status);
}

export function jsonResponse(
  body: unknown,
  status = 200,
  headers: Record<string, string> = {},
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Cache-Control": "no-store",
      "Content-Type": "application/json; charset=utf-8",
      ...headers,
    },
  });
}

function statusFor(code: OperatorAuthFailureCode): number {
  switch (code) {
    case "INVALID_OPERATOR_REQUEST":
      return 400;
    case "OPERATOR_UNAUTHORIZED":
      return 401;
    case "OPERATOR_CSRF_REJECTED":
    case "OPERATOR_ORIGIN_REJECTED":
      return 403;
    case "OPERATOR_UNAVAILABLE":
      return 503;
  }
}
