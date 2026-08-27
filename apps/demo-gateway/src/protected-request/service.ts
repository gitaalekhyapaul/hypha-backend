import { createHash } from "node:crypto";
import { ReconciliationValidationError } from "../reconciliation/model";
import { DatasetNotFoundError } from "../reconciliation/store";
import { AuthorizationRejectedError, ProtectedQueryInfrastructureError } from "./errors";
import type {
  ProtectedQueryDependencies,
  ProtectedQueryResult,
  PublicReceipt,
  VerifiedCapabilityClaims,
} from "./types";
import { MAX_M5_CAPABILITY_VALIDITY_SECONDS } from "./types";
import { parseProtectedRequestEnvelope, validateRequestFreshness } from "./validation";

export class ProtectedQueryService {
  readonly #dependencies: Required<ProtectedQueryDependencies>;

  public constructor(dependencies: ProtectedQueryDependencies) {
    this.#dependencies = {
      ...dependencies,
      nowUnixSeconds: dependencies.nowUnixSeconds ?? systemNowUnixSeconds,
    };
  }

  public async execute(datasetId: string, input: unknown): Promise<ProtectedQueryResult> {
    const envelope = parseProtectedRequestEnvelope(input, datasetId);
    const nowUnixSeconds = this.#dependencies.nowUnixSeconds();
    validateRequestFreshness(envelope.requestTimestampUnixSeconds, nowUnixSeconds);

    let inspectedCapabilityId: string;
    try {
      inspectedCapabilityId = await this.#dependencies.verifier.inspectCapabilityId(envelope);
    } catch (error) {
      if (error instanceof AuthorizationRejectedError) {
        throw error;
      }
      throw new ProtectedQueryInfrastructureError();
    }

    let claimed: boolean;
    try {
      claimed = await this.#dependencies.nonceStore.claim(
        inspectedCapabilityId,
        envelope.requestNonceHex,
        MAX_M5_CAPABILITY_VALIDITY_SECONDS,
      );
    } catch {
      throw new ProtectedQueryInfrastructureError();
    }
    if (!claimed) {
      throw new AuthorizationRejectedError();
    }

    let verification: Awaited<ReturnType<ProtectedQueryDependencies["verifier"]["verify"]>>;
    try {
      verification = await this.#dependencies.verifier.verify(envelope);
    } catch (error) {
      if (error instanceof AuthorizationRejectedError) {
        throw error;
      }
      throw new ProtectedQueryInfrastructureError();
    }
    if (verification.outcome !== "verified") {
      throw new AuthorizationRejectedError();
    }
    validateVerifiedClaims(
      verification.claims,
      inspectedCapabilityId,
      datasetId,
      envelope.agentPublicKeyHex,
      envelope.requestTimestampUnixSeconds,
      nowUnixSeconds,
    );

    try {
      const rows = await this.#dependencies.queryDataset(datasetId, envelope.query);
      const receipt = createPublicReceipt({
        claims: verification.claims,
        policyRecordId: verification.policyRecordId,
        proofDigestHex: verification.proofDigestHex,
        requestMethod: envelope.requestMethod,
        canonicalRequestTarget: envelope.canonicalRequestTarget,
        requestBodyHashHex: envelope.requestBodyHashHex,
        requestNonceHex: envelope.requestNonceHex,
        requestTimestampUnixSeconds: envelope.requestTimestampUnixSeconds,
        occurredAtUnixSeconds: nowUnixSeconds,
      });
      try {
        await this.#dependencies.receiptStore.append(receipt);
      } catch {
        throw new ProtectedQueryInfrastructureError();
      }
      return {
        classification: "SYNTHETIC",
        dataset_id: datasetId,
        row_count: rows.length,
        rows,
        receipt,
      };
    } catch (error) {
      if (error instanceof ProtectedQueryInfrastructureError) {
        throw error;
      }
      if (error instanceof DatasetNotFoundError || error instanceof ReconciliationValidationError) {
        throw new AuthorizationRejectedError();
      }
      throw new ProtectedQueryInfrastructureError();
    }
  }
}

interface ReceiptInput {
  readonly claims: VerifiedCapabilityClaims;
  readonly policyRecordId: string;
  readonly proofDigestHex: string;
  readonly requestMethod: string;
  readonly canonicalRequestTarget: string;
  readonly requestBodyHashHex: string;
  readonly requestNonceHex: string;
  readonly requestTimestampUnixSeconds: bigint;
  readonly occurredAtUnixSeconds: bigint;
}

