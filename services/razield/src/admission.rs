//! Pure capability-request admission before private storage or queue publication.

use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use ed25519_dalek::{Signature, VerifyingKey};
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AuthorizationError, CapabilityRequestBindingV1,
    TRUSTED_POLICY_CEILINGS, capability_request_commitment, validate_capability_request_binding,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CreateCapabilityRequest, authorization_context_entry,
};

/// Proof deadline plus the minimum useful capability period accepted by ADR-0013.
pub const LOCAL_PLONK_ADMISSION_WINDOW_SECONDS: i64 = 75 * 60;

const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;
const ISSUANCE_NONCE_BYTES: usize = 32;
const MAX_PROTOBUF_TIMESTAMP_SECONDS: i64 = 253_402_300_799;

/// A request that is safe to pass to the later private-envelope boundary.
pub struct AdmittedCapabilityRequest {
    agent_id: String,
    request: CapabilityRequestBindingV1,
    capability_request_commitment: [u8; 32],
    job_id: String,
}

impl AdmittedCapabilityRequest {
    /// Returns untrusted presentation metadata. It is never an authorization identity.
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Returns the validated proof-critical request model.
    pub const fn request(&self) -> &CapabilityRequestBindingV1 {
        &self.request
    }

    /// Returns the digest verified by the agent issuance signature.
    pub const fn capability_request_commitment(&self) -> &[u8; 32] {
        &self.capability_request_commitment
    }

    /// Returns the deterministic BullMQ custom job ID from ADR-0014.
    pub fn job_id(&self) -> &str {
        &self.job_id
    }
}

impl Debug for AdmittedCapabilityRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedCapabilityRequest")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Typed rejection classes for private audit mapping at the later RPC boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    UnsupportedFormatVersion,
    InvalidAgentMetadata,
    InvalidPublicKeyLength,
    InvalidPublicKey,
    WeakPublicKey,
    MissingRequestedExpiry,
    InvalidRequestedExpiry,
    InsufficientValidityWindow,
    InvalidIssuanceNonce,
    InvalidRequest,
    InvalidContext,
    CanonicalEncoding,
    InvalidSignatureLength,
    InvalidSignature,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedFormatVersion => "unsupported capability request version",
            Self::InvalidAgentMetadata => "invalid agent presentation metadata",
            Self::InvalidPublicKeyLength => "invalid agent public-key length",
            Self::InvalidPublicKey => "invalid agent public key",
            Self::WeakPublicKey => "weak agent public key",
            Self::MissingRequestedExpiry => "missing requested expiry",
            Self::InvalidRequestedExpiry => "invalid requested expiry",
            Self::InsufficientValidityWindow => "requested expiry window is too short",
            Self::InvalidIssuanceNonce => "invalid issuance nonce",
            Self::InvalidRequest => "invalid capability request",
            Self::InvalidContext => "invalid authorization context",
            Self::CanonicalEncoding => "capability request encoding failed",
            Self::InvalidSignatureLength => "invalid request-signature length",
            Self::InvalidSignature => "invalid request signature",
        })
    }
}

impl Error for AdmissionError {}

