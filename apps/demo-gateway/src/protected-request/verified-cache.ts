import { createHash } from "node:crypto";

import { createClient } from "redis";

import { ProtectedQueryInfrastructureError } from "./errors.ts";
import type { VerificationCache, VerifiedCapability, VerifiedCapabilityClaims } from "./types.ts";
import {
  boundedIdentifier,
  canonicalHex,
  canonicalUnixSeconds,
  strictRecord,
} from "./validation.ts";

const CACHE_FORMAT_VERSION = 1;
const MAX_CACHE_ENTRY_BYTES = 4_096;
const MAX_CACHE_TTL_SECONDS = 24 * 60 * 60;
const CACHE_FIELDS = [
  "format_version",
  "verified_claims",
  "policy_record_id",
  "proof_digest",
] as const;
const CLAIM_FIELDS = [
  "format_version",
  "capability_id",
  "agent_key_thumbprint",
  "task_id",
  "purpose",
  "action",
  "resource_type",
  "resource_scope",
  "policy_hash",
  "relationship_snapshot_root",
  "snapshot_id",
  "issued_at_unix_seconds",
  "expires_at_unix_seconds",
] as const;

type RedisClient = ReturnType<typeof configuredRedisClient>;

export class RedisVerificationCache implements VerificationCache {
  readonly #url: string;
  #connection: Promise<RedisClient> | undefined;

  public constructor(url: string) {
    if (url.length === 0 || url.length > 4_096) {
      throw new ProtectedQueryInfrastructureError();
    }
    this.#url = url;
  }

  public async get(
    capabilityId: string,
    proofDigestHex: string,
  ): Promise<VerifiedCapability | undefined> {
    try {
      const value = await (await this.#client()).get(cacheKey(capabilityId, proofDigestHex));
      if (value === null) {
        return undefined;
      }
      const verification = parseCachedVerification(value);
      if (
        verification.claims.capabilityId !== capabilityId ||
        verification.proofDigestHex !== proofDigestHex
      ) {
        throw new ProtectedQueryInfrastructureError();
      }
      return verification;
    } catch {
      this.#connection = undefined;
      throw new ProtectedQueryInfrastructureError();
    }
  }

  public async put(verification: VerifiedCapability, ttlSeconds: number): Promise<void> {
    if (!Number.isSafeInteger(ttlSeconds) || ttlSeconds < 1 || ttlSeconds > MAX_CACHE_TTL_SECONDS) {
      throw new ProtectedQueryInfrastructureError();
    }
    try {
      const value = encodeCachedVerification(verification);
      await (await this.#client()).set(
        cacheKey(verification.claims.capabilityId, verification.proofDigestHex),
        value,
        { expiration: { type: "EX", value: ttlSeconds } },
      );
    } catch {
      this.#connection = undefined;
      throw new ProtectedQueryInfrastructureError();
    }
  }

  async #client(): Promise<RedisClient> {
    this.#connection ??= this.#connect();
    return this.#connection;
  }

  async #connect(): Promise<RedisClient> {
    const client = configuredRedisClient(this.#url);
    client.on("error", () => undefined);
    await client.connect();
    return client;
  }
}

export function encodeCachedVerification(verification: VerifiedCapability): string {
  const value = JSON.stringify({
    format_version: CACHE_FORMAT_VERSION,
    verified_claims: claimsToWire(verification.claims),
    policy_record_id: verification.policyRecordId,
    proof_digest: verification.proofDigestHex,
  });
  if (Buffer.byteLength(value, "utf8") > MAX_CACHE_ENTRY_BYTES) {
    throw new ProtectedQueryInfrastructureError();
  }
  parseCachedVerification(value);
  return value;
}

export function parseCachedVerification(value: string): VerifiedCapability {
  if (
    Buffer.byteLength(value, "utf8") === 0 ||
    Buffer.byteLength(value, "utf8") > MAX_CACHE_ENTRY_BYTES
  ) {
    throw new ProtectedQueryInfrastructureError();
  }
  try {
    const object = strictRecord(JSON.parse(value) as unknown, CACHE_FIELDS);
    if (object.format_version !== CACHE_FORMAT_VERSION) {
      throw new ProtectedQueryInfrastructureError();
    }
    return {
      outcome: "verified",
      claims: claimsFromWire(object.verified_claims),
      policyRecordId: boundedIdentifier(object.policy_record_id),
      proofDigestHex: canonicalHex(object.proof_digest, 32),
    };
  } catch {
    throw new ProtectedQueryInfrastructureError();
  }
}

function cacheKey(capabilityId: string, proofDigestHex: string): string {
  try {
    const canonicalCapabilityId = canonicalHex(capabilityId, 32);
    const canonicalProofDigest = canonicalHex(proofDigestHex, 32);
    const digest = createHash("sha256")
      .update("raziel/verified-capability-cache/v1\0", "utf8")
      .update(canonicalCapabilityId, "ascii")
      .update("\0", "utf8")
      .update(canonicalProofDigest, "ascii")
      .digest("hex");
    return `raziel:m5:verified:${digest}`;
  } catch {
    throw new ProtectedQueryInfrastructureError();
  }
}

function claimsToWire(claims: VerifiedCapabilityClaims): Record<string, unknown> {
  return {
    format_version: claims.formatVersion,
    capability_id: claims.capabilityId,
    agent_key_thumbprint: claims.agentKeyThumbprintHex,
    task_id: claims.taskId,
    purpose: claims.purpose,
    action: claims.action,
    resource_type: claims.resourceType,
    resource_scope: claims.resourceScope,
    policy_hash: claims.policyHashHex,
    relationship_snapshot_root: claims.relationshipSnapshotRootHex,
    snapshot_id: claims.snapshotId,
    issued_at_unix_seconds: claims.issuedAtUnixSeconds.toString(),
    expires_at_unix_seconds: claims.expiresAtUnixSeconds.toString(),
  };
}

function claimsFromWire(value: unknown): VerifiedCapabilityClaims {
  const object = strictRecord(value, CLAIM_FIELDS);
  if (object.format_version !== 1) {
    throw new ProtectedQueryInfrastructureError();
  }
  return {
    formatVersion: 1,
    capabilityId: canonicalHex(object.capability_id, 32),
    agentKeyThumbprintHex: canonicalHex(object.agent_key_thumbprint, 32),
    taskId: boundedIdentifier(object.task_id),
    purpose: boundedIdentifier(object.purpose),
    action: boundedIdentifier(object.action),
    resourceType: boundedIdentifier(object.resource_type),
    resourceScope: boundedIdentifier(object.resource_scope),
    policyHashHex: canonicalHex(object.policy_hash, 32),
    relationshipSnapshotRootHex: canonicalHex(object.relationship_snapshot_root, 32),
    snapshotId: boundedIdentifier(object.snapshot_id),
    issuedAtUnixSeconds: canonicalUnixSeconds(object.issued_at_unix_seconds),
    expiresAtUnixSeconds: canonicalUnixSeconds(object.expires_at_unix_seconds),
  };
}

function configuredRedisClient(url: string) {
  return createClient({ url });
}
