//! Strict local protocol for one isolated authorization proof process.

mod codec;
mod executor;
mod frame;
mod launcher;
mod wire;

#[cfg(feature = "one-shot-prover")]
mod config;
#[cfg(feature = "one-shot-prover")]
mod runner;

#[cfg(test)]
pub(crate) mod tests;

use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use raziel_authorization_core::{AuthorizationPublicValuesV1, decode_public_values};

use crate::private_envelope::PrivateCapabilityEnvelope;
use crate::worker::CapabilityExecutionRequest;

pub use codec::{decode_request, decode_result, encode_request, encode_result};
#[cfg(feature = "one-shot-prover")]
pub use config::OneShotProverConfig;
pub use executor::{OneShotExecutorConfigError, UnixSocketExecutor};
pub use frame::{read_frame_to_eof, write_frame};
pub use launcher::{
    PROOF_LAUNCHER_SOCKET_PATH_ENV, PROVER_CHILD_PATH_ENV, PROVER_CHILD_SOCKET_PATH_ENV,
    ProofSandboxLauncherConfig, ProofSandboxLauncherError, run_launcher_from_env,
};
#[cfg(feature = "one-shot-prover")]
pub use runner::run_child_from_env;

/// Version of the one-request, one-result local child protocol.
pub const ONE_SHOT_PROTOCOL_VERSION: u32 = 1;
pub const MAX_ONE_SHOT_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_ONE_SHOT_RESULT_BYTES: usize = 2 * 1024 * 1024;
pub const PROOF_DEADLINE_SECONDS: u32 = 60 * 60;
pub const PROOF_MEMORY_LIMIT_BYTES: u64 = 60_129_542_144;
pub const MAX_CORRELATION_ID_BYTES: usize = 64;
pub const MAX_VERIFICATION_KEY_ID_BYTES: usize = 74;

/// Strictly decoded private handoff accepted by one prover child.
pub struct OneShotRequest {
    envelope: PrivateCapabilityEnvelope,
    worker_start_unix_seconds: i64,
    correlation_id: String,
}

impl OneShotRequest {
    pub fn from_execution(
        request: CapabilityExecutionRequest,
        correlation_id: String,
    ) -> Result<Self, OneShotProtocolError> {
        let (envelope, worker_start_unix_seconds) = request.into_parts();
        Self::new(envelope, worker_start_unix_seconds, correlation_id)
    }

    fn new(
        envelope: PrivateCapabilityEnvelope,
        worker_start_unix_seconds: i64,
        correlation_id: String,
    ) -> Result<Self, OneShotProtocolError> {
        if worker_start_unix_seconds < 0 || !valid_correlation_id(&correlation_id) {
            return Err(OneShotProtocolError::InvalidRequest);
        }
        Ok(Self {
            envelope,
            worker_start_unix_seconds,
            correlation_id,
        })
    }

    pub const fn envelope(&self) -> &PrivateCapabilityEnvelope {
        &self.envelope
    }

    pub const fn worker_start_unix_seconds(&self) -> i64 {
        self.worker_start_unix_seconds
    }

    pub const fn correlation_id(&self) -> &str {
        self.correlation_id.as_str()
    }
}

impl Debug for OneShotRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OneShotRequest")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Safe aggregate metrics returned by an isolated proof child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OneShotMetrics {
    pub execute_millis: u64,
    pub prove_millis: u64,
    pub artifact_bytes: u64,
    pub peak_memory_bytes: u64,
    pub peak_swap_bytes: u64,
}

/// Bounded public output from one completed Plonk proof.
#[derive(Clone, PartialEq, Eq)]
pub struct OneShotSuccess {
    proof_artifact: Vec<u8>,
    public_values_bytes: Vec<u8>,
    public_values: AuthorizationPublicValuesV1,
    verification_key_id: String,
    metrics: OneShotMetrics,
}

