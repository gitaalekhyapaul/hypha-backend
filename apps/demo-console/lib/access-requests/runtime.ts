import { agentAuthService } from "../agent-auth/runtime.ts";
import { loadDatasetGatewayConfig, SyntheticDatasetClient } from "../dataset-client/index.ts";
import { operatorAuthService } from "../operator-auth/index.ts";
import { createAccessHandlers } from "./http.ts";
import { rustAccessOwner } from "./process.ts";
import { AccessRequestError } from "./types.ts";

export const accessHandlers = createAccessHandlers({
  authenticate: async (request, body) =>
    (await agentAuthService().authenticate(request, body)).agent,
  authorizeOperator: async (request) => {
    const auth = await operatorAuthService();
    if (request.method === "GET") await auth.verify(request);
    else await auth.authorizeMutation(request);
  },
  owner: () => {
    const executable = process.env.RAZIEL_ACCESS_CLIENT_EXECUTABLE;
    const endpoint = process.env.RAZIEL_ACCESS_ENDPOINT;
    if (!executable || !endpoint) throw new AccessRequestError("SERVICE_UNAVAILABLE");
    return rustAccessOwner(executable, endpoint);
  },
  dataset: (id) => new SyntheticDatasetClient(loadDatasetGatewayConfig()).get(id),
});
