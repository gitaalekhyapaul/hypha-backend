import {
  operatorAgentRequest,
  readAgentOperatorBody,
} from "../../../../../../../../lib/agent-auth/http.ts";
import { agentAuthService } from "../../../../../../../../lib/agent-auth/runtime.ts";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

export async function POST(
  request: Request,
  context: { params: Promise<{ agentId: string; tokenId: string }> },
): Promise<Response> {
  return operatorAgentRequest(request, async () => {
    const { agentId, tokenId } = await context.params;
    return agentAuthService().revokeToken(agentId, tokenId, await readAgentOperatorBody(request));
  });
}
