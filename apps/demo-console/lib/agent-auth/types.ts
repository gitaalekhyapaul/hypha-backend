export type AgentAuthCode =
  | "INVALID_REQUEST"
  | "UNAUTHENTICATED"
  | "REQUEST_REPLAYED"
  | "SERVICE_UNAVAILABLE"
  | "AGENT_ALREADY_REGISTERED"
  | "NOT_FOUND";

export class AgentAuthError extends Error {
  constructor(readonly code: AgentAuthCode) {
    super(code);
    this.name = "AgentAuthError";
  }
}

export interface AgentRegistration {
  readonly formatVersion: 1;
  readonly agentId: string;
  readonly displayName: string;
  readonly publicKey: string;
  readonly keyThumbprint: string;
  readonly createdAtUnixSeconds: string;
}

export interface AgentRecord extends AgentRegistration {
  readonly active: boolean;
}

export interface AgentTokenMetadata {
  readonly formatVersion: 1;
  readonly tokenId: string;
  readonly agentId: string;
  readonly createdAtUnixSeconds: string;
  readonly expiresAtUnixSeconds: string;
  readonly revokedAtUnixSeconds?: string;
}

export interface AgentTokenRecord extends AgentTokenMetadata {
  readonly audience: string;
  readonly digest: string;
}

export interface AgentAuthStore {
  register(agent: AgentRecord): Promise<boolean>;
  getAgent(agentId: string): Promise<AgentRecord | undefined>;
  listAgents(): Promise<AgentRecord[]>;
  createToken(token: AgentTokenRecord): Promise<boolean>;
  getToken(tokenId: string): Promise<AgentTokenRecord | undefined>;
  listTokens(agentId: string): Promise<AgentTokenRecord[]>;
  revokeToken(agentId: string, tokenId: string, now: number): Promise<AgentTokenRecord | undefined>;
  consumeNonce(input: {
    agent: AgentRecord;
    token: AgentTokenRecord;
    nonce: string;
    timestamp: number;
    now: number;
  }): Promise<"admitted" | "replay" | "unauthenticated">;
}

export type AgentAuthCommand =
  | { operation: "validate_key"; agent_public_key: string }
  | {
      operation: "verify";
      audience: string;
      token_id: string;
      agent_public_key: string;
      request_method: string;
      canonical_request_target: string;
      request_body_hash: string;
      request_nonce: string;
      request_timestamp_unix_seconds: string;
      request_signature: string;
    };

export type AgentAuthVerifier = (command: AgentAuthCommand) => Promise<string>;
