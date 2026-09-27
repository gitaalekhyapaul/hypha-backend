import {
  accessGraphResponse,
  loadAccessGraphConfig,
  publishAccessGraph,
} from "../../../lib/access-graph/index.ts";
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
    const body = (await request.json()) as unknown;
    const projection = await publishAccessGraph(body, operatorId, loadAccessGraphConfig());
    return Response.json(projection, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return accessGraphResponse(error);
  }
}
