import { agentProtectedQuery } from "../../../../../lib/agent-query/runtime.ts";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

export async function POST(request: Request, context: { params: Promise<{ datasetId: string }> }) {
  const { datasetId } = await context.params;
  return agentProtectedQuery(request, datasetId);
}
