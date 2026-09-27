import { accessHandlers } from "../../../../../../../lib/access-requests/runtime.ts";
export const dynamic = "force-dynamic";
export const runtime = "nodejs";
export async function POST(
  request: Request,
  context: { params: Promise<{ agentId: string; requestId: string }> },
) {
  const { agentId, requestId } = await context.params;
  return accessHandlers.operatorApprove(request, agentId, requestId);
}
