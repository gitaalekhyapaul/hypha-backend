use std::panic::{AssertUnwindSafe, catch_unwind};

#[cfg(feature = "proving")]
use sp1_sdk::{
    Elf, ProvingKey,
    blocking::{LightProver, Prover, ProverClient},
};
use sp1_verifier::{
    PLONK_VK_BYTES, PlonkVerifier, blake3_hash, decode_sp1_vkey_hash, hash_public_inputs,
    hash_public_inputs_with_fn,
};

#[cfg(feature = "proving")]
use crate::require_safe_sp1_environment;
use crate::{
    ProverError, TrustedProgramKey,
    artifact::{DecodedPlonkArtifact, decode_plonk_artifact},
};

pub struct Sp1Verifier {
    trusted_key: TrustedProgramKey,
}

impl Sp1Verifier {
    #[must_use]
    pub const fn from_trusted_key(trusted_key: TrustedProgramKey) -> Self {
        Self { trusted_key }
    }

    /// Compatibility constructor for proving builds and M3 proof tests.
    ///
    /// Service verification uses a verifier-registry [`TrustedProgramKey`] and never
    /// performs runtime ELF setup.
    #[cfg(feature = "proving")]
    pub fn from_trusted_elf(guest_elf: &[u8]) -> Result<Self, ProverError> {
        require_safe_sp1_environment()?;
        let key_loader: LightProver = ProverClient::builder().light().build();
        let proving_key = key_loader
            .setup(Elf::from(guest_elf))
            .map_err(|_| ProverError::ProverSetupFailed)?;
        let trusted_key = TrustedProgramKey::from_verifying_key(proving_key.verifying_key())?;
        Ok(Self::from_trusted_key(trusted_key))
    }

    #[must_use]
    pub fn verification_key_id(&self) -> &str {
        self.trusted_key.verification_key_id()
    }

    #[must_use]
    pub fn sp1_vkey_hash(&self) -> &str {
        self.trusted_key.sp1_vkey_hash()
    }

    /// `expected_public_values` must come from verifier-trusted capability claims.
    pub fn verify(
        &self,
        artifact: &[u8],
        expected_verification_key_id: &str,
        expected_public_values: &[u8],
    ) -> Result<Sp1VerificationReport, ProverError> {
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
        if self.trusted_key.verification_key_id() != expected_verification_key_id {
            return Err(ProverError::VerificationKeyMismatch);
        }

        let proof = decode_plonk_artifact(artifact)?;
        if proof.public_values != expected_public_values {
            return Err(ProverError::PublicValuesMismatch);
        }
        validate_public_inputs(
            &proof,
            self.trusted_key.sp1_vkey_hash(),
            expected_public_values,
        )?;
        PlonkVerifier::verify(
            &proof.proof_bytes,
            expected_public_values,
            self.trusted_key.sp1_vkey_hash(),
            &PLONK_VK_BYTES,
        )
        .map_err(|_| ProverError::ProofVerificationFailed)?;

        Ok(Sp1VerificationReport {
            verification_key_id: self.trusted_key.verification_key_id().to_owned(),
            public_values: proof.public_values,
        })
    }
}

fn validate_public_inputs(
    proof: &DecodedPlonkArtifact,
    trusted_sp1_vkey_hash: &str,
    expected_public_values: &[u8],
) -> Result<(), ProverError> {
    let expected_vkey = decode_sp1_vkey_hash(trusted_sp1_vkey_hash)
        .map_err(|_| ProverError::VerificationKeyInvalid)?;
    if decimal_to_bytes32(&proof.public_inputs[0])? != expected_vkey {
        return Err(ProverError::ProofVerificationFailed);
    }

    let public_values_hash = decimal_to_bytes32(&proof.public_inputs[1])?;
    if public_values_hash != hash_public_inputs(expected_public_values)
        && public_values_hash != hash_public_inputs_with_fn(expected_public_values, blake3_hash)
    {
        return Err(ProverError::ProofVerificationFailed);
    }

    let encoded_metadata = proof
        .proof_bytes
        .get(4..100)
        .ok_or(ProverError::ProofArtifactInvalid)?;
    for (public_input, expected) in proof.public_inputs[2..]
        .iter()
        .zip(encoded_metadata.chunks_exact(32))
    {
        if decimal_to_bytes32(public_input)?.as_slice() != expected {
            return Err(ProverError::ProofVerificationFailed);
        }
    }
    Ok(())
}

fn decimal_to_bytes32(value: &str) -> Result<[u8; 32], ProverError> {
    if value.is_empty() {
        return Err(ProverError::ProofArtifactInvalid);
    }
    let mut output = [0_u8; 32];
    for digit in value.bytes() {
        let mut carry = u16::from(
            digit
                .checked_sub(b'0')
                .filter(|digit| *digit <= 9)
                .ok_or(ProverError::ProofArtifactInvalid)?,
        );
        for byte in output.iter_mut().rev() {
            let next = u16::from(*byte) * 10 + carry;
            *byte = next as u8;
            carry = next >> 8;
        }
        if carry != 0 {
            return Err(ProverError::ProofArtifactInvalid);
        }
    }
    Ok(output)
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

    const KEY_ID: &str =
        "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
    const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn verification_panics_fail_closed() {
        let result = fail_closed_on_panic::<()>(|| panic!("malformed proof"));
        assert_eq!(result, Err(ProverError::ProofVerificationFailed));
    }

    #[test]
    fn registry_key_mismatch_fails_before_artifact_processing() {
        let key = TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap();
        let verifier = Sp1Verifier::from_trusted_key(key);
        assert_eq!(
            verifier.verify(&[], &KEY_ID.replace('e', "f"), &[]),
            Err(ProverError::VerificationKeyMismatch)
        );
    }

    #[test]
    fn decimal_public_inputs_use_canonical_big_endian_bytes() {
        assert_eq!(decimal_to_bytes32("0").unwrap(), [0; 32]);
        assert_eq!(decimal_to_bytes32("255").unwrap()[31], 255);
        assert_eq!(decimal_to_bytes32("256").unwrap()[30..], [1, 0]);
        assert_eq!(
            decimal_to_bytes32(
                "21888242871839275222246405745257275088548364400416034343698204186575808495616"
            )
            .unwrap(),
            [
                0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81,
                0x58, 0x5d, 0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93,
                0xf0, 0x00, 0x00, 0x00,
            ]
        );
        assert_eq!(
            decimal_to_bytes32(""),
            Err(ProverError::ProofArtifactInvalid)
        );
        assert_eq!(
            decimal_to_bytes32("not-a-number"),
            Err(ProverError::ProofArtifactInvalid)
        );
    }
}
