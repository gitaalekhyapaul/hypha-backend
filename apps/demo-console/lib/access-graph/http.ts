import { jsonResponse } from "../operator-auth/index.ts";
import { AccessGraphError, type AccessGraphFailureCode } from "./types.ts";

export function accessGraphResponse(error: unknown): Response {
  const code: AccessGraphFailureCode =
    error instanceof AccessGraphError ? error.code : "ACCESS_GRAPH_UNAVAILABLE";
  return jsonResponse({ error: code }, statusFor(code));
}

function statusFor(code: AccessGraphFailureCode): number {
  switch (code) {
    case "INVALID_ACCESS_GRAPH_REQUEST":
      return 400;
    case "ACCESS_GRAPH_CONFLICT":
      return 409;
    case "ACCESS_GRAPH_REJECTED":
      return 422;
    case "ACCESS_GRAPH_UNAVAILABLE":
      return 503;
  }
}
