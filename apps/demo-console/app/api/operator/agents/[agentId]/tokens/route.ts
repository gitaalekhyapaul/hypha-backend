import {
  operatorAgentRequest,
  readAgentOperatorBody,
} from "../../../../../../lib/agent-auth/http.ts";
import { agentAuthService } from "../../../../../../lib/agent-auth/runtime.ts";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";
type Context = { params: Promise<{ agentId: string }> };

export async function GET(request: Request, context: Context): Promise<Response> {
  return operatorAgentRequest(request, async () => ({
    tokens: await agentAuthService().listTokens((await context.params).agentId),
  }));
}

export async function POST(request: Request, context: Context): Promise<Response> {
  return operatorAgentRequest(
    request,
    async () =>
      agentAuthService().issueToken(
        (await context.params).agentId,
        await readAgentOperatorBody(request),
      ),
    201,
  );
}
