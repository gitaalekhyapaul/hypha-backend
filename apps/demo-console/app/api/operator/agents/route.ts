import { operatorAgentRequest, readAgentOperatorBody } from "../../../../lib/agent-auth/http.ts";
import { agentAuthService } from "../../../../lib/agent-auth/runtime.ts";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

export async function GET(request: Request): Promise<Response> {
  return operatorAgentRequest(request, async () => ({
    agents: await agentAuthService().listAgents(),
  }));
}

export async function POST(request: Request): Promise<Response> {
  return operatorAgentRequest(
    request,
    async () => agentAuthService().register(await readAgentOperatorBody(request)),
    201,
  );
}
