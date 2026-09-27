import { accessHandlers } from "../../../../../../lib/access-requests/runtime.ts";
export const dynamic = "force-dynamic";
export const runtime = "nodejs";
export async function GET(request: Request, context: { params: Promise<{ requestId: string }> }) {
  return accessHandlers.agentGet(request, (await context.params).requestId);
}