/// Validates and authenticates one request without storage, queue, or network access.
pub fn admit_create_capability_request(
    transport: CreateCapabilityRequest,
    trusted_now_unix_seconds: i64,
) -> Result<AdmittedCapabilityRequest, AdmissionError> {
    let CreateCapabilityRequest {
        format_version,
        agent_id,
        agent_public_key,
        task_id,
        purpose,
        action,
        resource_type,
        resource_scope,
        requested_expires_at,
        issuance_nonce,
        snapshot_id,
        request_signature,
        request_context,
    } = transport;

    if format_version != AUTHORIZATION_GUEST_INPUT_VERSION {
        return Err(AdmissionError::UnsupportedFormatVersion);
    }
    validate_agent_metadata(&agent_id)?;

    let public_key_bytes: [u8; ED25519_PUBLIC_KEY_BYTES] =
        agent_public_key
            .as_slice()
            .try_into()
            .map_err(|_| AdmissionError::InvalidPublicKeyLength)?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| AdmissionError::InvalidPublicKey)?;
    if verifying_key.is_weak() {
        return Err(AdmissionError::WeakPublicKey);
    }
    let canonical_public_key = verifying_key.to_bytes();
    let agent_key_thumbprint = derive_ed25519_key_thumbprint(&canonical_public_key);

    let requested_expires_at_unix_seconds =
        parse_requested_expiry(requested_expires_at, trusted_now_unix_seconds)?;
    let issuance_nonce: [u8; ISSUANCE_NONCE_BYTES] = issuance_nonce
        .as_slice()
        .try_into()
        .map_err(|_| AdmissionError::InvalidIssuanceNonce)?;
    if request_context.len() > TRUSTED_POLICY_CEILINGS.policy_limits.max_context_entries as usize {
        return Err(AdmissionError::InvalidContext);
    }
    let request_context = request_context
        .into_iter()
        .map(convert_context_entry)
        .collect::<Result<Vec<_>, _>>()?;

    let request = CapabilityRequestBindingV1 {
        format_version,
        agent_key_thumbprint,
        task_id,
        purpose,
        action,
        resource_type,
        resource_scope,
        requested_expires_at_unix_seconds,
        issuance_nonce,
        snapshot_id,
        request_context,
    };
    validate_capability_request_binding(&request).map_err(map_authorization_error)?;

    let commitment = capability_request_commitment(&request).map_err(map_authorization_error)?;
    let signature_bytes: &[u8; ED25519_SIGNATURE_BYTES] =
        request_signature
            .as_slice()
            .try_into()
            .map_err(|_| AdmissionError::InvalidSignatureLength)?;
    let signature = Signature::from_bytes(signature_bytes);
    verifying_key
        .verify_strict(&commitment, &signature)
        .map_err(|_| AdmissionError::InvalidSignature)?;

    Ok(AdmittedCapabilityRequest {
        agent_id,
        request,
        capability_request_commitment: commitment,
        job_id: lowercase_hex(&commitment),
    })
}

fn validate_agent_metadata(agent_id: &str) -> Result<(), AdmissionError> {
    let max_bytes = TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize;
    if agent_id.trim().is_empty()
        || agent_id.len() > max_bytes
        || agent_id.chars().any(char::is_control)
    {
        return Err(AdmissionError::InvalidAgentMetadata);
    }
    Ok(())
}

fn parse_requested_expiry(
    timestamp: Option<prost_types::Timestamp>,
    trusted_now_unix_seconds: i64,
) -> Result<i64, AdmissionError> {
    let timestamp = timestamp.ok_or(AdmissionError::MissingRequestedExpiry)?;
    if !(0..=MAX_PROTOBUF_TIMESTAMP_SECONDS).contains(&trusted_now_unix_seconds)
        || !(0..=MAX_PROTOBUF_TIMESTAMP_SECONDS).contains(&timestamp.seconds)
        || timestamp.nanos != 0
    {
        return Err(AdmissionError::InvalidRequestedExpiry);
    }
    let minimum_expiry = trusted_now_unix_seconds
        .checked_add(LOCAL_PLONK_ADMISSION_WINDOW_SECONDS)
        .ok_or(AdmissionError::InvalidRequestedExpiry)?;
    if timestamp.seconds < minimum_expiry {
        return Err(AdmissionError::InsufficientValidityWindow);
    }
    Ok(timestamp.seconds)
}

fn convert_context_entry(entry: AuthorizationContextEntry) -> Result<ContextEntry, AdmissionError> {
    let value = match entry.value.ok_or(AdmissionError::InvalidContext)? {
        authorization_context_entry::Value::BoolValue(value) => CaveatValue::Bool(value),
        authorization_context_entry::Value::IntValue(value) => CaveatValue::Int(value),
        authorization_context_entry::Value::StringValue(value) => CaveatValue::String(value),
    };
    Ok(ContextEntry {
        name: entry.name,
        value,
    })
}

const fn map_authorization_error(error: AuthorizationError) -> AdmissionError {
    match error {
        AuthorizationError::UnsupportedRequestVersion => AdmissionError::UnsupportedFormatVersion,
        AuthorizationError::InvalidContext => AdmissionError::InvalidContext,
        AuthorizationError::CanonicalEncoding => AdmissionError::CanonicalEncoding,
        AuthorizationError::UnsupportedInputVersion
        | AuthorizationError::InvalidRequest
        | AuthorizationError::InvalidTimestamp
        | AuthorizationError::InvalidPolicy
        | AuthorizationError::PolicyTooLarge
        | AuthorizationError::PolicyHashMismatch
        | AuthorizationError::InvalidWitness
        | AuthorizationError::EvaluationFailed
        | AuthorizationError::AuthorizationDenied
        | AuthorizationError::InvalidWitnessUsage
        | AuthorizationError::InvalidPublicValues => AdmissionError::InvalidRequest,
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
