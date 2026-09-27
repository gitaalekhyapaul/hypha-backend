import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../lib/operator-auth/index.ts";
import { listApprovedPolicyVersions } from "../../../../../lib/policy-activation/process.ts";
import { loadPolicyActivationConfig } from "../../../../../lib/policy-activation/validation.ts";
import { policyWorkflowResponse } from "../../../../../lib/policy-workflow.ts";

export const dynamic = "force-dynamic";

export async function GET(request: Request): Promise<Response> {
  try {
    await (await operatorAuthService()).verify(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  const unavailable = policyWorkflowResponse("versioned");
  if (unavailable) return unavailable;
  try {
    const config = loadPolicyActivationConfig({
      ...process.env,
      RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: process.env.RAZIEL_POLICY_PUBLICATION_SOCKET_PATH,
    });
    const catalogue = await listApprovedPolicyVersions({ ...config, publication: true });
    return Response.json(catalogue, { headers: { "Cache-Control": "no-store" } });
  } catch {
    return Response.json(
      { error: "POLICY_CATALOGUE_UNAVAILABLE" },
      { status: 503, headers: { "Cache-Control": "no-store" } },
    );
  }
}
