import { createOperatorAgentSession } from "../../../lib/agent-capability/session-store.ts";
import { operatorAuthResponse, operatorAuthService } from "../../../lib/operator-auth/index.ts";
import { policyWorkflowResponse } from "../../../lib/policy-workflow.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.authorizeMutation(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }
  const unavailable = policyWorkflowResponse("legacy");
  if (unavailable) return unavailable;
  try {
    return Response.json(await createOperatorAgentSession(operatorId), {
      headers: { "Cache-Control": "no-store" },
    });
  } catch {
    return Response.json({ error: "AGENT_SESSION_UNAVAILABLE" }, { status: 503 });
  }
}
