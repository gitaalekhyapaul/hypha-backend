import { spawn } from "node:child_process";

import { AuthorizationRejectedError, ProtectedQueryInfrastructureError } from "./errors";
import type {
  ProtectedRequestEnvelope,
  ProtectedRequestVerifier,
  VerifiedCapabilityClaims,
  VerifierResult,
} from "./types";
import { boundedIdentifier, canonicalHex, canonicalUnixSeconds, strictRecord } from "./validation";

const MAX_VERIFIER_OUTPUT_BYTES = 64 * 1_024;
const MAX_VERIFIER_INPUT_BYTES = 4 * 1_024 * 1_024;
const VERIFIER_TIMEOUT_MILLISECONDS = 60_000;
const VERIFIED_OUTPUT_FIELDS = [
  "outcome",
  "failure_code",
  "verified_claims",
  "policy_record_id",
  "snapshot_id",
  "proof_digest",
] as const;
const REJECTED_OUTPUT_FIELDS = ["outcome", "failure_code"] as const;
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
const INSPECTION_FIELDS = ["capability_id"] as const;

export interface ProcessVerifierConfig {
  readonly executablePath: string;
  readonly endpoint: string;
}

export class ProcessProtectedRequestVerifier implements ProtectedRequestVerifier {
  readonly #config: ProcessVerifierConfig;

  public constructor(config: ProcessVerifierConfig) {
    if (config.executablePath.length === 0 || config.executablePath.length > 4_096) {
      throw new ProtectedQueryInfrastructureError();
    }
    validateEndpoint(config.endpoint);
    this.#config = config;
  }

  public async inspectCapabilityId(envelope: ProtectedRequestEnvelope): Promise<string> {
    const input = Buffer.from(
      JSON.stringify({
        operation: "inspect",
        capability_proto_hex: envelope.capabilityProtoHex,
      }),
      "utf8",
    );
    validateProcessInput(input);
    let output: Uint8Array;
    try {
      output = await runBoundedProcess(this.#config, input);
    } catch (error) {
      if (error instanceof VerifierCommandRejectedError) {
        throw new AuthorizationRejectedError();
      }
      throw error;
    }
    return parseCapabilityInspectionOutput(output);
  }

  public async verify(envelope: ProtectedRequestEnvelope): Promise<VerifierResult> {
    const input = Buffer.from(
      JSON.stringify({
        operation: "verify",
        capability_proto_hex: envelope.capabilityProtoHex,
        agent_public_key: envelope.agentPublicKeyHex,
        request_method: envelope.requestMethod,
        canonical_request_target: envelope.canonicalRequestTarget,
        request_body_hash: envelope.requestBodyHashHex,
        request_nonce: envelope.requestNonceHex,
        request_timestamp_unix_seconds: envelope.requestTimestampUnixSeconds.toString(),
        agent_request_signature: envelope.agentRequestSignatureHex,
      }),
      "utf8",
    );
    validateProcessInput(input);
    const output = await runBoundedProcess(this.#config, input);
    return parseVerifierOutput(output);
  }
}

export function parseVerifierOutput(bytes: Uint8Array): VerifierResult {
  if (bytes.length === 0 || bytes.length > MAX_VERIFIER_OUTPUT_BYTES) {
    throw new ProtectedQueryInfrastructureError();
  }
  let value: unknown;
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    value = JSON.parse(text) as unknown;
  } catch {
    throw new ProtectedQueryInfrastructureError();
  }

  try {
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      throw new ProtectedQueryInfrastructureError();
    }
    const candidate = value as Record<string, unknown>;
    const outcome = boundedIdentifier(candidate.outcome, 64);
    boundedIdentifier(candidate.failure_code, 128);
    if (outcome === "rejected" || outcome === "VERIFICATION_OUTCOME_REJECTED") {
      strictRecord(value, REJECTED_OUTPUT_FIELDS);
      return { outcome: "rejected" };
    }
    if (outcome !== "verified" && outcome !== "VERIFICATION_OUTCOME_VERIFIED") {
      throw new ProtectedQueryInfrastructureError();
    }
    const output = strictRecord(value, VERIFIED_OUTPUT_FIELDS);
    const claims = parseClaims(output.verified_claims);
    const policyRecordId = boundedIdentifier(output.policy_record_id);
    const snapshotId = boundedIdentifier(output.snapshot_id);
    if (policyRecordId.length === 0 || snapshotId !== claims.snapshotId) {
      throw new ProtectedQueryInfrastructureError();
    }
    return {
      outcome: "verified",
      claims,
      policyRecordId,
      proofDigestHex: canonicalHex(output.proof_digest, 32),
    };
  } catch (error) {
    if (error instanceof ProtectedQueryInfrastructureError) {
      throw error;
    }
    throw new ProtectedQueryInfrastructureError();
  }
}

