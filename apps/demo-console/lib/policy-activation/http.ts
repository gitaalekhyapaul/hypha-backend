import { jsonResponse } from "../operator-auth/index.ts";
import { PolicyActivationError, type PolicyActivationFailureCode } from "./types.ts";

export function policyActivationResponse(error: unknown): Response {
  const code: PolicyActivationFailureCode =
    error instanceof PolicyActivationError ? error.code : "POLICY_ACTIVATION_UNAVAILABLE";
  return jsonResponse({ error: code }, statusFor(code));
}

function statusFor(code: PolicyActivationFailureCode): number {
  switch (code) {
    case "INVALID_POLICY_ACTIVATION_REQUEST":
      return 400;
    case "POLICY_ACTIVATION_CONFLICT":
      return 409;
    case "POLICY_ACTIVATION_REJECTED":
      return 422;
    case "POLICY_ACTIVATION_UNAVAILABLE":
      return 503;
  }
}
