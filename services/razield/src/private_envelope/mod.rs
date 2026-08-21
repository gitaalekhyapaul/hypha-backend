//! Private capability request transport between admission and the proof worker.

mod codec;
mod store;
mod wire;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use raziel_authorization_core::CapabilityRequestBindingV1;

pub use store::PrivateEnvelopeStore;

/// Version of the service-private Protobuf envelope.
pub const PRIVATE_ENVELOPE_FORMAT_VERSION: u32 = 1;

/// The largest serialized private request accepted by storage or the proof worker.
///
/// A maximum-size ADR-0011 request is below 11 KiB. The 16 KiB ceiling leaves room
/// for Protobuf framing without approaching the 2 MiB public service ingress limit.
pub const MAX_PRIVATE_ENVELOPE_BYTES: usize = 16 * 1024;

const REFERENCE_PREFIX: &str = "pe1_";
const REFERENCE_HEX_BYTES: usize = 64;

/// Opaque queue-safe reference to one private envelope.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PrivateEnvelopeRef {
    commitment: [u8; 32],
}

impl PrivateEnvelopeRef {
    /// Parses the exact token carried in the later BullMQ payload.
    pub fn from_queue_token(token: &str) -> Result<Self, PrivateEnvelopeError> {
        let encoded = token
            .strip_prefix(REFERENCE_PREFIX)
            .ok_or(PrivateEnvelopeError::InvalidReference)?;
        if encoded.len() != REFERENCE_HEX_BYTES {
            return Err(PrivateEnvelopeError::InvalidReference);
        }

        let mut commitment = [0_u8; 32];
        for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
            commitment[index] = (decode_hex_nibble(pair[0])? << 4) | decode_hex_nibble(pair[1])?;
        }
        Ok(Self { commitment })
    }

    /// Returns the bounded token stored in queue metadata, never a filesystem path.
    pub fn queue_token(&self) -> String {
        format!("{REFERENCE_PREFIX}{}", lowercase_hex(&self.commitment))
    }

    pub(super) const fn from_commitment(commitment: [u8; 32]) -> Self {
        Self { commitment }
    }

    pub(super) const fn commitment(&self) -> &[u8; 32] {
        &self.commitment
    }

    pub(super) fn file_name(&self) -> String {
        format!("{}.pb", self.queue_token())
    }
}

impl Debug for PrivateEnvelopeRef {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateEnvelopeRef")
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Validated private request material loaded for the proof worker.
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateCapabilityEnvelope {
    request: CapabilityRequestBindingV1,
    capability_request_commitment: [u8; 32],
}

impl PrivateCapabilityEnvelope {
    /// Returns the proof-critical request after strict storage validation.
    pub const fn request(&self) -> &CapabilityRequestBindingV1 {
        &self.request
    }

    /// Returns the recalculated and reference-bound ADR-0011 commitment.
    pub const fn capability_request_commitment(&self) -> &[u8; 32] {
        &self.capability_request_commitment
    }

    fn new(request: CapabilityRequestBindingV1, commitment: [u8; 32]) -> Self {
        Self {
            request,
            capability_request_commitment: commitment,
        }
    }
}

impl Debug for PrivateCapabilityEnvelope {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateCapabilityEnvelope")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Result of an idempotent envelope deletion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteStatus {
    Deleted,
    NotFound,
}

/// Counts from one bounded orphan-cleanup pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CleanupSummary {
    pub envelopes_deleted: usize,
    pub temporary_files_deleted: usize,
}

/// Redacted failure classes for private-envelope audit mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateEnvelopeError {
    InvalidReference,
    UnsupportedEnvelopeVersion,
    EnvelopeTooLarge,
    MalformedEnvelope,
    InvalidEnvelope,
    CommitmentMismatch,
    UnsupportedPlatform,
    StoreRootUnavailable,
    StoreRootUnsafe,
    EnvelopeAlreadyExists,
    EnvelopeNotFound,
    TemporaryFileConflict,
    UnsafeEnvelopeFile,
    StorageWrite,
    StorageRead,
    StoragePublish,
    StorageSync,
    StorageDelete,
    CleanupFailed,
}

impl fmt::Display for PrivateEnvelopeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidReference => "invalid private-envelope reference",
            Self::UnsupportedEnvelopeVersion => "unsupported private-envelope version",
            Self::EnvelopeTooLarge => "private envelope exceeds its byte limit",
            Self::MalformedEnvelope => "malformed private envelope",
            Self::InvalidEnvelope => "invalid private envelope",
            Self::CommitmentMismatch => "private-envelope commitment mismatch",
            Self::UnsupportedPlatform => "private-envelope storage is unsupported",
            Self::StoreRootUnavailable => "private-envelope store is unavailable",
            Self::StoreRootUnsafe => "private-envelope store is unsafe",
            Self::EnvelopeAlreadyExists => "private envelope already exists",
            Self::EnvelopeNotFound => "private envelope was not found",
            Self::TemporaryFileConflict => "private-envelope staging conflict",
            Self::UnsafeEnvelopeFile => "private-envelope file is unsafe",
            Self::StorageWrite => "private-envelope write failed",
            Self::StorageRead => "private-envelope read failed",
            Self::StoragePublish => "private-envelope publication failed",
            Self::StorageSync => "private-envelope durability sync failed",
            Self::StorageDelete => "private-envelope deletion failed",
            Self::CleanupFailed => "private-envelope cleanup failed",
        })
    }
}

impl Error for PrivateEnvelopeError {}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_hex_nibble(value: u8) -> Result<u8, PrivateEnvelopeError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(PrivateEnvelopeError::InvalidReference),
    }
}
