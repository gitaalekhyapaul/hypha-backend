use std::error::Error;
use std::fmt::{self, Display, Formatter};

use raziel_policy_ir::EvaluationError;
use raziel_snapshot::SnapshotError;

/// Closed witness-construction failures with no private values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessBuildError {
    InvalidSnapshot(SnapshotError),
    SnapshotRootMismatch,
    EvaluationFailed(EvaluationError),
    AuthorizationDenied,
    InvalidEvaluationTrace,
    InvalidConstructedWitness(SnapshotError),
}

impl Display for WitnessBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot(_) => "selected relationship snapshot is invalid",
            Self::SnapshotRootMismatch => {
                "selected relationship data does not match the committed snapshot root"
            }
            Self::EvaluationFailed(_) => "authorization evaluation failed",
            Self::AuthorizationDenied => "authorization has no positive witness path",
            Self::InvalidEvaluationTrace => "authorization returned an invalid witness trace",
            Self::InvalidConstructedWitness(_) => {
                "constructed relationship witness failed validation"
            }
        })
    }
}

impl Error for WitnessBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSnapshot(error) | Self::InvalidConstructedWitness(error) => Some(error),
            Self::EvaluationFailed(error) => Some(error),
            Self::SnapshotRootMismatch
            | Self::AuthorizationDenied
            | Self::InvalidEvaluationTrace => None,
        }
    }
}
