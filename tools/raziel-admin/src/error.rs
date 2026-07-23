use std::fmt::{self, Display, Formatter};

use raziel_authority::AuthorityError;
use raziel_canonical::{CanonicalError, PolicyApprovalValidationError};
use raziel_policy_ir::PolicyIngressError;
use raziel_verifier::RegistryError;

#[derive(Debug)]
pub enum AdminError {
    Usage(&'static str),
    UnknownOption,
    MissingOption,
    DuplicateOption,
    InvalidNumber,
    FileRead,
    FileWrite,
    Authority(AuthorityError),
    Canonical(CanonicalError),
    PolicyApproval(PolicyApprovalValidationError),
    PolicyIngress(PolicyIngressError),
    Registry(RegistryError),
}

impl Display for AdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Usage(message) => message,
            Self::UnknownOption => "command contains an unknown option",
            Self::MissingOption => "command is missing a required option",
            Self::DuplicateOption => "command repeats a single-value option",
            Self::InvalidNumber => "command contains an invalid integer",
            Self::FileRead => "an input file could not be read",
            Self::FileWrite => "an output file could not be written atomically",
            Self::Authority(_) => "authority operation failed",
            Self::Canonical(_) => "canonical encoding failed",
            Self::PolicyApproval(_) => "policy approval claims are invalid",
            Self::PolicyIngress(_) => "PolicyIR input failed strict validation",
            Self::Registry(_) => "registry validation failed",
        })
    }
}

impl std::error::Error for AdminError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authority(source) => Some(source),
            Self::Canonical(source) => Some(source),
            Self::PolicyApproval(source) => Some(source),
            Self::PolicyIngress(source) => Some(source),
            Self::Registry(source) => Some(source),
            _ => None,
        }
    }
}

impl From<AuthorityError> for AdminError {
    fn from(value: AuthorityError) -> Self {
        Self::Authority(value)
    }
}

impl From<CanonicalError> for AdminError {
    fn from(value: CanonicalError) -> Self {
        Self::Canonical(value)
    }
}

impl From<PolicyApprovalValidationError> for AdminError {
    fn from(value: PolicyApprovalValidationError) -> Self {
        Self::PolicyApproval(value)
    }
}

impl From<PolicyIngressError> for AdminError {
    fn from(value: PolicyIngressError) -> Self {
        Self::PolicyIngress(value)
    }
}

impl From<RegistryError> for AdminError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}
