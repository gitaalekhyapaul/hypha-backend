use std::fmt::{self, Display, Formatter};

use raziel_canonical::CanonicalError;
use raziel_snapshot::SnapshotDescriptorError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorityError {
    Canonical(CanonicalError),
    InvalidPkcs8,
    InvalidPublicKey,
    WeakPublicKey,
    InvalidKeyId,
    InvalidSignatureLength,
    InvalidSignature,
    WrongRole,
    UnsupportedFormatVersion,
    InvalidTrustDomain,
    InvalidValidityWindow,
    InactiveKey,
    RevokedKey,
    InvalidRevocation,
    InvalidRegistryGeneration,
    Snapshot(SnapshotDescriptorError),
    SnapshotEpochMismatch,
    SnapshotEpochOverflow,
    SnapshotEpochStateConflict,
    KeyFileNotRegular,
    KeyFileWritable,
    KeyFileRead,
}

impl Display for AuthorityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Canonical(_) => "authority canonical encoding failed",
            Self::InvalidPkcs8 => "authority private key is not valid PKCS#8 Ed25519 material",
            Self::InvalidPublicKey => "authority public key is invalid",
            Self::WeakPublicKey => "authority public key is weak",
            Self::InvalidKeyId => "authority key identifier does not match its public key",
            Self::InvalidSignatureLength => "authority signature has the wrong length",
            Self::InvalidSignature => "authority signature is invalid",
            Self::WrongRole => "authority key has the wrong role",
            Self::UnsupportedFormatVersion => "authority key format version is unsupported",
            Self::InvalidTrustDomain => "authority key trust domain is invalid",
            Self::InvalidValidityWindow => "authority key validity window is invalid",
            Self::InactiveKey => "authority key is not active at the trusted time",
            Self::RevokedKey => "authority key is revoked",
            Self::InvalidRevocation => "authority key revocation metadata is invalid",
            Self::InvalidRegistryGeneration => "authority key registry generation is invalid",
            Self::Snapshot(_) => "snapshot descriptor claims are invalid",
            Self::SnapshotEpochMismatch => {
                "snapshot expected previous epoch does not match trusted state"
            }
            Self::SnapshotEpochOverflow => "snapshot epoch overflowed",
            Self::SnapshotEpochStateConflict => {
                "snapshot epoch state contains a duplicate policy hash"
            }
            Self::KeyFileNotRegular => "authority private-key path is not a regular file",
            Self::KeyFileWritable => "authority private-key file is writable",
            Self::KeyFileRead => "authority private-key file could not be read",
        })
    }
}

impl std::error::Error for AuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonical(source) => Some(source),
            Self::Snapshot(source) => Some(source),
            _ => None,
        }
    }
}

impl From<CanonicalError> for AuthorityError {
    fn from(value: CanonicalError) -> Self {
        Self::Canonical(value)
    }
}

impl From<SnapshotDescriptorError> for AuthorityError {
    fn from(value: SnapshotDescriptorError) -> Self {
        Self::Snapshot(value)
    }
}
