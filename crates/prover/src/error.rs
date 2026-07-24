use std::fmt::{self, Display, Formatter};

use raziel_authorization_core::AuthorizationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProverError {
    InvalidInput(AuthorizationError),
    InvalidCycleLimit,
    GuestExecutionFailed,
    PublicValuesMismatch,
}

impl Display for ProverError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput(_) => "authorization guest input is invalid",
            Self::InvalidCycleLimit => "authorization guest cycle limit is invalid",
            Self::GuestExecutionFailed => "authorization guest execution failed",
            Self::PublicValuesMismatch => {
                "authorization guest public values do not match the host statement"
            }
        })
    }
}

impl std::error::Error for ProverError {}

impl From<AuthorizationError> for ProverError {
    fn from(value: AuthorizationError) -> Self {
        Self::InvalidInput(value)
    }
}
