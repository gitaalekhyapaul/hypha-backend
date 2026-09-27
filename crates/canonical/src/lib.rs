//! Deterministic canonical encodings and domain-separated PolicyIR hashes.

#![forbid(unsafe_code)]

mod agent_api;
mod authority;
mod encoding;
mod policy;
mod protected_request;
mod relationship;

use std::fmt::{self, Display, Formatter};

use raziel_policy_ir::ValidatedPolicyProgram;
use sha2::{Digest, Sha256};

pub use agent_api::{
    AGENT_API_REQUEST_FORMAT_VERSION, AGENT_API_REQUEST_SIGNATURE_DOMAIN, AgentApiCanonicalError,
    AgentApiRequestSignatureInputV1, MAX_AGENT_API_AUDIENCE_BYTES,
    MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS, agent_api_request_signature_message,
};
pub use encoding::CanonicalEncoder;
pub use policy::{CANONICAL_POLICY_ENCODING_VERSION, encode_policy};
pub use protected_request::{
    MAX_CANONICAL_REQUEST_TARGET_BYTES, PROTECTED_REQUEST_FORMAT_VERSION,
    PROTECTED_REQUEST_SIGNATURE_DOMAIN, ProtectedRequestCanonicalError,
    ProtectedRequestSignatureInputV1, canonicalize_request_method, canonicalize_request_target,
    protected_request_signature_message,
};
pub use relationship::{
    CANONICAL_RELATIONSHIP_ENCODING_VERSION, RelationshipRecord, StoredCaveat, StoredContextEntry,
    compare_relationship_identity, encode_relationship,
};

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
    framed_hash_preimage(POLICY_HASH_DOMAIN, &payload)
}

pub fn policy_hash(program: &ValidatedPolicyProgram) -> Result<[u8; 32], CanonicalError> {
    let payload = encode_policy(program)?;
    framed_hash(POLICY_HASH_DOMAIN, &payload)
}

pub fn framed_hash_preimage(domain: &[u8], payload: &[u8]) -> Result<Vec<u8>, CanonicalError> {
    framed_message(domain, payload)
}

pub fn framed_message(domain: &[u8], payload: &[u8]) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_bytes(domain)?;
    encoder.write_bytes(payload)?;
    Ok(encoder.finish())
}

pub fn framed_hash(domain: &[u8], payload: &[u8]) -> Result<[u8; 32], CanonicalError> {
    let preimage = framed_hash_preimage(domain, payload)?;
    Ok(Sha256::digest(preimage).into())
}
pub use authority::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, POLICY_APPROVAL_SIGNATURE_DOMAIN, PolicyApprovalClaimsV1,
    PolicyApprovalLimits, PolicyApprovalValidationError, ValidatedPolicyApprovalClaimsV1,
    encode_policy_approval_claims, policy_approval_signature_message,
};
