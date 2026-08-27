import { type AgentIdentity, createAgentIdentity } from "./sdk.ts";

import type { AgentSessionProjection } from "./types.ts";

interface AgentSessionRecord {
  readonly createdAtUnixSeconds: bigint;
  readonly identity: AgentIdentity;
}

interface AgentSessionGlobal {
  __razielM5AgentSessions?: Map<string, Promise<AgentSessionRecord>>;
}

const processState = globalThis as typeof globalThis & AgentSessionGlobal;
if (!processState.__razielM5AgentSessions) {
  processState.__razielM5AgentSessions = new Map<string, Promise<AgentSessionRecord>>();
}
const sessions = processState.__razielM5AgentSessions;

export async function createOperatorAgentSession(
  operatorId: string,
): Promise<AgentSessionProjection> {
  return projection(await operatorAgentSession(operatorId));
}

export async function operatorAgentIdentity(operatorId: string): Promise<AgentIdentity> {
  return (await operatorAgentSession(operatorId)).identity;
}

async function operatorAgentSession(operatorId: string): Promise<AgentSessionRecord> {
  let pending = sessions.get(operatorId);
  if (!pending) {
    pending = createSession();
    sessions.set(operatorId, pending);
  }
  try {
    return await pending;
  } catch (cause) {
    sessions.delete(operatorId);
    throw cause;
  }
}

async function createSession(): Promise<AgentSessionRecord> {
  return {
    createdAtUnixSeconds: BigInt(Math.floor(Date.now() / 1000)),
    identity: await createAgentIdentity(),
  };
}

function projection(record: AgentSessionRecord): AgentSessionProjection {
  if (record.identity.keyThumbprint.length !== 32) {
    throw new Error("agent key thumbprint has the wrong length");
  }
  const subjectId = lowercaseHex(record.identity.keyThumbprint);
  return {
    key_thumbprint: subjectId,
    subject_type: "agent",
    subject_id: subjectId,
    created_at_unix_seconds: record.createdAtUnixSeconds.toString(),
  };
}

function lowercaseHex(input: Uint8Array): string {
  return Array.from(input, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
