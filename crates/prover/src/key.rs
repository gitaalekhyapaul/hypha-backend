use std::fmt::Write;

use raziel_canonical::{CanonicalEncoder, framed_hash};
use sp1_sdk::{HashableKey, SP1_CIRCUIT_VERSION, SP1VerifyingKey};

use crate::ProverError;

const VERIFICATION_KEY_ID_DOMAIN: &[u8] = b"raziel/sp1-verification-key-id/v1";
const VERIFICATION_KEY_ID_PREFIX: &str = "sp1-vk-v1:";

pub fn verification_key_id(key: &SP1VerifyingKey) -> Result<String, ProverError> {
    verification_key_id_from_digest(SP1_CIRCUIT_VERSION, &key.hash_bytes())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_identifier_is_domain_separated_and_stable() {
        assert_eq!(
            verification_key_id_from_digest("v6.1.0", &[0x11; 32]).unwrap(),
            "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9"
        );
    }

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
}
