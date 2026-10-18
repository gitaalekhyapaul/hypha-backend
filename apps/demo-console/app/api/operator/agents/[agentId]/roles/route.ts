import { agentRolesResponse } from "../../../../../../lib/agent-roles/http.ts";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";
type Context = { params: Promise<{ agentId: string }> };

export async function GET(request: Request, context: Context): Promise<Response> {
  return agentRolesResponse(request, async () => (await context.params).agentId);
}

export async function POST(request: Request, context: Context): Promise<Response> {
  return agentRolesResponse(request, async () => (await context.params).agentId);
}
