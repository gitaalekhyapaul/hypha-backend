//! ADR-0020 agent API authentication bytes, separate from capability signatures.

use std::fmt::{self, Display, Formatter};

use crate::{
    CanonicalEncoder, ProtectedRequestCanonicalError, canonicalize_request_method,
    canonicalize_request_target, framed_message,
};

pub const AGENT_API_REQUEST_FORMAT_VERSION: u32 = 1;
pub const AGENT_API_REQUEST_SIGNATURE_DOMAIN: &[u8] = b"raziel/agent-api-request/v1";
pub const MAX_AGENT_API_AUDIENCE_BYTES: usize = 128;
pub const MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS: i64 = 253_402_300_799;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentApiCanonicalError {
    UnsupportedFormatVersion,
    InvalidAudience,
    InvalidTokenId,
    InvalidTimestamp,
    Request(ProtectedRequestCanonicalError),
    LengthOverflow,
}

impl Display for AgentApiCanonicalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormatVersion => formatter.write_str("unsupported agent API version"),
            Self::InvalidAudience => formatter.write_str("invalid agent API audience"),
            Self::InvalidTokenId => formatter.write_str("invalid agent API token identifier"),
            Self::InvalidTimestamp => formatter.write_str("invalid agent API timestamp"),
            Self::Request(error) => Display::fmt(error, formatter),
            Self::LengthOverflow => formatter.write_str("agent API canonical value is too large"),
        }
    }
}

impl std::error::Error for AgentApiCanonicalError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentApiRequestSignatureInputV1<'a> {
    pub format_version: u32,
    pub audience: &'a str,
    pub token_id: &'a str,
    pub agent_key_thumbprint: &'a [u8; 32],
    pub request_method: &'a str,
    pub canonical_request_target: &'a str,
    pub request_body_hash: &'a [u8; 32],
    pub request_nonce: &'a [u8; 32],
    pub request_timestamp_unix_seconds: i64,
}

/// Validates the exact supplied fields and returns the directly signed framed message.
/// This checks timestamp representation bounds, not freshness or token validity.
pub fn agent_api_request_signature_message(
    input: &AgentApiRequestSignatureInputV1<'_>,
) -> Result<Vec<u8>, AgentApiCanonicalError> {
    if input.format_version != AGENT_API_REQUEST_FORMAT_VERSION {
        return Err(AgentApiCanonicalError::UnsupportedFormatVersion);
    }
    if input.audience.is_empty()
        || input.audience.len() > MAX_AGENT_API_AUDIENCE_BYTES
        || !input
            .audience
            .bytes()
            .all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err(AgentApiCanonicalError::InvalidAudience);
    }
    if input.token_id.len() != 32
        || !input
            .token_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(AgentApiCanonicalError::InvalidTokenId);
    }
    let method = canonicalize_request_method(input.request_method)
        .map_err(AgentApiCanonicalError::Request)?;
    if method != input.request_method {
        return Err(AgentApiCanonicalError::Request(
            ProtectedRequestCanonicalError::NonCanonicalMethod,
        ));
    }
    let target = canonicalize_request_target(input.canonical_request_target)
        .map_err(AgentApiCanonicalError::Request)?;
    if target != input.canonical_request_target {
        return Err(AgentApiCanonicalError::Request(
            ProtectedRequestCanonicalError::NonCanonicalTarget,
        ));
    }
    if !(0..=MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS).contains(&input.request_timestamp_unix_seconds) {
        return Err(AgentApiCanonicalError::InvalidTimestamp);
    }

    let mut payload = CanonicalEncoder::new();
    payload.write_u32(input.format_version);
    for field in [
        input.audience.as_bytes(),
        input.token_id.as_bytes(),
        input.agent_key_thumbprint.as_slice(),
        input.request_method.as_bytes(),
        input.canonical_request_target.as_bytes(),
        input.request_body_hash.as_slice(),
        input.request_nonce.as_slice(),
    ] {
        payload
            .write_bytes(field)
            .map_err(|_| AgentApiCanonicalError::LengthOverflow)?;
    }
    payload.write_i64(input.request_timestamp_unix_seconds);
    framed_message(AGENT_API_REQUEST_SIGNATURE_DOMAIN, &payload.finish())
        .map_err(|_| AgentApiCanonicalError::LengthOverflow)
}
