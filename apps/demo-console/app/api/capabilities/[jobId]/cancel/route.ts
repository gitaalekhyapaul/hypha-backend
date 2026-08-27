import { loadCapabilityProcessConfig } from "../../../../../lib/agent-capability/config.ts";
import { capabilityErrorResponse } from "../../../../../lib/agent-capability/http.ts";
import { cancelCapability } from "../../../../../lib/agent-capability/lifecycle.ts";
import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

interface CapabilityCancelRouteContext {
  readonly params: Promise<{ readonly jobId: string }>;
}

export async function POST(
  request: Request,
  context: CapabilityCancelRouteContext,
): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.authorizeMutation(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const { jobId } = await context.params;
    return Response.json(await cancelCapability(jobId, operatorId, loadCapabilityProcessConfig()), {
      headers: { "Cache-Control": "no-store" },
    });
  } catch (error) {
    return capabilityErrorResponse(error);
  }
}
