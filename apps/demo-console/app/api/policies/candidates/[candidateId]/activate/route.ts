import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../../lib/operator-auth/index.ts";
import {
  activatePolicyCandidate,
  loadPolicyActivationConfig,
  policyActivationResponse,
} from "../../../../../../lib/policy-activation/index.ts";

export const dynamic = "force-dynamic";

interface ActivationRouteContext {
  readonly params: Promise<{ readonly candidateId: string }>;
}

export async function POST(request: Request, context: ActivationRouteContext): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.authorizeMutation(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const { candidateId } = await context.params;
    const projection = await activatePolicyCandidate(
      candidateId,
      operatorId,
      loadPolicyActivationConfig(),
    );
    return Response.json(projection, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return policyActivationResponse(error);
  }
}
