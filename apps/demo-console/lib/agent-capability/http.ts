import { CapabilityFlowError } from "./types.ts";

export function capabilityErrorResponse(error: unknown): Response {
  const code = error instanceof CapabilityFlowError ? error.code : "CAPABILITY_UNAVAILABLE";
  const status =
    code === "INVALID_CAPABILITY_REQUEST"
      ? 400
      : code === "CAPABILITY_NOT_FOUND"
        ? 404
        : code === "CAPABILITY_CONFLICT"
          ? 409
          : 503;
  return Response.json({ error: code }, { status, headers: { "Cache-Control": "no-store" } });
}
