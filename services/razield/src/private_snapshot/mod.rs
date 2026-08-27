//! Owner-only persistence for the private data behind an accepted snapshot root.

mod codec;
mod store;
mod wire;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_snapshot::{Digest, RelationshipRecord, SnapshotTree};
use raziel_verifier::DEFAULT_MAX_REGISTRY_STRING_BYTES;

pub use store::PrivateSnapshotStore;

/// Version of the Rust-only private snapshot transport.
pub const PRIVATE_SNAPSHOT_FORMAT_VERSION: u32 = 1;

/// The largest serialized private snapshot file accepted by the prover boundary.
pub const MAX_PRIVATE_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

const SNAPSHOT_FILE_SUFFIX: &str = ".snapshot.pb";

/// Private relationship data that must reproduce one registry-approved root.
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateRelationshipSnapshot {
    snapshot_id: String,
    relationship_root: Digest,
    snapshot_secret_salt: Digest,
    relationships: Vec<RelationshipRecord>,
}

impl PrivateRelationshipSnapshot {
    /// Validates the complete private record against an approved policy and root.
    pub fn new(
        policy: &ValidatedPolicyProgram,
        snapshot_id: String,
        relationship_root: Digest,
        snapshot_secret_salt: Digest,
        relationships: Vec<RelationshipRecord>,
    ) -> Result<Self, PrivateSnapshotError> {
        if snapshot_id.is_empty()
            || snapshot_id.len() > DEFAULT_MAX_REGISTRY_STRING_BYTES as usize
            || !snapshot_id.is_ascii()
            || snapshot_id.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(PrivateSnapshotError::InvalidSnapshot);
        }
        let tree = SnapshotTree::build(
            policy,
            relationships.clone(),
            &snapshot_secret_salt,
            raziel_authorization_core::TRUSTED_SNAPSHOT_LIMITS,
        )
        .map_err(|_| PrivateSnapshotError::InvalidSnapshot)?;
        if tree.root() != relationship_root {
            return Err(PrivateSnapshotError::RootMismatch);
        }
        Ok(Self {
            snapshot_id,
            relationship_root,
            snapshot_secret_salt,
            relationships,
        })
    }

    pub const fn snapshot_id(&self) -> &str {
        self.snapshot_id.as_str()
    }

    pub const fn relationship_root(&self) -> &Digest {
        &self.relationship_root
    }

    #[cfg(feature = "capability-service")]
    pub fn into_selected(self) -> crate::input_assembly::SelectedPrivateSnapshot {
        crate::input_assembly::SelectedPrivateSnapshot::new(
            self.snapshot_id,
            self.relationships,
            self.snapshot_secret_salt,
        )
    }
}

impl Debug for PrivateRelationshipSnapshot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateRelationshipSnapshot")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Redacted failure classes for private snapshot persistence and binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateSnapshotError {
    UnsupportedSnapshotVersion,
    SnapshotTooLarge,
    MalformedSnapshot,
    InvalidSnapshot,
    SnapshotIdMismatch,
    RootMismatch,
    StoreRootUnavailable,
    StoreRootUnsafe,
    SnapshotAlreadyExists,
    SnapshotNotFound,
    TemporaryFileConflict,
    UnsafeSnapshotFile,
    StorageWrite,
    StorageRead,
    StoragePublish,
    StorageSync,
    CleanupFailed,
}

impl fmt::Display for PrivateSnapshotError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedSnapshotVersion => "unsupported private snapshot version",
            Self::SnapshotTooLarge => "private snapshot exceeds its byte limit",
            Self::MalformedSnapshot => "malformed private snapshot",
            Self::InvalidSnapshot => "invalid private snapshot",
            Self::SnapshotIdMismatch => "private snapshot identifier mismatch",
            Self::RootMismatch => "private snapshot root mismatch",
            Self::StoreRootUnavailable => "private snapshot store is unavailable",
            Self::StoreRootUnsafe => "private snapshot store is unsafe",
            Self::SnapshotAlreadyExists => "private snapshot already exists",
            Self::SnapshotNotFound => "private snapshot was not found",
            Self::TemporaryFileConflict => "private snapshot staging conflict",
            Self::UnsafeSnapshotFile => "private snapshot file is unsafe",
            Self::StorageWrite => "private snapshot write failed",
            Self::StorageRead => "private snapshot read failed",
            Self::StoragePublish => "private snapshot publication failed",
            Self::StorageSync => "private snapshot durability sync failed",
            Self::CleanupFailed => "private snapshot cleanup failed",
        })
    }
}

impl Error for PrivateSnapshotError {}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn snapshot_file_name(root: &Digest) -> String {
    format!("{}{SNAPSHOT_FILE_SUFFIX}", lowercase_hex(root))
}
