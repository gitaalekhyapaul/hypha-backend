import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../lib/operator-auth/index.ts";
import {
  loadPolicyCandidateConfig,
  loadPolicyCandidateProjection,
  policyCandidateResponse,
} from "../../../../../lib/policy-candidate/index.ts";

export const dynamic = "force-dynamic";

interface CandidateRouteContext {
  readonly params: Promise<{ readonly candidateId: string }>;
}

export async function GET(request: Request, context: CandidateRouteContext): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.verify(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const { candidateId } = await context.params;
    const projection = await loadPolicyCandidateProjection(
      candidateId,
      loadPolicyCandidateConfig(),
    );
    return Response.json(projection, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return policyCandidateResponse(error);
  }
}
