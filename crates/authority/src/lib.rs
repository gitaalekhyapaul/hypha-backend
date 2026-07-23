//! Host-only authority signing and strict Ed25519 verification.

#![forbid(unsafe_code)]

mod error;
mod key;
mod signer;
mod snapshot_publisher;

pub use error::AuthorityError;
pub use key::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    TrustedAuthorityKey, derive_key_id,
};
pub use signer::{AuthoritySigner, PolicyApprovalRole, RoleMarker, SnapshotRole};
pub use snapshot_publisher::{
    SignedSnapshotDescriptorV1, SnapshotAuthorityPublisher, SnapshotDescriptorDraftV1,
};
