use std::fmt::{self, Debug, Formatter};

use raziel_proto::raziel::v1::VerifiedClaims;
use raziel_prover::TrustedProgramKey;

/// Minimal public input passed to the later isolated verifier worker.
#[derive(Clone, PartialEq, Eq)]
pub struct VerificationWorkerRequest {
    proof_artifact: Vec<u8>,
    expected_public_values: Vec<u8>,
    trusted_program_key: TrustedProgramKey,
}

impl VerificationWorkerRequest {
    pub(crate) const fn new(
        proof_artifact: Vec<u8>,
        expected_public_values: Vec<u8>,
        trusted_program_key: TrustedProgramKey,
    ) -> Self {
        Self {
            proof_artifact,
            expected_public_values,
            trusted_program_key,
        }
    }

    #[must_use]
    pub fn proof_artifact(&self) -> &[u8] {
        &self.proof_artifact
    }

    #[must_use]
    pub fn expected_public_values(&self) -> &[u8] {
        &self.expected_public_values
    }

    #[must_use]
    pub const fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.trusted_program_key
    }
}

impl Debug for VerificationWorkerRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerificationWorkerRequest")
            .field("public_payload", &"<redacted>")
            .finish()
    }
}

/// Public claims and proof input that passed admission but not proof verification.
#[derive(Clone, PartialEq, Eq)]
pub struct AdmittedVerificationRequest {
    worker_request: VerificationWorkerRequest,
    candidate_claims: VerifiedClaims,
    policy_record_id: String,
    snapshot_id: String,
    proof_digest: [u8; 32],
}

impl AdmittedVerificationRequest {
    pub(crate) const fn new(
        worker_request: VerificationWorkerRequest,
        candidate_claims: VerifiedClaims,
        policy_record_id: String,
        snapshot_id: String,
        proof_digest: [u8; 32],
    ) -> Self {
        Self {
            worker_request,
            candidate_claims,
            policy_record_id,
            snapshot_id,
            proof_digest,
        }
    }

    #[must_use]
    pub const fn worker_request(&self) -> &VerificationWorkerRequest {
        &self.worker_request
    }

    /// Claims remain candidates until the isolated worker verifies the proof.
    #[must_use]
    pub const fn candidate_claims(&self) -> &VerifiedClaims {
        &self.candidate_claims
    }

    #[must_use]
    pub fn policy_record_id(&self) -> &str {
        &self.policy_record_id
    }

    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    #[must_use]
    pub const fn proof_digest(&self) -> &[u8; 32] {
        &self.proof_digest
    }
}

impl Debug for AdmittedVerificationRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedVerificationRequest")
            .field("public_fields", &"<redacted>")
            .finish()
    }
}
