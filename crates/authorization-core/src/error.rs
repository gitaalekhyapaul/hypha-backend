use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationError {
    UnsupportedInputVersion,
    UnsupportedRequestVersion,
    InvalidRequest,
    InvalidContext,
    InvalidTimestamp,
    InvalidPolicy,
    PolicyTooLarge,
    PolicyHashMismatch,
    InvalidWitness,
    EvaluationFailed,
    AuthorizationDenied,
    InvalidWitnessUsage,
    CanonicalEncoding,
    InvalidPublicValues,
}

impl Display for AuthorizationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedInputVersion => "unsupported authorization input version",
            Self::UnsupportedRequestVersion => "unsupported capability request version",
            Self::InvalidRequest => "invalid capability request",
            Self::InvalidContext => "invalid authorization context",
            Self::InvalidTimestamp => "invalid authorization timestamp",
            Self::InvalidPolicy => "invalid authorization policy",
            Self::PolicyTooLarge => "authorization policy exceeds its trusted limit",
            Self::PolicyHashMismatch => "authorization policy hash does not match",
            Self::InvalidWitness => "invalid relationship witness",
            Self::EvaluationFailed => "authorization evaluation failed",
            Self::AuthorizationDenied => "authorization was not allowed",
            Self::InvalidWitnessUsage => "relationship witness usage is invalid",
            Self::CanonicalEncoding => "authorization canonical encoding failed",
            Self::InvalidPublicValues => "authorization public values are invalid",
        })
    }
}

impl std::error::Error for AuthorizationError {}
