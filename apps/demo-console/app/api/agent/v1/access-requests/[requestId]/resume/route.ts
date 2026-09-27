import { accessHandlers } from "../../../../../../../lib/access-requests/runtime.ts";
export const dynamic = "force-dynamic";
export const runtime = "nodejs";
export async function POST(request: Request, context: { params: Promise<{ requestId: string }> }) {
  return accessHandlers.agentResume(request, (await context.params).requestId);
}
