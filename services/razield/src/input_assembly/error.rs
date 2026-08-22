use std::error::Error;
use std::fmt::{self, Display, Formatter};

use raziel_authorization_core::AuthorizationError;
use raziel_verifier::RegistryError;
use raziel_witness::WitnessBuildError;

/// Closed authorization-input assembly failures with no private field values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAssemblyError {
    InvalidWorkerStart,
    InsufficientValidityWindow,
    Registry(RegistryError),
    TrustedSelectionMismatch,
    SnapshotSelectionMismatch,
    RequestMapping(AuthorizationError),
    Witness(WitnessBuildError),
    HostValidation(AuthorizationError),
    EnvelopeCommitmentMismatch,
}

impl Display for InputAssemblyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidWorkerStart => "proof-worker start time is invalid",
            Self::InsufficientValidityWindow => {
                "authorization input lacks the required proof-bound validity window"
            }
            Self::Registry(_) => "trusted policy or snapshot resolution failed",
            Self::TrustedSelectionMismatch => "trusted policy and snapshot records do not match",
            Self::SnapshotSelectionMismatch => {
                "private snapshot selection does not match the signed request"
            }
            Self::RequestMapping(_) => "capability request mapping failed",
            Self::Witness(_) => "positive authorization witness construction failed",
            Self::HostValidation(_) => "assembled authorization input failed host validation",
            Self::EnvelopeCommitmentMismatch => {
                "assembled authorization input does not match its private envelope"
            }
        })
    }
}

impl Error for InputAssemblyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registry(error) => Some(error),
            Self::RequestMapping(error) | Self::HostValidation(error) => Some(error),
            Self::Witness(error) => Some(error),
            Self::InvalidWorkerStart
            | Self::InsufficientValidityWindow
            | Self::TrustedSelectionMismatch
            | Self::SnapshotSelectionMismatch
            | Self::EnvelopeCommitmentMismatch => None,
        }
    }
}
