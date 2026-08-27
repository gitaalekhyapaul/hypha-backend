//! Safe UI adapter for explicit policy approval and registry publication.

mod codec;
mod server;
mod store;
mod wire;

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

pub use server::run_from_env;

const PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicTimestamp {
    pub seconds: String,
    pub nanos: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyActivationProjection {
    pub record_id: String,
    pub policy_name: String,
    pub policy_version: String,
    pub policy_hash: String,
    pub approval_epoch: String,
    pub status: String,
    pub registered_at: PublicTimestamp,
    pub valid_from: PublicTimestamp,
    pub valid_until: PublicTimestamp,
    pub registry_generation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyActivationRequest {
    pub candidate_id: String,
    pub approved_by: String,
}

#[derive(Clone)]
pub struct PolicyActivationClient {
    socket_path: PathBuf,
}

impl PolicyActivationClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Result<Self, PolicyActivationClientError> {
        let socket_path = socket_path.into();
        if !valid_absolute_path(&socket_path) {
            return Err(PolicyActivationClientError::InvalidConfiguration);
        }
        Ok(Self { socket_path })
    }

    pub fn activate(
        &self,
        request: PolicyActivationRequest,
    ) -> Result<PolicyActivationProjection, PolicyActivationClientError> {
        codec::send_request(
            &self.socket_path,
            wire::PolicyActivationRequestWire {
                format_version: PROTOCOL_VERSION,
                candidate_id: request.candidate_id,
                approved_by: request.approved_by,
            },
        )
    }
}

impl fmt::Debug for PolicyActivationClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PolicyActivationClient")
            .field("socket_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyActivationClientError {
    InvalidConfiguration,
    InvalidRequest,
    Unavailable,
    Protocol,
    Conflict,
    CandidateRejected,
}

impl fmt::Display for PolicyActivationClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid policy-activation configuration",
            Self::InvalidRequest => "invalid policy-activation request",
            Self::Unavailable => "policy activation is unavailable",
            Self::Protocol => "policy-activation protocol failed",
            Self::Conflict => "policy activation conflicts with trusted state",
            Self::CandidateRejected => "policy candidate was rejected",
        })
    }
}

impl Error for PolicyActivationClientError {}

pub fn run_client_from_env() -> Result<(), PolicyActivationClientError> {
    let (candidate_id, approved_by) = parse_client_arguments()?;
    let socket_path = std::env::var("RAZIEL_POLICY_ACTIVATION_SOCKET_PATH")
        .map(PathBuf::from)
        .map_err(|_| PolicyActivationClientError::InvalidConfiguration)?;
    let projection =
        PolicyActivationClient::new(socket_path)?.activate(PolicyActivationRequest {
            candidate_id,
            approved_by,
        })?;
    let output =
        serde_json::to_string(&projection).map_err(|_| PolicyActivationClientError::Protocol)?;
    println!("{output}");
    Ok(())
}

fn parse_client_arguments() -> Result<(String, String), PolicyActivationClientError> {
    let mut arguments = std::env::args().skip(1);
    let mut candidate_id = None;
    let mut approved_by = None;
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or(PolicyActivationClientError::InvalidRequest)?;
        match option.as_str() {
            "--candidate-id" if candidate_id.is_none() => candidate_id = Some(value),
            "--approved-by" if approved_by.is_none() => approved_by = Some(value),
            _ => return Err(PolicyActivationClientError::InvalidRequest),
        }
    }
    Ok((
        candidate_id.ok_or(PolicyActivationClientError::InvalidRequest)?,
        approved_by.ok_or(PolicyActivationClientError::InvalidRequest)?,
    ))
}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && !path.as_os_str().as_encoded_bytes().contains(&0)
}
