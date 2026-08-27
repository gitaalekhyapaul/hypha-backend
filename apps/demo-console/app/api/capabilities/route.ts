import { loadCapabilityProcessConfig } from "../../../lib/agent-capability/config.ts";
import { capabilityErrorResponse } from "../../../lib/agent-capability/http.ts";
import { createCapability } from "../../../lib/agent-capability/lifecycle.ts";
import { operatorAuthResponse, operatorAuthService } from "../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.authorizeMutation(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const input = (await request.json()) as unknown;
    return Response.json(await createCapability(input, operatorId, loadCapabilityProcessConfig()), {
      headers: { "Cache-Control": "no-store" },
    });
  } catch (error) {
    return capabilityErrorResponse(error);
  }
}