export function parseCapabilityInspectionOutput(bytes: Uint8Array): string {
  if (bytes.length === 0 || bytes.length > MAX_VERIFIER_OUTPUT_BYTES) {
    throw new ProtectedQueryInfrastructureError();
  }
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    const output = strictRecord(JSON.parse(text) as unknown, INSPECTION_FIELDS);
    return canonicalHex(output.capability_id, 32);
  } catch {
    throw new ProtectedQueryInfrastructureError();
  }
}

function parseClaims(value: unknown): VerifiedCapabilityClaims {
  const object = strictRecord(value, CLAIM_FIELDS);
  if (object.format_version !== 1) {
    throw new ProtectedQueryInfrastructureError();
  }
  const policyHashHex = canonicalHex(object.policy_hash, 32);
  const relationshipSnapshotRootHex = canonicalHex(object.relationship_snapshot_root, 32);
  const issuedAtUnixSeconds = canonicalUnixSeconds(object.issued_at_unix_seconds);
  const expiresAtUnixSeconds = canonicalUnixSeconds(object.expires_at_unix_seconds);
  return {
    formatVersion: 1,
    capabilityId: canonicalHex(object.capability_id, 32),
    agentKeyThumbprintHex: canonicalHex(object.agent_key_thumbprint, 32),
    taskId: boundedIdentifier(object.task_id),
    purpose: boundedIdentifier(object.purpose),
    action: boundedIdentifier(object.action),
    resourceType: boundedIdentifier(object.resource_type),
    resourceScope: boundedIdentifier(object.resource_scope),
    policyHashHex,
    relationshipSnapshotRootHex,
    snapshotId: boundedIdentifier(object.snapshot_id),
    issuedAtUnixSeconds,
    expiresAtUnixSeconds,
  };
}

function validateEndpoint(value: string): void {
  if (value.length === 0 || value.length > 2_048) {
    throw new ProtectedQueryInfrastructureError();
  }
  try {
    const url = new URL(value);
    if ((url.protocol !== "http:" && url.protocol !== "https:") || url.username || url.password) {
      throw new ProtectedQueryInfrastructureError();
    }
  } catch {
    throw new ProtectedQueryInfrastructureError();
  }
}

function validateProcessInput(input: Buffer): void {
  if (input.length === 0 || input.length > MAX_VERIFIER_INPUT_BYTES) {
    throw new AuthorizationRejectedError();
  }
}

class VerifierCommandRejectedError extends Error {}

function runBoundedProcess(config: ProcessVerifierConfig, input: Buffer): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const child = spawn(config.executablePath, [], {
      env: { ...process.env, RAZIEL_VERIFIERD_ENDPOINT: config.endpoint },
      stdio: ["pipe", "pipe", "pipe"],
    });
    const stdout: Buffer[] = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    let settled = false;
    const timer = setTimeout(() => fail(), VERIFIER_TIMEOUT_MILLISECONDS);

    const finish = (operation: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      operation();
    };
    const fail = (): void => {
      if (!child.killed) {
        child.kill("SIGKILL");
      }
      finish(() => reject(new ProtectedQueryInfrastructureError()));
    };

    child.on("error", fail);
    child.stdout.on("data", (chunk: Buffer) => {
      stdoutBytes += chunk.length;
      if (stdoutBytes > MAX_VERIFIER_OUTPUT_BYTES) {
        fail();
        return;
      }
      stdout.push(chunk);
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderrBytes += chunk.length;
      if (stderrBytes > MAX_VERIFIER_OUTPUT_BYTES) {
        fail();
      }
    });
    child.stdin.on("error", fail);
    child.on("close", (code) => {
      if (code !== 0) {
        finish(() => reject(new VerifierCommandRejectedError()));
        return;
      }
      finish(() => resolve(Buffer.concat(stdout)));
    });
    child.stdin.end(input);
  });
}
