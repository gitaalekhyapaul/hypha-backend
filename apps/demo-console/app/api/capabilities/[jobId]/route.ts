import { loadCapabilityProcessConfig } from "../../../../lib/agent-capability/config.ts";
import { capabilityErrorResponse } from "../../../../lib/agent-capability/http.ts";
import { getCapability } from "../../../../lib/agent-capability/lifecycle.ts";
import { operatorAuthResponse, operatorAuthService } from "../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

interface CapabilityRouteContext {
  readonly params: Promise<{ readonly jobId: string }>;
}

export async function GET(request: Request, context: CapabilityRouteContext): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.verify(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const { jobId } = await context.params;
    return Response.json(await getCapability(jobId, operatorId, loadCapabilityProcessConfig()), {
      headers: { "Cache-Control": "no-store" },
    });
  } catch (error) {
    return capabilityErrorResponse(error);
  }
}
