import { createHash } from "node:crypto";
import { ReconciliationValidationError } from "../reconciliation/model";
import { DatasetNotFoundError } from "../reconciliation/store";
import { AuthorizationRejectedError, ProtectedQueryInfrastructureError } from "./errors";
import type {
  ProtectedQueryDependencies,
  ProtectedQueryResult,
  PublicReceipt,
  VerifiedCapability,
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

    let inspection: Awaited<
      ReturnType<ProtectedQueryDependencies["verifier"]["inspectCapability"]>
    >;
    try {
      inspection = await this.#dependencies.verifier.inspectCapability(envelope);
    } catch (error) {
      if (error instanceof AuthorizationRejectedError) {
        throw error;
      }
      throw new ProtectedQueryInfrastructureError();
    }

    let claimed: boolean;
    try {
      claimed = await this.#dependencies.nonceStore.claim(
        inspection.capabilityId,
        envelope.requestNonceHex,
        MAX_M5_CAPABILITY_VALIDITY_SECONDS,
      );
    } catch {
      throw new ProtectedQueryInfrastructureError();
    }
    if (!claimed) {
      await this.#recordTrustedReplayReceipt(
        inspection.capabilityId,
        inspection.proofDigestHex,
        datasetId,
        envelope,
        nowUnixSeconds,
      );
      throw new AuthorizationRejectedError();
    }

    let verification: VerifiedCapability | undefined;
    try {
      verification = await this.#dependencies.verificationCache.get(
        inspection.capabilityId,
        inspection.proofDigestHex,
      );
    } catch {
      throw new ProtectedQueryInfrastructureError();
    }

    let verificationPath: ProtectedQueryResult["verification_path"];
    if (verification) {
      let signatureValid: boolean;
      try {
        signatureValid = await this.#dependencies.signatureVerifier.verify(
          inspection.capabilityId,
          envelope,
        );
      } catch {
        throw new ProtectedQueryInfrastructureError();
      }
      if (!signatureValid) {
        throw new AuthorizationRejectedError();
      }
      verificationPath = "CACHE";
    } else {
      let verifierResult: Awaited<ReturnType<ProtectedQueryDependencies["verifier"]["verify"]>>;
      try {
        verifierResult = await this.#dependencies.verifier.verify(envelope);
      } catch (error) {
        if (error instanceof AuthorizationRejectedError) {
          throw error;
        }
        throw new ProtectedQueryInfrastructureError();
      }
      if (verifierResult.outcome !== "verified") {
        throw new AuthorizationRejectedError();
      }
      if (verifierResult.proofDigestHex !== inspection.proofDigestHex) {
        throw new ProtectedQueryInfrastructureError();
      }
      verification = verifierResult;
      try {
        await this.#dependencies.receiptStore.append(
          createCapabilityVerificationReceipt({
            claims: verification.claims,
            policyRecordId: verification.policyRecordId,
            proofDigestHex: verification.proofDigestHex,
            requestMethod: envelope.requestMethod,
            canonicalRequestTarget: envelope.canonicalRequestTarget,
            requestBodyHashHex: envelope.requestBodyHashHex,
            requestNonceHex: envelope.requestNonceHex,
            requestTimestampUnixSeconds: envelope.requestTimestampUnixSeconds,
            occurredAtUnixSeconds: nowUnixSeconds,
          }),
        );
      } catch {
        throw new ProtectedQueryInfrastructureError();
      }
      verificationPath = "VERIFIER";
    }

    const receiptInput: ReceiptInput = {
      claims: verification.claims,
      policyRecordId: verification.policyRecordId,
      proofDigestHex: verification.proofDigestHex,
      requestMethod: envelope.requestMethod,
      canonicalRequestTarget: envelope.canonicalRequestTarget,
      requestBodyHashHex: envelope.requestBodyHashHex,
      requestNonceHex: envelope.requestNonceHex,
      requestTimestampUnixSeconds: envelope.requestTimestampUnixSeconds,
      occurredAtUnixSeconds: nowUnixSeconds,
    };
    try {
      validateVerifiedClaims(
        verification.claims,
        inspection.capabilityId,
        datasetId,
        envelope.agentPublicKeyHex,
        envelope.requestTimestampUnixSeconds,
        nowUnixSeconds,
      );
    } catch (error) {
      if (!(error instanceof AuthorizationRejectedError)) {
        throw error;
      }
      const deniedReceipt = createPublicReceipt({
        ...receiptInput,
        outcome: "AUDIT_OUTCOME_REJECTED",
        reasonCode: "PUBLIC_ERROR_CODE_NOT_AUTHORIZED",
      });
      try {
        await this.#dependencies.receiptStore.append(deniedReceipt);
      } catch {
        throw new ProtectedQueryInfrastructureError();
      }
      throw error;
    }

    if (verificationPath === "VERIFIER") {
      const cacheTtlSeconds = Number(verification.claims.expiresAtUnixSeconds - nowUnixSeconds);
      try {
        await this.#dependencies.verificationCache.put(verification, cacheTtlSeconds);
      } catch {
        throw new ProtectedQueryInfrastructureError();
      }
    }

    try {
      const rows = await this.#dependencies.queryDataset(datasetId, envelope.query);
      const receipt = createPublicReceipt(receiptInput);
      try {
        await this.#dependencies.receiptStore.append(receipt);
      } catch {
        throw new ProtectedQueryInfrastructureError();
      }
      return {
        classification: "SYNTHETIC",
        dataset_id: datasetId,
        verification_path: verificationPath,
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

  async #recordTrustedReplayReceipt(
    capabilityId: string,
    proofDigestHex: string,
    datasetId: string,
    envelope: ReturnType<typeof parseProtectedRequestEnvelope>,
    nowUnixSeconds: bigint,
  ): Promise<void> {
    let verification: VerifiedCapability | undefined;
    try {
      verification = await this.#dependencies.verificationCache.get(capabilityId, proofDigestHex);
    } catch {
      throw new ProtectedQueryInfrastructureError();
    }
    if (!verification) {
      return;
    }

    let signatureValid: boolean;
    try {
      signatureValid = await this.#dependencies.signatureVerifier.verify(capabilityId, envelope);
    } catch {
      throw new ProtectedQueryInfrastructureError();
    }
    if (!signatureValid) {
      return;
    }
    try {
      validateVerifiedClaims(
        verification.claims,
        capabilityId,
        datasetId,
        envelope.agentPublicKeyHex,
        envelope.requestTimestampUnixSeconds,
        nowUnixSeconds,
      );
    } catch (error) {
      if (error instanceof AuthorizationRejectedError) {
        return;
      }
      throw error;
    }

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
      outcome: "AUDIT_OUTCOME_REJECTED",
      reasonCode: "PUBLIC_ERROR_CODE_REQUEST_REPLAYED",
    });
    try {
      await this.#dependencies.receiptStore.append(receipt);
    } catch {
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
  readonly outcome?: PublicReceipt["outcome"];
  readonly reasonCode?: PublicReceipt["reason_code"];
}

