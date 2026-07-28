use std::fmt::{self, Display, Formatter};

use raziel_authorization_core::AuthorizationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProverError {
    InvalidInput(AuthorizationError),
    InvalidCycleLimit,
    UnsafeSp1Environment,
    PlonkArtifactsUnavailable,
    ProofJobUnavailable,
    ProverSetupFailed,
    GuestExecutionFailed,
    ProofGenerationFailed,
    ProofArtifactTooLarge,
    ProofArtifactInvalid,
    ProofModeUnsupported,
    SdkVersionMismatch,
    CircuitVersionMismatch,
    TeeProofUnsupported,
    VerificationKeyInvalid,
    VerificationKeyMismatch,
    PublicValuesMismatch,
    ProofVerificationFailed,
}

impl Display for ProverError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput(_) => "authorization guest input is invalid",
            Self::InvalidCycleLimit => "authorization guest cycle limit is invalid",
            Self::UnsafeSp1Environment => "an unsafe SP1 environment setting is not permitted",
            Self::PlonkArtifactsUnavailable => "pinned SP1 Plonk setup artifacts are unavailable",
            Self::ProofJobUnavailable => "another local authorization proof job is active",
            Self::ProverSetupFailed => "SP1 local key setup failed",
            Self::GuestExecutionFailed => "authorization guest execution failed",
            Self::ProofGenerationFailed => "authorization proof generation failed",
            Self::ProofArtifactTooLarge => "authorization proof artifact exceeds its size limit",
            Self::ProofArtifactInvalid => "authorization proof artifact is invalid",
            Self::ProofModeUnsupported => "authorization proof mode is unsupported",
            Self::SdkVersionMismatch => "authorization proof SDK version is unsupported",
            Self::CircuitVersionMismatch => "authorization proof circuit version is unsupported",
            Self::TeeProofUnsupported => "TEE proof material is unsupported",
            Self::VerificationKeyInvalid => "authorization verification key is invalid",
            Self::VerificationKeyMismatch => {
                "authorization verification key does not match the expected key"
            }
            Self::PublicValuesMismatch => {
                "authorization guest public values do not match the host statement"
            }
            Self::ProofVerificationFailed => "authorization proof verification failed",
        })
    }
}

impl std::error::Error for ProverError {}

impl From<AuthorizationError> for ProverError {
    fn from(value: AuthorizationError) -> Self {
        Self::InvalidInput(value)
    }
}
