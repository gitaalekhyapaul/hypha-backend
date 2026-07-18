import { createHealthPayload } from "../../../src/service-status";

export const dynamic = "force-dynamic";

export function GET(): Response {
  return Response.json(createHealthPayload());
}
