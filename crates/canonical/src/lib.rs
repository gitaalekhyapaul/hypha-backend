//! Deterministic canonical encodings and domain-separated PolicyIR hashes.

#![forbid(unsafe_code)]

mod encoding;
mod policy;

use std::fmt::{self, Display, Formatter};

use raziel_policy_ir::ValidatedPolicyProgram;
use sha2::{Digest, Sha256};

use encoding::Encoder;

pub use policy::{CANONICAL_POLICY_ENCODING_VERSION, encode_policy};

pub const POLICY_HASH_DOMAIN: &[u8] = b"raziel/policy/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalError {
    LengthOverflow,
}

impl Display for CanonicalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LengthOverflow => "canonical value exceeds the u32 length limit",
        })
    }
}

impl std::error::Error for CanonicalError {}

pub fn policy_hash_preimage(program: &ValidatedPolicyProgram) -> Result<Vec<u8>, CanonicalError> {
    let payload = encode_policy(program)?;
    let mut encoder = Encoder::new();
    encoder.write_bytes(POLICY_HASH_DOMAIN)?;
    encoder.write_bytes(&payload)?;
    Ok(encoder.finish())
}

pub fn policy_hash(program: &ValidatedPolicyProgram) -> Result<[u8; 32], CanonicalError> {
    let preimage = policy_hash_preimage(program)?;
    Ok(Sha256::digest(preimage).into())
}
