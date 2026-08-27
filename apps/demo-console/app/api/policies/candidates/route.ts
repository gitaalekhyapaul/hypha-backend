import { operatorAuthResponse, operatorAuthService } from "../../../../lib/operator-auth/index.ts";
import {
  loadPolicyCandidateConfig,
  policyCandidateResponse,
  preparePolicyCandidate,
  readCandidateJson,
} from "../../../../lib/policy-candidate/index.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.authorizeMutation(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const input = await readCandidateJson(request);
    const projection = await preparePolicyCandidate(input, loadPolicyCandidateConfig());
    return Response.json(projection, { status: 201, headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return policyCandidateResponse(error);
  }
}
