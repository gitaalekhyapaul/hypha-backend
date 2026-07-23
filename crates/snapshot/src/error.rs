use std::fmt::{self, Display, Formatter};

use raziel_canonical::CanonicalError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipField {
    ResourceType,
    ResourceId,
    Relation,
    SubjectType,
    SubjectId,
    SubjectRelation,
    CaveatName,
    ContextName,
    ContextString,
}

impl Display for RelationshipField {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ResourceType => "resource type",
            Self::ResourceId => "resource identifier",
            Self::Relation => "relation",
            Self::SubjectType => "subject type",
            Self::SubjectId => "subject identifier",
            Self::SubjectRelation => "subject relation",
            Self::CaveatName => "caveat name",
            Self::ContextName => "stored context name",
            Self::ContextString => "stored context string",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyReference {
    ResourceType,
    Relation,
    SubjectType,
    SubjectRelation,
    Caveat,
}

impl Display for PolicyReference {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ResourceType => "resource type",
            Self::Relation => "relation",
            Self::SubjectType => "subject type",
            Self::SubjectRelation => "subject relation",
            Self::Caveat => "caveat",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotLimit {
    Relationships,
    RelationshipBytes,
    TotalCanonicalBytes,
    MerkleDepth,
    ContextEntries,
    StringBytes,
    RelationshipWitnesses,
    TotalPrivateInputBytes,
}

impl Display for SnapshotLimit {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Relationships => "relationship count",
            Self::RelationshipBytes => "canonical relationship bytes",
            Self::TotalCanonicalBytes => "total canonical relationship bytes",
            Self::MerkleDepth => "Merkle depth",
            Self::ContextEntries => "stored context entries",
            Self::StringBytes => "string bytes",
            Self::RelationshipWitnesses => "relationship witnesses",
            Self::TotalPrivateInputBytes => "total private input bytes",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    Canonical(CanonicalError),
    InvalidSchemaIdentifier(RelationshipField),
    InvalidObjectIdentifier(RelationshipField),
    MissingPolicyReference(PolicyReference),
    UnsupportedRelationshipShape,
    ContextNotStrictlySorted,
    ContextParameterMismatch,
    ContextTypeMismatch,
    RelationshipsNotStrictlySorted,
    DuplicateRelationshipIdentity,
    LimitExceeded(SnapshotLimit),
    ArithmeticOverflow,
    EmptySnapshotOpening,
    LeafIndexOutOfRange,
    SiblingCountMismatch,
    RootMismatch,
    UnsupportedWitnessVersion,
    EmptyWitnessSet,
    WitnessEntriesNotStrictlySorted,
}

impl Display for SnapshotError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => write!(formatter, "canonical encoding failed: {error}"),
            Self::InvalidSchemaIdentifier(field) => {
                write!(formatter, "{field} is not a supported schema identifier")
            }
            Self::InvalidObjectIdentifier(field) => {
                write!(formatter, "{field} is not a supported object identifier")
            }
            Self::MissingPolicyReference(reference) => {
                write!(
                    formatter,
                    "relationship references an unknown policy {reference}"
                )
            }
            Self::UnsupportedRelationshipShape => {
                formatter.write_str("relationship shape is not allowed by the approved policy")
            }
            Self::ContextNotStrictlySorted => {
                formatter.write_str("stored context entries are not strictly sorted")
            }
            Self::ContextParameterMismatch => {
                formatter.write_str("stored context does not match relationship parameters")
            }
            Self::ContextTypeMismatch => {
                formatter.write_str("stored context value has the wrong scalar type")
            }
            Self::RelationshipsNotStrictlySorted => {
                formatter.write_str("relationships are not in strict structural identity order")
            }
            Self::DuplicateRelationshipIdentity => {
                formatter.write_str("snapshot contains a duplicate relationship identity")
            }
            Self::LimitExceeded(limit) => write!(formatter, "snapshot exceeds the {limit} limit"),
            Self::ArithmeticOverflow => formatter.write_str("snapshot size calculation overflowed"),
            Self::EmptySnapshotOpening => {
                formatter.write_str("an empty snapshot has no membership opening")
            }
            Self::LeafIndexOutOfRange => {
                formatter.write_str("relationship leaf index is outside the committed range")
            }
            Self::SiblingCountMismatch => {
                formatter.write_str("membership opening has the wrong sibling count")
            }
            Self::RootMismatch => {
                formatter.write_str("membership opening does not match the snapshot root")
            }
            Self::UnsupportedWitnessVersion => {
                formatter.write_str("relationship witness set version is unsupported")
            }
            Self::EmptyWitnessSet => formatter.write_str("relationship witness set has no entries"),
            Self::WitnessEntriesNotStrictlySorted => {
                formatter.write_str("relationship witness entries are not strictly ordered")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<CanonicalError> for SnapshotError {
    fn from(value: CanonicalError) -> Self {
        Self::Canonical(value)
    }
}
