import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../lib/operator-auth/index.ts";
import { policyCandidateResponse } from "../../../../../lib/policy-candidate/index.ts";
import { loadPolicyVersionCandidate } from "../../../../../lib/policy-version/store.ts";
import { loadPolicyVersionConfig } from "../../../../../lib/policy-version/validation.ts";

export const dynamic = "force-dynamic";

export async function GET(
  request: Request,
  context: { readonly params: Promise<{ readonly candidateId: string }> },
): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.verify(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const { candidateId } = await context.params;
    const projection = await loadPolicyVersionCandidate(candidateId, loadPolicyVersionConfig());
    return Response.json(projection, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return policyCandidateResponse(error);
  }
}
