import { createHash } from "node:crypto";
import { createClient } from "redis";
import {
  AgentAuthError,
  type AgentAuthStore,
  type AgentRecord,
  type AgentTokenRecord,
} from "./types.ts";

type RedisClient = ReturnType<typeof createClient>;

const REGISTER = `
if redis.call('HEXISTS', KEYS[2], ARGV[1]) == 1 then return 0 end
if redis.call('HEXISTS', KEYS[1], ARGV[2]) == 1 then return 0 end
redis.call('HSET', KEYS[1], ARGV[2], ARGV[3])
redis.call('HSET', KEYS[2], ARGV[1], ARGV[2])
return 1`;

const CREATE_TOKEN = `
local a = redis.call('HGET', KEYS[1], ARGV[1])
if not a or not cjson.decode(a).active then return 0 end
return redis.call('HSETNX', KEYS[2], ARGV[2], ARGV[3])`;

const REVOKE = `
local raw = redis.call('HGET', KEYS[1], ARGV[1])
if not raw then return '' end
local token = cjson.decode(raw)
if token.agentId ~= ARGV[2] then return '' end
if not token.revokedAtUnixSeconds then
  token.revokedAtUnixSeconds = ARGV[3]
  raw = cjson.encode(token)
  redis.call('HSET', KEYS[1], ARGV[1], raw)
end
return raw`;

// Compare the verified records again inside the same operation that admits the nonce.
const CONSUME = `
local a = redis.call('HGET', KEYS[1], ARGV[1])
local t = redis.call('HGET', KEYS[2], ARGV[2])
if not a or not t then return 0 end
local agent = cjson.decode(a)
local token = cjson.decode(t)
local expectedAgent = cjson.decode(ARGV[3])
local expectedToken = cjson.decode(ARGV[4])
if not agent.active or agent.publicKey ~= expectedAgent.publicKey or
  agent.keyThumbprint ~= expectedAgent.keyThumbprint or
  token.agentId ~= ARGV[1] or token.digest ~= expectedToken.digest or
  token.audience ~= expectedToken.audience or token.revokedAtUnixSeconds or
  token.createdAtUnixSeconds ~= expectedToken.createdAtUnixSeconds or
  token.expiresAtUnixSeconds ~= expectedToken.expiresAtUnixSeconds then return 0 end
local now = tonumber(redis.call('TIME')[1])
local stamp = tonumber(ARGV[5])
local created = tonumber(token.createdAtUnixSeconds)
local expires = tonumber(token.expiresAtUnixSeconds)
if stamp < now - 300 or stamp > now + 30 or
  now < created or stamp < created or now >= expires or stamp >= expires then return 0 end
if not redis.call('SET', KEYS[3], '1', 'NX', 'EXAT', stamp + 301) then return 2 end
return 1`;

export class RedisAgentAuthStore implements AgentAuthStore {
  private readonly prefix: string;
  private readonly client: RedisClient;
  private connecting: Promise<void> | undefined;

  constructor(url: string, audience: string) {
    this.prefix = `raziel:agent-api:v1:${createHash("sha256").update(audience).digest("hex")}:`;
    this.client = createClient({
      url,
      disableOfflineQueue: true,
      socket: { connectTimeout: 2000, reconnectStrategy: false },
    });
    this.client.on("error", () => undefined);
  }

  private async run<T>(operation: (client: RedisClient) => Promise<T>): Promise<T> {
    try {
      if (!this.client.isReady) {
        this.connecting ??= this.client
          .connect()
          .then(() => undefined)
          .finally(() => {
            this.connecting = undefined;
          });
        await this.connecting;
      }
      return await operation(this.client);
    } catch {
      throw new AgentAuthError("SERVICE_UNAVAILABLE");
    }
  }

  async close(): Promise<void> {
    if (this.client.isOpen) await this.client.quit();
  }

  async register(agent: AgentRecord): Promise<boolean> {
    return this.run(
      async (client) =>
        (await client.eval(REGISTER, {
          keys: [this.key("agents"), this.key("keys")],
          arguments: [agent.keyThumbprint, agent.agentId, JSON.stringify(agent)],
        })) === 1,
    );
  }

  async getAgent(id: string): Promise<AgentRecord | undefined> {
    return this.run(async (client) =>
      parse<AgentRecord>(await client.hGet(this.key("agents"), id)),
    );
  }

  async listAgents(): Promise<AgentRecord[]> {
    return this.run(async (client) =>
      Object.values(await client.hGetAll(this.key("agents")))
        .map((value) => JSON.parse(value) as AgentRecord)
        .sort((a, b) => a.createdAtUnixSeconds.localeCompare(b.createdAtUnixSeconds)),
    );
  }

  async createToken(token: AgentTokenRecord): Promise<boolean> {
    return this.run(
      async (client) =>
        (await client.eval(CREATE_TOKEN, {
          keys: [this.key("agents"), this.key("tokens")],
          arguments: [token.agentId, token.tokenId, JSON.stringify(token)],
        })) === 1,
    );
  }

  async getToken(id: string): Promise<AgentTokenRecord | undefined> {
    return this.run(async (client) =>
      parse<AgentTokenRecord>(await client.hGet(this.key("tokens"), id)),
    );
  }

  async listTokens(agentId: string): Promise<AgentTokenRecord[]> {
    return this.run(async (client) =>
      Object.values(await client.hGetAll(this.key("tokens")))
        .map((value) => JSON.parse(value) as AgentTokenRecord)
        .filter((token) => token.agentId === agentId),
    );
  }

  async revokeToken(
    agentId: string,
    tokenId: string,
    now: number,
  ): Promise<AgentTokenRecord | undefined> {
    return this.run(async (client) => {
      const raw = await client.eval(REVOKE, {
        keys: [this.key("tokens")],
        arguments: [tokenId, agentId, String(now)],
      });
      return typeof raw === "string" && raw ? (JSON.parse(raw) as AgentTokenRecord) : undefined;
    });
  }

  async consumeNonce(
    input: Parameters<AgentAuthStore["consumeNonce"]>[0],
  ): Promise<"admitted" | "replay" | "unauthenticated"> {
    return this.run(async (client) => {
      const result = await client.eval(CONSUME, {
        keys: [
          this.key("agents"),
          this.key("tokens"),
          this.key(`nonce:${input.token.tokenId}:${input.nonce}`),
        ],
        arguments: [
          input.agent.agentId,
          input.token.tokenId,
          JSON.stringify(input.agent),
          JSON.stringify(input.token),
          String(input.timestamp),
        ],
      });
      return result === 1 ? "admitted" : result === 2 ? "replay" : "unauthenticated";
    });
  }

  private key(suffix: string): string {
    return this.prefix + suffix;
  }
}

function parse<T>(raw: string | null | undefined): T | undefined {
  return raw ? (JSON.parse(raw) as T) : undefined;
}
