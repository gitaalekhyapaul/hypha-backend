import { agentAuthResponse, readAgentOperatorBody } from "../agent-auth/http.ts";
import { agentAuthService } from "../agent-auth/runtime.ts";
import { AgentAuthError } from "../agent-auth/types.ts";
import {
  jsonResponse,
  OperatorAuthError,
  operatorAuthResponse,
  operatorAuthService,
} from "../operator-auth/index.ts";
import { policyActivationResponse } from "../policy-activation/http.ts";
import { executeAgentRoles, parseAgentRolesRequest } from "./service.ts";

export async function agentRolesResponse(
  request: Request,
  agentId: () => Promise<string>,
): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    if (request.method === "GET") await auth.verify(request);
    else await auth.authorizeMutation(request);
    const query = new URL(request.url).searchParams;
    let input: unknown;
    if (request.method === "GET") {
      const fields = ["policyVersionId", "approvalEpoch", "datasetId"];
      if (
        query.size !== fields.length ||
        fields.some((field) => query.getAll(field).length !== 1)
      ) {
        throw new AgentAuthError("INVALID_REQUEST");
      }
      input = { formatVersion: 1, operation: "list", ...Object.fromEntries(query) };
    } else {
      if (query.size !== 0) throw new AgentAuthError("INVALID_REQUEST");
      input = await readAgentOperatorBody(request);
    }
    const operation = parseAgentRolesRequest(input, request.method !== "GET");
    const agent = await agentAuthService().getAgent(await agentId());
    return jsonResponse(await executeAgentRoles(agent, operation));
  } catch (error) {
    if (error instanceof OperatorAuthError) return operatorAuthResponse(error);
    if (error instanceof AgentAuthError) return agentAuthResponse(error);
    return policyActivationResponse(error);
  }
}
