import { agentAuthService } from "../agent-auth/runtime.ts";
import { loadDatasetGatewayConfig } from "../dataset-client/index.ts";
import { createAgentQueryHandler } from "./http.ts";

export const agentProtectedQuery = createAgentQueryHandler({
  authenticate: async (request, body) =>
    (await agentAuthService().authenticate(request, body)).agent,
  gatewayUrl: () => loadDatasetGatewayConfig().baseUrl,
});
