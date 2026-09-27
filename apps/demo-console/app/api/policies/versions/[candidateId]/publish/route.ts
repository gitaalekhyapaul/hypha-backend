import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../../lib/operator-auth/index.ts";
import { policyActivationResponse } from "../../../../../../lib/policy-activation/index.ts";
import {
  PolicyCandidateError,
  policyCandidateResponse,
} from "../../../../../../lib/policy-candidate/index.ts";
import { publishPolicyVersion } from "../../../../../../lib/policy-version/publication.ts";
import { policyWorkflowResponse } from "../../../../../../lib/policy-workflow.ts";

export const dynamic = "force-dynamic";

export async function POST(
  request: Request,
  context: { readonly params: Promise<{ readonly candidateId: string }> },
): Promise<Response> {
  let operatorId: string;
  try {
    const auth = await operatorAuthService();
    operatorId = (await auth.authorizeMutation(request)).operatorId;
  } catch (error) {
    return operatorAuthResponse(error);
  }
  const unavailable = policyWorkflowResponse("versioned");
  if (unavailable) return unavailable;
  try {
    const { candidateId } = await context.params;
    const projection = await publishPolicyVersion(candidateId, operatorId);
    return Response.json(projection, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return error instanceof PolicyCandidateError
      ? policyCandidateResponse(error)
      : policyActivationResponse(error);
  }
}
