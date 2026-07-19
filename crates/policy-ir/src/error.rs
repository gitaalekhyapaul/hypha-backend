use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    ExpressionNodes,
    CaveatNodes,
    PathDepth,
    RelationshipWitnesses,
    MerkleDepth,
    StringBytes,
    ContextEntries,
    TotalPrivateInputBytes,
    SyntaxDepth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionKind {
    Definitions,
    Relations,
    AllowedSubjectTypes,
    Permissions,
    Caveats,
    CaveatParameters,
    PermissionChildren,
    CaveatChildren,
    Context,
    Relationships,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceKind {
    Object,
    Relation,
    Permission,
    Caveat,
    CaveatParameter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyError {
    UnsupportedVersion,
    MissingLimits,
    MissingNode,
    MissingExpression,
    MissingOperand,
    UnknownInputSource,
    UnknownValueType,
    InvalidIdentifier,
    InvalidMetadata,
    InvalidSourceSchemaDigest,
    LimitIsZero(LimitKind),
    LimitAboveCeiling(LimitKind),
    ProgramLimitExceeded(LimitKind),
    CollectionTooLarge(CollectionKind),
    NotStrictlySorted(CollectionKind),
    InvalidChildCount(CollectionKind),
    MissingReference(ReferenceKind),
    AmbiguousReference(ReferenceKind),
    UnsupportedRelationshipShape,
    CaveatTypeMismatch,
    CaveatRootNotBoolean,
    PermissionCycle,
    ArrowDepthExceeded,
}

impl Display for PolicyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "unsupported PolicyIR version",
            Self::MissingLimits => "PolicyIR limits are missing",
            Self::MissingNode => "PolicyIR node is missing or unknown",
            Self::MissingExpression => "PolicyIR expression is missing",
            Self::MissingOperand => "PolicyIR operand is missing",
            Self::UnknownInputSource => "unknown caveat input source",
            Self::UnknownValueType => "unknown caveat value type",
            Self::InvalidIdentifier => "invalid PolicyIR identifier",
            Self::InvalidMetadata => "invalid PolicyIR metadata",
            Self::InvalidSourceSchemaDigest => "invalid source schema digest",
            Self::LimitIsZero(_) => "PolicyIR limit is zero",
            Self::LimitAboveCeiling(_) => "PolicyIR limit exceeds trusted ceiling",
            Self::ProgramLimitExceeded(_) => "PolicyIR content exceeds its declared limit",
            Self::CollectionTooLarge(_) => "PolicyIR collection exceeds trusted ceiling",
            Self::NotStrictlySorted(_) => "PolicyIR collection is not strictly sorted",
            Self::InvalidChildCount(_) => "PolicyIR expression has an invalid child count",
            Self::MissingReference(_) => "PolicyIR reference does not resolve",
            Self::AmbiguousReference(_) => "PolicyIR reference is ambiguous",
            Self::UnsupportedRelationshipShape => "unsupported relationship shape",
            Self::CaveatTypeMismatch => "caveat expression type mismatch",
            Self::CaveatRootNotBoolean => "caveat root is not Boolean",
            Self::PermissionCycle => "permission reference cycle",
            Self::ArrowDepthExceeded => "permission path contains too many arrows",
        })
    }
}

impl std::error::Error for PolicyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolverError {
    Unavailable,
    InvalidData,
}

impl Display for ResolverError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "relationship resolver unavailable",
            Self::InvalidData => "relationship resolver returned invalid data",
        })
    }
}

impl std::error::Error for ResolverError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    InvalidRequest,
    MissingPermission,
    MissingCaveatInput,
    CaveatInputTypeMismatch,
    InvalidRelationshipData,
    BudgetExceeded(LimitKind),
    Resolver(ResolverError),
}

impl Display for EvaluationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "invalid authorization request",
            Self::MissingPermission => "authorization permission does not resolve",
            Self::MissingCaveatInput => "required caveat input is missing",
            Self::CaveatInputTypeMismatch => "caveat input has the wrong type",
            Self::InvalidRelationshipData => "relationship resolver returned invalid data",
            Self::BudgetExceeded(_) => "authorization evaluation exceeded a declared limit",
            Self::Resolver(_) => "relationship resolver failed",
        })
    }
}

impl std::error::Error for EvaluationError {}

impl From<ResolverError> for EvaluationError {
    fn from(value: ResolverError) -> Self {
        Self::Resolver(value)
    }
}
