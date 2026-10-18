import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { RAW_TARGET_HEADER } from "../../server/agent-transport.ts";
import {
  AgentAuthError,
  type AgentAuthStore,
  type AgentAuthVerifier,
  type AgentRecord,
  type AgentRegistration,
  type AgentTokenMetadata,
  type AgentTokenRecord,
} from "./types.ts";
import {
  assertId,
  exactObject,
  MAX_TOKEN_LIFETIME_SECONDS,
  publicKeyBytes,
  tokenParts,
  wholeSeconds,
} from "./validation.ts";

export class AgentAuthService {
  constructor(
    readonly audience: string,
    private readonly store: AgentAuthStore,
    private readonly verify: AgentAuthVerifier,
    private readonly clock: () => number = () => Math.floor(Date.now() / 1000),
  ) {
    if (!/^[\x20-\x7e]{1,128}$/.test(audience)) {
      throw new AgentAuthError("SERVICE_UNAVAILABLE");
    }
  }

  async register(input: unknown): Promise<AgentRegistration> {
    const value = exactObject(input, ["formatVersion", "displayName", "publicKey"]);
    if (
      value.formatVersion !== 1 ||
      typeof value.displayName !== "string" ||
      value.displayName.trim() !== value.displayName ||
      value.displayName.length === 0 ||
      Buffer.byteLength(value.displayName) > 128 ||
      [...value.displayName].some(
        (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
      )
    ) {
      throw new AgentAuthError("INVALID_REQUEST");
    }
    const key = publicKeyBytes(value.publicKey);
    let thumbprint: string;
    try {
      thumbprint = await this.verify({
        operation: "validate_key",
        agent_public_key: key.toString("hex"),
      });
    } catch (error) {
      if (error instanceof AgentAuthError && error.code === "UNAUTHENTICATED") {
        throw new AgentAuthError("INVALID_REQUEST");
      }
      throw error;
    }
    const agent: AgentRecord = {
      formatVersion: 1,
      agentId: randomBytes(16).toString("hex"),
      displayName: value.displayName,
      publicKey: key.toString("base64"),
      keyThumbprint: Buffer.from(thumbprint, "hex").toString("base64"),
      createdAtUnixSeconds: String(this.clock()),
      active: true,
    };
    if (!(await this.store.register(agent))) throw new AgentAuthError("AGENT_ALREADY_REGISTERED");
    return registration(agent);
  }

  async listAgents(): Promise<AgentRegistration[]> {
    return (await this.store.listAgents()).map(registration);
  }

  async getAgent(agentId: string): Promise<AgentRegistration> {
    assertId(agentId);
    if (agentId.length !== 32) throw new AgentAuthError("INVALID_REQUEST");
    const agent = await this.store.getAgent(agentId);
    if (!agent?.active) throw new AgentAuthError("NOT_FOUND");
    return registration(agent);
  }

  async issueToken(
    agentId: string,
    input: unknown,
  ): Promise<{ token: string; metadata: AgentTokenMetadata }> {
    assertId(agentId);
    const value = exactObject(input, ["formatVersion", "expiresAtUnixSeconds"]);
    const expires = wholeSeconds(value.expiresAtUnixSeconds);
    const now = this.clock();
    if (value.formatVersion !== 1 || expires <= now || expires > now + MAX_TOKEN_LIFETIME_SECONDS) {
      throw new AgentAuthError("INVALID_REQUEST");
    }
    const agent = await this.store.getAgent(agentId);
    if (!agent?.active) throw new AgentAuthError("NOT_FOUND");
    const tokenId = randomBytes(16).toString("hex");
    const token = `rz1.${tokenId}.${randomBytes(32).toString("base64url")}`;
    const record: AgentTokenRecord = {
      formatVersion: 1,
      tokenId,
      agentId,
      audience: this.audience,
      createdAtUnixSeconds: String(now),
      expiresAtUnixSeconds: String(expires),
      digest: digest(token),
    };
    if (!(await this.store.createToken(record))) throw new AgentAuthError("SERVICE_UNAVAILABLE");
    return { token, metadata: tokenMetadata(record) };
  }

  async listTokens(agentId: string): Promise<AgentTokenMetadata[]> {
    assertId(agentId);
    if (!(await this.store.getAgent(agentId))) throw new AgentAuthError("NOT_FOUND");
    return (await this.store.listTokens(agentId)).map(tokenMetadata);
  }

  async revokeToken(agentId: string, tokenId: string, input: unknown): Promise<AgentTokenMetadata> {
    assertId(agentId);
    assertId(tokenId);
    if (exactObject(input, ["formatVersion"]).formatVersion !== 1) {
      throw new AgentAuthError("INVALID_REQUEST");
    }
    const token = await this.store.revokeToken(agentId, tokenId, this.clock());
    if (!token) throw new AgentAuthError("NOT_FOUND");
    return tokenMetadata(token);
  }

  async authenticate(
    request: Request,
    body: Uint8Array = new Uint8Array(),
  ): Promise<{
    agent: AgentRegistration;
    token: AgentTokenMetadata;
    audience: string;
  }> {
    let timestamp: number;
    const authorization = singleHeader(request, "authorization", 128);
    if (!authorization.startsWith("Bearer ")) throw new AgentAuthError("UNAUTHENTICATED");
    const bearer = authorization.slice(7);
    const tokenId = tokenParts(bearer).tokenId;
    const timestampText = singleHeader(request, "x-raziel-agent-timestamp", 12);
    try {
      timestamp = wholeSeconds(timestampText);
    } catch {
      throw new AgentAuthError("UNAUTHENTICATED");
    }
    const nonce = singleHeader(request, "x-raziel-agent-nonce", 64);
    const signature = singleHeader(request, "x-raziel-agent-signature", 128);
    if (
      singleHeader(request, "x-raziel-agent-version", 1) !== "1" ||
      !/^[0-9a-f]{64}$/.test(nonce) ||
      !/^[0-9a-f]{128}$/.test(signature) ||
      request.headers.has("content-encoding") ||
      (request.method === "GET" &&
        (body.length > 0 ||
          request.body !== null ||
          request.headers.has("transfer-encoding") ||
          (request.headers.has("content-length") && request.headers.get("content-length") !== "0")))
    ) {
      throw new AgentAuthError("UNAUTHENTICATED");
    }
    const now = this.clock();
    const token = await this.store.getToken(tokenId);
    if (
      !token ||
      token.audience !== this.audience ||
      !tokenActive(token, now, timestamp) ||
      !/^[0-9a-f]{64}$/.test(token.digest) ||
      !timingSafeEqual(Buffer.from(token.digest, "hex"), Buffer.from(digest(bearer), "hex"))
    ) {
      throw new AgentAuthError("UNAUTHENTICATED");
    }
    const agent = await this.store.getAgent(token.agentId);
    if (!agent?.active) throw new AgentAuthError("UNAUTHENTICATED");
    const rawTarget = singleHeader(request, RAW_TARGET_HEADER, 8192);
    const thumbprint = await this.verify({
      operation: "verify",
      audience: this.audience,
      token_id: tokenId,
      agent_public_key: publicKeyBytes(agent.publicKey).toString("hex"),
      request_method: request.method,
      canonical_request_target: rawTarget,
      request_body_hash: createHash("sha256").update(body).digest("hex"),
      request_nonce: nonce,
      request_timestamp_unix_seconds: timestampText,
      request_signature: signature,
    });
    if (Buffer.from(thumbprint, "hex").toString("base64") !== agent.keyThumbprint) {
      throw new AgentAuthError("SERVICE_UNAVAILABLE");
    }
    const admitted = await this.store.consumeNonce({
      agent,
      token,
      nonce,
      timestamp,
      now: this.clock(),
    });
    if (admitted === "replay") throw new AgentAuthError("REQUEST_REPLAYED");
    if (admitted !== "admitted") throw new AgentAuthError("UNAUTHENTICATED");
    return { agent: registration(agent), token: tokenMetadata(token), audience: this.audience };
  }
}

export function tokenActive(token: AgentTokenRecord, now: number, timestamp: number): boolean {
  const created = wholeSeconds(token.createdAtUnixSeconds);
  const expires = wholeSeconds(token.expiresAtUnixSeconds);
  return (
    token.revokedAtUnixSeconds === undefined &&
    timestamp >= now - 300 &&
    timestamp <= now + 30 &&
    created <= timestamp &&
    timestamp < expires &&
    created <= now &&
    now < expires
  );
}

function singleHeader(request: Request, name: string, max: number): string {
  const value = request.headers.get(name);
  if (value === null || value.length === 0 || value.length > max || value.includes(",")) {
    throw new AgentAuthError("UNAUTHENTICATED");
  }
  return value;
}

function digest(value: string): string {
  return createHash("sha256").update(value, "ascii").digest("hex");
}

function registration(agent: AgentRecord): AgentRegistration {
  return {
    formatVersion: 1,
    agentId: agent.agentId,
    displayName: agent.displayName,
    publicKey: agent.publicKey,
    keyThumbprint: agent.keyThumbprint,
    createdAtUnixSeconds: agent.createdAtUnixSeconds,
  };
}

function tokenMetadata(token: AgentTokenRecord): AgentTokenMetadata {
  return {
    formatVersion: 1,
    tokenId: token.tokenId,
    agentId: token.agentId,
    createdAtUnixSeconds: token.createdAtUnixSeconds,
    expiresAtUnixSeconds: token.expiresAtUnixSeconds,
    ...(token.revokedAtUnixSeconds === undefined
      ? {}
      : { revokedAtUnixSeconds: token.revokedAtUnixSeconds }),
  };
}
