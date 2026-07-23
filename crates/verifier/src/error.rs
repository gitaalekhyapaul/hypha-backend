use std::fmt::{self, Display, Formatter};

use raziel_authority::AuthorityError;
use raziel_canonical::{CanonicalError, PolicyApprovalValidationError};
use raziel_policy_ir::{PolicyError, PolicyIngressError};
use raziel_snapshot::SnapshotDescriptorError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    InputTooLarge,
    InvalidDescriptor,
    MissingMessageDescriptor,
    MalformedProtobuf,
    UnknownField,
    ExtensionField,
    NonCanonicalProtobuf,
    Authority(AuthorityError),
    Canonical(CanonicalError),
    PolicyApproval(PolicyApprovalValidationError),
    Policy(PolicyError),
    PolicyIngress(PolicyIngressError),
    Snapshot(SnapshotDescriptorError),
    UnsupportedFormatVersion,
    InvalidEnum,
    MissingField,
    InvalidTimestamp,
    InvalidIdentifier,
    StringLimitExceeded,
    InvalidHashLength,
    InvalidSignatureLength,
    TrustDomainMismatch,
    PolicyHashMismatch,
    PolicyRecordMismatch,
    InvalidValidityWindow,
    ArtifactLifetimeExceeded,
    InvalidEpoch,
    DuplicateRecord,
    EpochConflict,
    EpochGap,
    EpochRollback,
    EpochOverflow,
    UnknownAuthorityKey,
    WrongAuthorityRole,
    UnapprovedPolicy,
    UnknownPolicy,
    AmbiguousPolicy,
    UnknownRoot,
    RevokedPolicy,
    RevokedRoot,
    ExpiredPolicy,
    ExpiredRoot,
    RevocationRollback,
    ImmutableRecordChanged,
    RegistryGenerationInvalid,
    RegistryGenerationRollback,
    ClockRollback,
    FilePathInvalid,
    SymlinkForbidden,
    ManifestFileLimitExceeded,
    AggregateFileBytesExceeded,
    FileDigestMismatch,
    FileRead,
}

impl Display for RegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputTooLarge => "registry Protobuf exceeds its byte ceiling",
            Self::InvalidDescriptor => "frozen registry descriptor is invalid",
            Self::MissingMessageDescriptor => {
                "frozen registry descriptor does not contain the requested message"
            }
            Self::MalformedProtobuf => "registry Protobuf is malformed",
            Self::UnknownField => "registry Protobuf contains an unknown field",
            Self::ExtensionField => "registry Protobuf contains an extension field",
            Self::NonCanonicalProtobuf => "registry Protobuf uses a noncanonical wire spelling",
            Self::Authority(_) => "authority verification failed",
            Self::Canonical(_) => "canonical encoding failed",
            Self::PolicyApproval(_) => "policy approval claim validation failed",
            Self::Policy(_) => "policy validation failed",
            Self::PolicyIngress(_) => "strict PolicyIR ingress failed",
            Self::Snapshot(_) => "snapshot descriptor validation failed",
            Self::UnsupportedFormatVersion => "registry format version is unsupported",
            Self::InvalidEnum => "registry enum value is invalid",
            Self::MissingField => "registry record is missing a required field",
            Self::InvalidTimestamp => "registry timestamp is invalid",
            Self::InvalidIdentifier => "registry identifier is invalid",
            Self::StringLimitExceeded => "registry string exceeds its byte limit",
            Self::InvalidHashLength => "registry hash has the wrong length",
            Self::InvalidSignatureLength => "registry signature has the wrong length",
            Self::TrustDomainMismatch => "registry trust domain does not match",
            Self::PolicyHashMismatch => "policy hash does not match canonical PolicyIR",
            Self::PolicyRecordMismatch => "snapshot does not match its exact policy record",
            Self::InvalidValidityWindow => "registry validity window is invalid",
            Self::ArtifactLifetimeExceeded => "registry artifact lifetime exceeds its limit",
            Self::InvalidEpoch => "registry epoch is invalid",
            Self::DuplicateRecord => "registry contains a duplicate record",
            Self::EpochConflict => "registry contains conflicting records at one epoch",
            Self::EpochGap => "registry epoch sequence has a gap",
            Self::EpochRollback => "registry epoch decreased",
            Self::EpochOverflow => "registry epoch overflowed",
            Self::UnknownAuthorityKey => "registry references an unknown authority key",
            Self::WrongAuthorityRole => "registry references the wrong authority role",
            Self::UnapprovedPolicy => "policy is not approved",
            Self::UnknownPolicy => "policy record is unknown",
            Self::AmbiguousPolicy => "policy hash resolves to more than one policy name",
            Self::UnknownRoot => "relationship snapshot root is unknown",
            Self::RevokedPolicy => "policy record is revoked",
            Self::RevokedRoot => "relationship snapshot root is revoked",
            Self::ExpiredPolicy => "policy record is expired",
            Self::ExpiredRoot => "relationship snapshot descriptor is expired",
            Self::RevocationRollback => "registry attempted to undo a revocation",
            Self::ImmutableRecordChanged => "registry changed an immutable record",
            Self::RegistryGenerationInvalid => "registry generation is invalid",
            Self::RegistryGenerationRollback => "registry generation did not increase",
            Self::ClockRollback => "trusted time moved below the observed floor",
            Self::FilePathInvalid => "registry manifest path is invalid",
            Self::SymlinkForbidden => "registry manifest path traverses a symbolic link",
            Self::ManifestFileLimitExceeded => "registry manifest contains too many files",
            Self::AggregateFileBytesExceeded => "registry files exceed the aggregate byte limit",
            Self::FileDigestMismatch => "registry file digest does not match its manifest",
            Self::FileRead => "registry file could not be read",
        })
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authority(source) => Some(source),
            Self::Canonical(source) => Some(source),
            Self::PolicyApproval(source) => Some(source),
            Self::Policy(source) => Some(source),
            Self::PolicyIngress(source) => Some(source),
            Self::Snapshot(source) => Some(source),
            _ => None,
        }
    }
}

impl From<AuthorityError> for RegistryError {
    fn from(value: AuthorityError) -> Self {
        Self::Authority(value)
    }
}

impl From<CanonicalError> for RegistryError {
    fn from(value: CanonicalError) -> Self {
        Self::Canonical(value)
    }
}

impl From<PolicyApprovalValidationError> for RegistryError {
    fn from(value: PolicyApprovalValidationError) -> Self {
        Self::PolicyApproval(value)
    }
}

impl From<PolicyError> for RegistryError {
    fn from(value: PolicyError) -> Self {
        Self::Policy(value)
    }
}

impl From<PolicyIngressError> for RegistryError {
    fn from(value: PolicyIngressError) -> Self {
        Self::PolicyIngress(value)
    }
}

impl From<SnapshotDescriptorError> for RegistryError {
    fn from(value: SnapshotDescriptorError) -> Self {
        Self::Snapshot(value)
    }
}