export function createPublicReceipt(input: ReceiptInput): PublicReceipt {
  const requestId = digestFields("raziel/protected-request-id/v1", [
    input.claims.capabilityId,
    input.requestMethod,
    input.canonicalRequestTarget,
    input.requestBodyHashHex,
    input.requestNonceHex,
    input.requestTimestampUnixSeconds.toString(),
  ]);
  const receiptId = digestFields("raziel/audit-receipt-id/v1", [
    requestId,
    input.proofDigestHex,
    "AUDIT_OUTCOME_ACCEPTED",
  ]);
  const publicClaimsDigest = createHash("sha256")
    .update(
      JSON.stringify({
        format_version: input.claims.formatVersion,
        capability_id: input.claims.capabilityId,
        agent_key_thumbprint: input.claims.agentKeyThumbprintHex,
        task_id: input.claims.taskId,
        purpose: input.claims.purpose,
        action: input.claims.action,
        resource_type: input.claims.resourceType,
        resource_scope: input.claims.resourceScope,
        policy_hash: input.claims.policyHashHex,
        relationship_snapshot_root: input.claims.relationshipSnapshotRootHex,
        snapshot_id: input.claims.snapshotId,
        issued_at_unix_seconds: input.claims.issuedAtUnixSeconds.toString(),
        expires_at_unix_seconds: input.claims.expiresAtUnixSeconds.toString(),
      }),
      "utf8",
    )
    .digest("hex");
  return {
    format_version: 1,
    receipt_id: `receipt-v1:${receiptId}`,
    receipt_type: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
    occurred_at: wholeSecondUtc(input.occurredAtUnixSeconds),
    request_id: `request-v1:${requestId}`,
    task_id: input.claims.taskId,
    capability_id: input.claims.capabilityId,
    proof_job_id: input.claims.capabilityId,
    policy_record_id: input.policyRecordId,
    policy_hash: input.claims.policyHashHex,
    snapshot_id: input.claims.snapshotId,
    relationship_snapshot_root: input.claims.relationshipSnapshotRootHex,
    public_claims_digest: publicClaimsDigest,
    proof_digest: input.proofDigestHex,
    action: input.claims.action,
    resource_type: input.claims.resourceType,
    resource_scope: input.claims.resourceScope,
    outcome: "AUDIT_OUTCOME_ACCEPTED",
    reason_code: "PUBLIC_ERROR_CODE_UNSPECIFIED",
  };
}

function digestFields(domain: string, fields: readonly string[]): string {
  const hash = createHash("sha256").update(domain, "utf8");
  for (const field of fields) {
    hash.update("\0", "utf8").update(field, "utf8");
  }
  return hash.digest("hex");
}

function wholeSecondUtc(unixSeconds: bigint): string {
  if (unixSeconds < 0n || unixSeconds > 253_402_300_799n) {
    throw new ProtectedQueryInfrastructureError();
  }
  return new Date(Number(unixSeconds) * 1_000).toISOString().replace(".000Z", "Z");
}

function validateVerifiedClaims(
  claims: VerifiedCapabilityClaims,
  inspectedCapabilityId: string,
  datasetId: string,
  agentPublicKeyHex: string,
  requestTimestampUnixSeconds: bigint,
  nowUnixSeconds: bigint,
): void {
  const expectedThumbprint = createHash("sha256")
    .update(Buffer.from(agentPublicKeyHex, "hex"))
    .digest("hex");
  if (
    claims.capabilityId.length === 0 ||
    claims.capabilityId !== inspectedCapabilityId ||
    claims.action !== "query" ||
    claims.resourceType !== "dataset" ||
    claims.resourceScope !== datasetId ||
    claims.agentKeyThumbprintHex !== expectedThumbprint ||
    claims.issuedAtUnixSeconds > requestTimestampUnixSeconds ||
    requestTimestampUnixSeconds >= claims.expiresAtUnixSeconds ||
    nowUnixSeconds < claims.issuedAtUnixSeconds ||
    nowUnixSeconds >= claims.expiresAtUnixSeconds ||
    claims.expiresAtUnixSeconds - nowUnixSeconds > BigInt(MAX_M5_CAPABILITY_VALIDITY_SECONDS)
  ) {
    throw new AuthorizationRejectedError();
  }
}

function systemNowUnixSeconds(): bigint {
  return BigInt(Math.floor(Date.now() / 1_000));
}
