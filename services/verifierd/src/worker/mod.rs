//! Strict public protocol for one isolated SP1 verification process.

mod client;
mod codec;
mod frame;
mod launcher;
mod runner;
mod wire;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use raziel_authorization_core::decode_public_values;
use raziel_prover::{MAX_PROOF_ARTIFACT_BYTES, TrustedProgramKey};

use crate::admission::VerificationWorkerRequest;

pub use client::{
    UnixSocketVerificationExecutor, VerificationExecutionError, VerificationExecutor,
    VerificationFuture,
};
pub use codec::{decode_request, decode_result, encode_request, encode_result};
pub use frame::{read_frame_to_eof, write_frame};
pub use launcher::{
    VERIFIER_CHILD_PATH_ENV, VERIFIER_CHILD_SOCKET_PATH_ENV, VERIFIER_LAUNCHER_SOCKET_PATH_ENV,
    VerifierSandboxLauncherConfig, VerifierSandboxLauncherError, run_launcher_from_env,
};
pub use runner::run_child_from_env;

pub const VERIFICATION_WORKER_PROTOCOL_VERSION: u32 = 1;
pub const VERIFICATION_DEADLINE_SECONDS: u32 = 60;
pub const VERIFICATION_MEMORY_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_VERIFICATION_WORKER_REQUEST_BYTES: usize = MAX_PROOF_ARTIFACT_BYTES + 64 * 1024;
pub const MAX_VERIFICATION_WORKER_RESULT_BYTES: usize = 4 * 1024;

const MAX_EXPECTED_PUBLIC_VALUES_BYTES: usize = 4 * 1024;
const MAX_CORRELATION_ID_BYTES: usize = 64;

/// Public proof input accepted by one isolated verifier child.
#[derive(Clone, PartialEq, Eq)]
pub struct IsolatedVerificationRequest {
    proof_artifact: Vec<u8>,
    expected_public_values: Vec<u8>,
    trusted_program_key: TrustedProgramKey,
    correlation_id: String,
}

impl IsolatedVerificationRequest {
    pub fn from_admission(
        request: &VerificationWorkerRequest,
        proof_digest: &[u8; 32],
    ) -> Result<Self, VerificationWorkerProtocolError> {
        Self::new(
            request.proof_artifact().to_vec(),
            request.expected_public_values().to_vec(),
            request.trusted_program_key().clone(),
            lowercase_hex(proof_digest),
        )
    }

    fn new(
        proof_artifact: Vec<u8>,
        expected_public_values: Vec<u8>,
        trusted_program_key: TrustedProgramKey,
        correlation_id: String,
    ) -> Result<Self, VerificationWorkerProtocolError> {
        if proof_artifact.is_empty()
            || proof_artifact.len() > MAX_PROOF_ARTIFACT_BYTES
            || expected_public_values.is_empty()
            || expected_public_values.len() > MAX_EXPECTED_PUBLIC_VALUES_BYTES
            || decode_public_values(&expected_public_values).is_err()
            || !valid_correlation_id(&correlation_id)
        {
            return Err(VerificationWorkerProtocolError::InvalidRequest);
        }
        Ok(Self {
            proof_artifact,
            expected_public_values,
            trusted_program_key,
            correlation_id,
        })
    }

    #[must_use]
    pub fn proof_artifact(&self) -> &[u8] {
        &self.proof_artifact
    }

    #[must_use]
    pub fn expected_public_values(&self) -> &[u8] {
        &self.expected_public_values
    }

    #[must_use]
    pub const fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.trusted_program_key
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

impl Debug for IsolatedVerificationRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IsolatedVerificationRequest")
            .field("public_proof_input", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationWorkerSuccess {
    correlation_id: String,
    verification_key_id: String,
}

impl VerificationWorkerSuccess {
    fn new(
        correlation_id: String,
        verification_key_id: String,
    ) -> Result<Self, VerificationWorkerProtocolError> {
        if !valid_correlation_id(&correlation_id)
            || !valid_verification_key_id(&verification_key_id)
        {
            return Err(VerificationWorkerProtocolError::InvalidResult);
        }
        Ok(Self {
            correlation_id,
            verification_key_id,
        })
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    #[must_use]
    pub fn verification_key_id(&self) -> &str {
        &self.verification_key_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationWorkerFailureClass {
    InvalidRequest,
    ProofRejected,
    ResourceProfile,
    Cancelled,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationWorkerFailure {
    class: VerificationWorkerFailureClass,
    correlation_id: String,
}

impl VerificationWorkerFailure {
    fn new(
        class: VerificationWorkerFailureClass,
        correlation_id: String,
    ) -> Result<Self, VerificationWorkerProtocolError> {
        if !valid_correlation_id(&correlation_id) {
            return Err(VerificationWorkerProtocolError::InvalidResult);
        }
        Ok(Self {
            class,
            correlation_id,
        })
    }

    #[must_use]
    pub const fn class(&self) -> VerificationWorkerFailureClass {
        self.class
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationWorkerResult {
    Success(VerificationWorkerSuccess),
    Failure(VerificationWorkerFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationWorkerProtocolError {
    RequestTooLarge,
    ResultTooLarge,
    MalformedRequest,
    MalformedResult,
    UnsupportedVersion,
    InvalidRequest,
    InvalidResult,
    FrameRead,
    FrameWrite,
    TrailingFrameData,
    Configuration,
    ResourceProfile,
}

impl fmt::Display for VerificationWorkerProtocolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("isolated verification protocol failed")
    }
}

impl Error for VerificationWorkerProtocolError {}

fn valid_correlation_id(value: &str) -> bool {
    value.len() == MAX_CORRELATION_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_verification_key_id(value: &str) -> bool {
    value
        .strip_prefix("sp1-vk-v1:")
        .is_some_and(valid_correlation_id)
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
