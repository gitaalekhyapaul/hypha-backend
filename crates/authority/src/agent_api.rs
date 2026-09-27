//! Strict agent API key and signature checks. No token, clock, or replay state.

use std::fmt::{self, Display, Formatter};

use ed25519_dalek::{Signature, VerifyingKey};
use raziel_canonical::{
    AgentApiCanonicalError, AgentApiRequestSignatureInputV1, agent_api_request_signature_message,
};

use crate::derive_ed25519_key_thumbprint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentApiVerificationError {
    InvalidPublicKeyLength,
    InvalidPublicKey,
    WeakPublicKey,
    KeyThumbprintMismatch,
    InvalidSignatureLength,
    InvalidSignature,
    Canonical(AgentApiCanonicalError),
}

impl Display for AgentApiVerificationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPublicKeyLength => formatter.write_str("invalid agent public-key length"),
            Self::InvalidPublicKey => formatter.write_str("invalid agent public key"),
            Self::WeakPublicKey => formatter.write_str("weak agent public key"),
            Self::KeyThumbprintMismatch => formatter.write_str("agent key thumbprint mismatch"),
            Self::InvalidSignatureLength => formatter.write_str("invalid agent signature length"),
            Self::InvalidSignature => formatter.write_str("invalid agent signature"),
            Self::Canonical(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for AgentApiVerificationError {}

/// Validates one raw Ed25519 key and derives its ADR-0014 thumbprint.
pub fn validate_agent_api_key(public_key: &[u8]) -> Result<[u8; 32], AgentApiVerificationError> {
    let key = parse_key(public_key)?;
    Ok(derive_ed25519_key_thumbprint(&key.to_bytes()))
}

/// Verifies the exact ADR-0020 message and returns the validated key's thumbprint.
/// Callers must separately check token binding, freshness, and atomic replay admission.
pub fn verify_agent_api_request(
    public_key: &[u8],
    input: &AgentApiRequestSignatureInputV1<'_>,
    signature: &[u8],
) -> Result<[u8; 32], AgentApiVerificationError> {
    let key = parse_key(public_key)?;
    let thumbprint = derive_ed25519_key_thumbprint(&key.to_bytes());
    if &thumbprint != input.agent_key_thumbprint {
        return Err(AgentApiVerificationError::KeyThumbprintMismatch);
    }
    let message =
        agent_api_request_signature_message(input).map_err(AgentApiVerificationError::Canonical)?;
    let signature: &[u8; 64] = signature
        .try_into()
        .map_err(|_| AgentApiVerificationError::InvalidSignatureLength)?;
    key.verify_strict(&message, &Signature::from_bytes(signature))
        .map_err(|_| AgentApiVerificationError::InvalidSignature)?;
    Ok(thumbprint)
}

fn parse_key(public_key: &[u8]) -> Result<VerifyingKey, AgentApiVerificationError> {
    let public_key: &[u8; 32] = public_key
        .try_into()
        .map_err(|_| AgentApiVerificationError::InvalidPublicKeyLength)?;
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| AgentApiVerificationError::InvalidPublicKey)?;
    // Dalek parsing accepts noncanonical compressed encodings; registration does not.
    if key.to_edwards().compress().to_bytes() != *public_key {
        return Err(AgentApiVerificationError::InvalidPublicKey);
    }
    if key.is_weak() {
        return Err(AgentApiVerificationError::WeakPublicKey);
    }
    Ok(key)
}
