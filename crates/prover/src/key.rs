#[cfg(feature = "proving")]
use std::fmt::Write;

#[cfg(feature = "proving")]
use raziel_canonical::{CanonicalEncoder, framed_hash};
#[cfg(feature = "proving")]
use sp1_sdk::{HashableKey, SP1VerifyingKey};
use sp1_verifier::decode_sp1_vkey_hash;

use crate::ProverError;
#[cfg(feature = "proving")]
use crate::version::SP1_CIRCUIT_VERSION;

#[cfg(feature = "proving")]
const VERIFICATION_KEY_ID_DOMAIN: &[u8] = b"raziel/sp1-verification-key-id/v1";
const VERIFICATION_KEY_ID_PREFIX: &str = "sp1-vk-v1:";
const SP1_VKEY_HASH_PREFIX: &str = "0x";

/// The two public key representations stored as one verifier-registry record.
///
/// The registry derives both values from one approved SP1 verifying key. Request data
/// must never construct this record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedProgramKey {
    verification_key_id: String,
    sp1_vkey_hash: String,
}

impl TrustedProgramKey {
    pub fn from_registry_record(
        verification_key_id: &str,
        sp1_vkey_hash: &str,
    ) -> Result<Self, ProverError> {
        validate_verification_key_id(verification_key_id)?;
        validate_sp1_vkey_hash(sp1_vkey_hash)?;
        Ok(Self {
            verification_key_id: verification_key_id.to_owned(),
            sp1_vkey_hash: sp1_vkey_hash.to_owned(),
        })
    }

    #[cfg(feature = "proving")]
    pub(crate) fn from_verifying_key(key: &SP1VerifyingKey) -> Result<Self, ProverError> {
        Self::from_registry_record(&verification_key_id(key)?, &key.bytes32())
    }

    #[must_use]
    pub fn verification_key_id(&self) -> &str {
        &self.verification_key_id
    }

    #[must_use]
    pub fn sp1_vkey_hash(&self) -> &str {
        &self.sp1_vkey_hash
    }
}

#[cfg(feature = "proving")]
pub fn verification_key_id(key: &SP1VerifyingKey) -> Result<String, ProverError> {
    verification_key_id_from_digest(SP1_CIRCUIT_VERSION, &key.hash_bytes())
}

#[cfg(feature = "proving")]
fn verification_key_id_from_digest(
    circuit_version: &str,
    key_digest: &[u8; 32],
) -> Result<String, ProverError> {
    let mut encoder = CanonicalEncoder::new();
    encoder
        .write_str(circuit_version)
        .map_err(|_| ProverError::VerificationKeyInvalid)?;
    encoder
        .write_bytes(key_digest)
        .map_err(|_| ProverError::VerificationKeyInvalid)?;
    let digest = framed_hash(VERIFICATION_KEY_ID_DOMAIN, &encoder.finish())
        .map_err(|_| ProverError::VerificationKeyInvalid)?;

    let mut identifier = String::with_capacity(VERIFICATION_KEY_ID_PREFIX.len() + 64);
    identifier.push_str(VERIFICATION_KEY_ID_PREFIX);
    for byte in digest {
        write!(&mut identifier, "{byte:02x}").map_err(|_| ProverError::VerificationKeyInvalid)?;
    }
    Ok(identifier)
}

fn validate_verification_key_id(identifier: &str) -> Result<(), ProverError> {
    let Some(digest) = identifier.strip_prefix(VERIFICATION_KEY_ID_PREFIX) else {
        return Err(ProverError::VerificationKeyInvalid);
    };
    if digest.len() != 64 || !digest.bytes().all(is_lower_hex_digit) {
        return Err(ProverError::VerificationKeyInvalid);
    }
    Ok(())
}

fn validate_sp1_vkey_hash(hash: &str) -> Result<(), ProverError> {
    let Some(encoded) = hash.strip_prefix(SP1_VKEY_HASH_PREFIX) else {
        return Err(ProverError::VerificationKeyInvalid);
    };
    if encoded.len() != 64 || !encoded.bytes().all(is_lower_hex_digit) {
        return Err(ProverError::VerificationKeyInvalid);
    }
    decode_sp1_vkey_hash(hash).map_err(|_| ProverError::VerificationKeyInvalid)?;
    Ok(())
}

const fn is_lower_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_ID: &str =
        "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
    const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[cfg(feature = "proving")]
    #[test]
    fn key_identifier_is_domain_separated_and_stable() {
        assert_eq!(
            verification_key_id_from_digest("v6.1.0", &[0x11; 32]).unwrap(),
            KEY_ID
        );
    }

    #[cfg(feature = "proving")]
    #[test]
    fn key_identifier_changes_with_circuit_or_program_key() {
        let baseline = verification_key_id_from_digest("v6.1.0", &[0x11; 32]).unwrap();
        assert_ne!(
            baseline,
            verification_key_id_from_digest("v6.2.0", &[0x11; 32]).unwrap()
        );
        assert_ne!(
            baseline,
            verification_key_id_from_digest("v6.1.0", &[0x12; 32]).unwrap()
        );
    }

    #[test]
    fn registry_key_pair_requires_canonical_public_identifiers() {
        let key = TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap();
        assert_eq!(key.verification_key_id(), KEY_ID);
        assert_eq!(key.sp1_vkey_hash(), SP1_KEY_HASH);

        for invalid_id in [
            "sp1-vk-v2:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9",
            "sp1-vk-v1:E96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9",
            "sp1-vk-v1:00",
        ] {
            assert_eq!(
                TrustedProgramKey::from_registry_record(invalid_id, SP1_KEY_HASH),
                Err(ProverError::VerificationKeyInvalid)
            );
        }

        for invalid_hash in [
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
            "0x00112233445566778899AABBccddeeff00112233445566778899aabbccddeeff",
            "0x00",
        ] {
            assert_eq!(
                TrustedProgramKey::from_registry_record(KEY_ID, invalid_hash),
                Err(ProverError::VerificationKeyInvalid)
            );
        }
    }
}
