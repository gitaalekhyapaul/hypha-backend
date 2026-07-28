use std::panic::{AssertUnwindSafe, catch_unwind};

use sp1_sdk::{
    Elf, ProvingKey, SP1ProvingKey, StatusCode,
    blocking::{LightProver, Prover, ProverClient},
};

use crate::{
    ProverError, decode_proof_artifact, require_safe_sp1_environment, verification_key_id,
};

pub struct Sp1Verifier {
    verifier: LightProver,
    trusted_key: SP1ProvingKey,
    trusted_key_id: String,
}

impl Sp1Verifier {
    pub fn from_trusted_elf(guest_elf: &[u8]) -> Result<Self, ProverError> {
        require_safe_sp1_environment()?;
        let verifier: LightProver = ProverClient::builder().light().build();
        let trusted_key = verifier
            .setup(Elf::from(guest_elf))
            .map_err(|_| ProverError::ProverSetupFailed)?;
        let trusted_key_id = verification_key_id(trusted_key.verifying_key())?;
        Ok(Self {
            verifier,
            trusted_key,
            trusted_key_id,
        })
    }

    #[must_use]
    pub fn verification_key_id(&self) -> &str {
        &self.trusted_key_id
    }

    /// `expected_public_values` must come from verifier-trusted capability claims.
    pub fn verify(
        &self,
        artifact: &[u8],
        expected_verification_key_id: &str,
        expected_public_values: &[u8],
    ) -> Result<Sp1VerificationReport, ProverError> {
        require_safe_sp1_environment()?;
        fail_closed_on_panic(|| {
            self.verify_inner(
                artifact,
                expected_verification_key_id,
                expected_public_values,
            )
        })
    }

    fn verify_inner(
        &self,
        artifact: &[u8],
        expected_verification_key_id: &str,
        expected_public_values: &[u8],
    ) -> Result<Sp1VerificationReport, ProverError> {
        let proof = decode_proof_artifact(artifact)?;
        if self.trusted_key_id != expected_verification_key_id {
            return Err(ProverError::VerificationKeyMismatch);
        }
        if proof.public_values.as_slice() != expected_public_values {
            return Err(ProverError::PublicValuesMismatch);
        }
        self.verifier
            .verify(
                &proof,
                self.trusted_key.verifying_key(),
                Some(StatusCode::SUCCESS),
            )
            .map_err(|_| ProverError::ProofVerificationFailed)?;

        Ok(Sp1VerificationReport {
            verification_key_id: self.trusted_key_id.clone(),
            public_values: proof.public_values.to_vec(),
        })
    }
}

fn fail_closed_on_panic<T>(
    operation: impl FnOnce() -> Result<T, ProverError>,
) -> Result<T, ProverError> {
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or(Err(ProverError::ProofVerificationFailed))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sp1VerificationReport {
    pub verification_key_id: String,
    pub public_values: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_panics_fail_closed() {
        let result = fail_closed_on_panic::<()>(|| panic!("malformed proof"));
        assert_eq!(result, Err(ProverError::ProofVerificationFailed));
    }
}