export function createPublicReceipt(input: ReceiptInput): PublicReceipt {
  const outcome = input.outcome ?? "AUDIT_OUTCOME_ACCEPTED";
  const reasonCode = input.reasonCode ?? "PUBLIC_ERROR_CODE_UNSPECIFIED";
  const requestId = protectedRequestId(input);
  const receiptId = digestFields("raziel/audit-receipt-id/v1", [
    requestId,
    input.proofDigestHex,
    outcome,
  ]);
  return projectReceipt({
    claims: input.claims,
    occurredAtUnixSeconds: input.occurredAtUnixSeconds,
    outcome,
    policyRecordId: input.policyRecordId,
    proofDigestHex: input.proofDigestHex,
    reasonCode,
    receiptId: `receipt-v1:${receiptId}`,
    receiptType: "AUDIT_RECEIPT_TYPE_PROTECTED_REQUEST",
    requestId: `request-v1:${requestId}`,
  });
}

interface CapabilityVerificationReceiptInput {
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

export function createCapabilityVerificationReceipt(
  input: CapabilityVerificationReceiptInput,
): PublicReceipt {
  const requestId = `request-v1:${protectedRequestId(input)}`;
  const receiptId = digestFields("raziel/audit-receipt-id/v1", [
    requestId,
    input.proofDigestHex,
    "AUDIT_RECEIPT_TYPE_CAPABILITY_VERIFICATION",
    "AUDIT_OUTCOME_ACCEPTED",
  ]);
  return projectReceipt({
    claims: input.claims,
    occurredAtUnixSeconds: input.occurredAtUnixSeconds,
    outcome: "AUDIT_OUTCOME_ACCEPTED",
    policyRecordId: input.policyRecordId,
    proofDigestHex: input.proofDigestHex,
    reasonCode: "PUBLIC_ERROR_CODE_UNSPECIFIED",
    receiptId: `receipt-v1:${receiptId}`,
    receiptType: "AUDIT_RECEIPT_TYPE_CAPABILITY_VERIFICATION",
    requestId,
  });
}

interface ProtectedRequestIdentityInput {
  readonly claims: VerifiedCapabilityClaims;
  readonly requestMethod: string;
  readonly canonicalRequestTarget: string;
  readonly requestBodyHashHex: string;
  readonly requestNonceHex: string;
  readonly requestTimestampUnixSeconds: bigint;
}

function protectedRequestId(input: ProtectedRequestIdentityInput): string {
  return digestFields("raziel/protected-request-id/v1", [
    input.claims.capabilityId,
    input.requestMethod,
    input.canonicalRequestTarget,
    input.requestBodyHashHex,
    input.requestNonceHex,
    input.requestTimestampUnixSeconds.toString(),
  ]);
}

interface ReceiptProjectionInput {
  readonly claims: VerifiedCapabilityClaims;
  readonly occurredAtUnixSeconds: bigint;
  readonly outcome: PublicReceipt["outcome"];
  readonly policyRecordId: string;
  readonly proofDigestHex: string;
  readonly reasonCode: PublicReceipt["reason_code"];
  readonly receiptId: string;
  readonly receiptType: PublicReceipt["receipt_type"];
  readonly requestId: string;
}

function projectReceipt(input: ReceiptProjectionInput): PublicReceipt {
  return {
    format_version: 1,
    receipt_id: input.receiptId,
    receipt_type: input.receiptType,
    occurred_at: wholeSecondUtc(input.occurredAtUnixSeconds),
    request_id: input.requestId,
    task_id: input.claims.taskId,
    capability_id: input.claims.capabilityId,
    proof_job_id: input.claims.capabilityId,
    policy_record_id: input.policyRecordId,
    policy_hash: input.claims.policyHashHex,
    snapshot_id: input.claims.snapshotId,
    relationship_snapshot_root: input.claims.relationshipSnapshotRootHex,
    public_claims_digest: publicClaimsDigest(input.claims),
    proof_digest: input.proofDigestHex,
    action: input.claims.action,
    resource_type: input.claims.resourceType,
    resource_scope: input.claims.resourceScope,
    outcome: input.outcome,
    reason_code: input.reasonCode,
  };
}

function publicClaimsDigest(claims: VerifiedCapabilityClaims): string {
  return createHash("sha256")
    .update(
      JSON.stringify({
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
      }),
      "utf8",
    )
    .digest("hex");
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
