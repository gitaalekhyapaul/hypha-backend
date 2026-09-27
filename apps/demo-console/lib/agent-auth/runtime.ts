import { rustAgentVerifier } from "./process.ts";
import { RedisAgentAuthStore } from "./redis-store.ts";
import { AgentAuthService } from "./service.ts";
import { AgentAuthError } from "./types.ts";

const RUNTIME = Symbol.for("raziel.agent-auth.v1");
type Runtime = typeof globalThis & { [RUNTIME]?: AgentAuthService };

export function agentAuthService(): AgentAuthService {
  const runtime = globalThis as Runtime;
  if (runtime[RUNTIME]) return runtime[RUNTIME];
  const url = process.env.RAZIEL_REDIS_URL;
  const audience = process.env.RAZIEL_TRUST_DOMAIN_ID;
  const executable = process.env.RAZIEL_AGENT_AUTH_EXECUTABLE;
  if (!url || !audience || !executable) throw new AgentAuthError("SERVICE_UNAVAILABLE");
  const service = new AgentAuthService(
    audience,
    new RedisAgentAuthStore(url, audience),
    rustAgentVerifier(executable),
  );
  runtime[RUNTIME] = service;
  return service;
}
