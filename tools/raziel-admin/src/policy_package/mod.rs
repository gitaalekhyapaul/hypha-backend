//! Durable owner-private policy packages. No schema publication or snapshot creation.
//!
//! The publication adapter must select a dedicated package root outside temporary
//! candidate cleanup. Registry approval is checked at retention and every approved
//! read. Retained files are never removed on expiry, revocation, or supersession.
//! Retained reads authenticate history but cannot grant new snapshot authorization.

mod store;
mod validation;
mod wire;

#[cfg(test)]
mod tests;

use std::fmt;

use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_proto::raziel::v1 as proto;

pub use store::PolicyPackageStore;

pub const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_COMPILER_CONFIG_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyVersionIdentity {
    family_id: String,
    version_id: String,
    version: u32,
}

impl PolicyVersionIdentity {
    pub fn new(family_id: String, version_id: String, version: u32) -> Result<Self, PackageError> {
        if !validation::immutable_id(&family_id)
            || !validation::immutable_id(&version_id)
            || version == 0
            || version > i32::MAX as u32
        {
            return Err(PackageError::InvalidIdentity);
        }
        Ok(Self {
            family_id,
            version_id,
            version,
        })
    }

    pub fn family_id(&self) -> &str {
        &self.family_id
    }
    pub fn version_id(&self) -> &str {
        &self.version_id
    }
    pub const fn version(&self) -> u32 {
        self.version
    }
    pub fn signed_policy_name(&self) -> String {
        format!("family:{}:v{}", self.family_id, self.version)
    }
    pub fn resource_type(&self) -> String {
        format!("dataset_{}", self.version_id)
    }
}

/// An approval is retained separately so reapproval never rewrites an older package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyPackageKey {
    version_id: String,
    approval_epoch: u64,
}

impl PolicyPackageKey {
    pub fn new(version_id: String, approval_epoch: u64) -> Result<Self, PackageError> {
        if !validation::immutable_id(&version_id) || approval_epoch == 0 {
            return Err(PackageError::InvalidIdentity);
        }
        Ok(Self {
            version_id,
            approval_epoch,
        })
    }
    pub fn version_id(&self) -> &str {
        &self.version_id
    }
    pub const fn approval_epoch(&self) -> u64 {
        self.approval_epoch
    }
}

/// Exact owner-selected candidate artifacts, not an agent-facing DTO.
pub struct PolicyPackageInput<'a> {
    pub identity: PolicyVersionIdentity,
    pub candidate_manifest: &'a [u8],
    pub source_schema: &'a [u8],
    pub compiler_config: &'a [u8],
    pub policy_pb: &'a [u8],
}

#[derive(Clone, PartialEq, Eq)]
pub struct PolicyInventory {
    definitions: Vec<String>,
    caveats: Vec<String>,
}

impl PolicyInventory {
    /// Includes the shared, empty `agent` definition.
    pub fn definitions(&self) -> &[String] {
        &self.definitions
    }
    pub fn caveats(&self) -> &[String] {
        &self.caveats
    }
}

/// Authenticated retained artifacts for schema composition, not current authorization.
/// Expiry, revocation, or a newer approval can make this package unusable for snapshots.
pub struct RetainedPolicyPackage {
    identity: PolicyVersionIdentity,
    key: PolicyPackageKey,
    inventory: PolicyInventory,
    policy: ValidatedPolicyProgram,
    record: proto::PolicyRecord,
    current_record: proto::PolicyRecord,
    stored: wire::StoredPackage,
}

impl RetainedPolicyPackage {
    pub fn identity(&self) -> &PolicyVersionIdentity {
        &self.identity
    }
    pub fn key(&self) -> &PolicyPackageKey {
        &self.key
    }
    pub fn inventory(&self) -> &PolicyInventory {
        &self.inventory
    }
    pub fn source_schema(&self) -> &[u8] {
        &self.stored.source_schema
    }
    pub fn compiler_config(&self) -> &[u8] {
        &self.stored.compiler_config
    }
    pub fn policy_pb(&self) -> &[u8] {
        &self.stored.policy_pb
    }
    pub fn candidate_manifest(&self) -> &[u8] {
        &self.stored.candidate_manifest
    }
    pub fn policy(&self) -> &ValidatedPolicyProgram {
        &self.policy
    }
    /// Original approval bytes decoded at retention, not the current mutable status.
    pub fn approval_record(&self) -> &proto::PolicyRecord {
        &self.record
    }
    /// Current authenticated status and generation, not a claim of current approval.
    pub fn registry_record(&self) -> &proto::PolicyRecord {
        &self.current_record
    }
    pub fn policy_hash(&self) -> &[u8] {
        &self.record.policy_hash
    }
}

impl fmt::Debug for RetainedPolicyPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetainedPolicyPackage")
            .field("key", &self.key)
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Approval was checked at the supplied trusted time, not for all future use.
pub struct ApprovedPolicyPackage {
    retained: RetainedPolicyPackage,
}

impl ApprovedPolicyPackage {
    pub fn identity(&self) -> &PolicyVersionIdentity {
        self.retained.identity()
    }
    pub fn key(&self) -> &PolicyPackageKey {
        self.retained.key()
    }
    pub fn inventory(&self) -> &PolicyInventory {
        self.retained.inventory()
    }
    pub fn source_schema(&self) -> &[u8] {
        self.retained.source_schema()
    }
    pub fn compiler_config(&self) -> &[u8] {
        self.retained.compiler_config()
    }
    pub fn policy_pb(&self) -> &[u8] {
        self.retained.policy_pb()
    }
    pub fn candidate_manifest(&self) -> &[u8] {
        self.retained.candidate_manifest()
    }
    pub fn policy(&self) -> &ValidatedPolicyProgram {
        self.retained.policy()
    }
    pub fn approval_record(&self) -> &proto::PolicyRecord {
        self.retained.approval_record()
    }
    pub fn policy_hash(&self) -> &[u8] {
        self.retained.policy_hash()
    }
}

impl fmt::Debug for ApprovedPolicyPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovedPolicyPackage")
            .field("key", self.key())
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageError {
    InvalidConfiguration,
    InvalidIdentity,
    InvalidPackage,
    NamespaceRejected,
    RegistryRejected,
    Conflict,
    NotFound,
    Unavailable,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration => "invalid policy package configuration",
            Self::InvalidIdentity => "invalid policy version identity",
            Self::InvalidPackage => "policy package was rejected",
            Self::NamespaceRejected => "policy package namespace was rejected",
            Self::RegistryRejected => "policy package approval was rejected",
            Self::Conflict => "immutable policy package conflicts with retained state",
            Self::NotFound => "policy package was not found",
            Self::Unavailable => "policy package storage is unavailable",
        })
    }
}

impl std::error::Error for PackageError {}
