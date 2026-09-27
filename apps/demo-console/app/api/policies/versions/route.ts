import { operatorAuthResponse, operatorAuthService } from "../../../../lib/operator-auth/index.ts";
import {
  policyCandidateResponse,
  readCandidateJson,
} from "../../../../lib/policy-candidate/index.ts";
import { preparePolicyVersion } from "../../../../lib/policy-version/store.ts";
import { loadPolicyVersionConfig } from "../../../../lib/policy-version/validation.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.authorizeMutation(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const projection = await preparePolicyVersion(
      await readCandidateJson(request),
      loadPolicyVersionConfig(),
    );
    return Response.json(projection, { status: 201, headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return policyCandidateResponse(error);
  }
}
