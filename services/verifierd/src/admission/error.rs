use std::error::Error;
use std::fmt::{self, Display, Formatter};

use raziel_proto::raziel::v1::PublicErrorCode;
use raziel_verifier::RegistryError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationAdmissionError {
    RequestTooLarge,
    InvalidTransportDescriptor,
    MalformedProtobuf,
    UnknownProtobufField,
    ProtobufExtension,
    NonCanonicalProtobuf,
    MissingCapability,
    UnsupportedCapabilityVersion,
    InvalidCapabilityId,
    InvalidCapabilityClaim,
    UnsupportedProofSystem,
    EmptyProofArtifact,
    ProofArtifactTooLarge,
    InvalidVerificationKey,
    MissingSnapshotAttestation,
    SnapshotAttestationMismatch,
    InvalidAgentPublicKeyLength,
    InvalidAgentPublicKey,
    WeakAgentPublicKey,
    AgentKeyMismatch,
    InvalidRequestMethod,
    InvalidRequestTarget,
    InvalidBodyHash,
    InvalidRequestNonce,
    MissingRequestTimestamp,
    InvalidRequestTimestamp,
    RequestTooOld,
    RequestTooFarInFuture,
    CapabilityInactive,
    InvalidRequestSignatureLength,
    InvalidRequestSignature,
    PublicValuesEncoding,
    Registry(RegistryError),
}

impl VerificationAdmissionError {
    #[must_use]
    pub const fn public_error_code(self) -> PublicErrorCode {
        match self {
            Self::RequestTooLarge
            | Self::MalformedProtobuf
            | Self::UnknownProtobufField
            | Self::ProtobufExtension
            | Self::NonCanonicalProtobuf
            | Self::InvalidAgentPublicKeyLength
            | Self::InvalidAgentPublicKey
            | Self::WeakAgentPublicKey
            | Self::InvalidRequestMethod
            | Self::InvalidRequestTarget
            | Self::InvalidBodyHash
            | Self::InvalidRequestNonce
            | Self::MissingRequestTimestamp
            | Self::InvalidRequestTimestamp
            | Self::RequestTooOld
            | Self::RequestTooFarInFuture
            | Self::InvalidRequestSignatureLength
            | Self::InvalidRequestSignature => PublicErrorCode::InvalidRequest,
            Self::InvalidTransportDescriptor => PublicErrorCode::ServiceUnavailable,
            Self::CapabilityInactive
            | Self::Registry(RegistryError::ExpiredPolicy | RegistryError::ExpiredRoot) => {
                PublicErrorCode::CapabilityExpired
            }
            Self::MissingCapability
            | Self::UnsupportedCapabilityVersion
            | Self::InvalidCapabilityId
            | Self::InvalidCapabilityClaim
            | Self::UnsupportedProofSystem
            | Self::EmptyProofArtifact
            | Self::ProofArtifactTooLarge
            | Self::InvalidVerificationKey
            | Self::MissingSnapshotAttestation
            | Self::SnapshotAttestationMismatch
            | Self::AgentKeyMismatch
            | Self::PublicValuesEncoding
            | Self::Registry(_) => PublicErrorCode::CapabilityInvalid,
        }
    }
}

impl Display for VerificationAdmissionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("capability verification admission failed")
    }
}

impl Error for VerificationAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RegistryError> for VerificationAdmissionError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}