impl OneShotSuccess {
    pub fn new(
        proof_artifact: Vec<u8>,
        public_values_bytes: Vec<u8>,
        verification_key_id: String,
        metrics: OneShotMetrics,
    ) -> Result<Self, OneShotProtocolError> {
        let maximum_duration_millis = u64::from(PROOF_DEADLINE_SECONDS) * 1_000;
        let total_duration_millis = metrics
            .execute_millis
            .checked_add(metrics.prove_millis)
            .ok_or(OneShotProtocolError::InvalidResult)?;
        if proof_artifact.is_empty()
            || proof_artifact.len() > raziel_prover_artifact_limit()
            || metrics.artifact_bytes != proof_artifact.len() as u64
            || total_duration_millis > maximum_duration_millis
            || metrics.peak_memory_bytes > PROOF_MEMORY_LIMIT_BYTES
            || metrics.peak_swap_bytes != 0
            || !valid_verification_key_id(&verification_key_id)
        {
            return Err(OneShotProtocolError::InvalidResult);
        }
        let public_values = decode_public_values(&public_values_bytes)
            .map_err(|_| OneShotProtocolError::InvalidResult)?;
        Ok(Self {
            proof_artifact,
            public_values_bytes,
            public_values,
            verification_key_id,
            metrics,
        })
    }

    pub fn proof_artifact(&self) -> &[u8] {
        &self.proof_artifact
    }

    pub fn public_values_bytes(&self) -> &[u8] {
        &self.public_values_bytes
    }

    pub const fn public_values(&self) -> &AuthorizationPublicValuesV1 {
        &self.public_values
    }

    pub const fn verification_key_id(&self) -> &str {
        self.verification_key_id.as_str()
    }

    pub const fn metrics(&self) -> OneShotMetrics {
        self.metrics
    }
}

impl Debug for OneShotSuccess {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OneShotSuccess")
            .field("proof_artifact", &"<redacted>")
            .field("public_values", &self.public_values)
            .field("verification_key_id", &self.verification_key_id)
            .field("metrics", &self.metrics)
            .finish()
    }
}

/// Closed failure classes returned over the local process channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OneShotFailureClass {
    InvalidRequest,
    TrustedState,
    WitnessRejected,
    HostRejected,
    ProofFailed,
    JournalMismatch,
    ResourceProfile,
    Cancelled,
    Internal,
}

/// Generic child failure with no private detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneShotFailure {
    pub class: OneShotFailureClass,
    pub correlation_id: String,
}

impl OneShotFailure {
    pub fn new(
        class: OneShotFailureClass,
        correlation_id: String,
    ) -> Result<Self, OneShotProtocolError> {
        if !valid_correlation_id(&correlation_id) {
            return Err(OneShotProtocolError::InvalidResult);
        }
        Ok(Self {
            class,
            correlation_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OneShotResult {
    Success(Box<OneShotSuccess>),
    Failure(OneShotFailure),
}

/// Redacted transport and validation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OneShotProtocolError {
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
    TrustedState,
    Execution,
}

impl fmt::Display for OneShotProtocolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RequestTooLarge => "one-shot request exceeds its byte limit",
            Self::ResultTooLarge => "one-shot result exceeds its byte limit",
            Self::MalformedRequest => "one-shot request is malformed",
            Self::MalformedResult => "one-shot result is malformed",
            Self::UnsupportedVersion => "one-shot protocol version is unsupported",
            Self::InvalidRequest => "one-shot request is invalid",
            Self::InvalidResult => "one-shot result is invalid",
            Self::FrameRead => "one-shot frame read failed",
            Self::FrameWrite => "one-shot frame write failed",
            Self::TrailingFrameData => "one-shot channel contains trailing data",
            Self::Configuration => "one-shot prover configuration is invalid",
            Self::ResourceProfile => "one-shot prover resource profile is invalid",
            Self::TrustedState => "one-shot trusted state is invalid",
            Self::Execution => "one-shot proof execution failed",
        })
    }
}

impl Error for OneShotProtocolError {}

const fn raziel_prover_artifact_limit() -> usize {
    1024 * 1024
}

fn valid_correlation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CORRELATION_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_verification_key_id(value: &str) -> bool {
    let Some(digest) = value.strip_prefix("sp1-vk-v1:") else {
        return false;
    };
    value.len() == MAX_VERIFICATION_KEY_ID_BYTES
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
