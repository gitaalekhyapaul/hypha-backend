//! Owner-only policy approval client and authority service.

mod codec;
mod server;
mod store;
mod wire;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use raziel_proto::raziel::v1 as proto;

pub use server::run_from_env;
pub use wire::PolicyCandidateManifestWire;

const PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 1 << 20;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, PartialEq)]
pub struct PolicyAuthorityResult {
    pub staged_policy_record: proto::PolicyRecord,
}

impl fmt::Debug for PolicyAuthorityResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PolicyAuthorityResult")
            .field("record_id", &self.staged_policy_record.record_id)
            .field("staged_policy_record", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PolicyAuthorityApprovalRequest {
    pub candidate_id: String,
    pub expected_policy_hash: [u8; 32],
    pub expected_approval_epoch: u64,
    pub valid_from_unix_seconds: i64,
    pub valid_until_unix_seconds: i64,
    pub approved_by: String,
    pub approval_nonce: [u8; 32],
}

impl fmt::Debug for PolicyAuthorityApprovalRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PolicyAuthorityApprovalRequest")
            .field("candidate_id", &self.candidate_id)
            .field("expected_policy_hash", &"<public hash>")
            .field("expected_approval_epoch", &self.expected_approval_epoch)
            .field("valid_from_unix_seconds", &self.valid_from_unix_seconds)
            .field("valid_until_unix_seconds", &self.valid_until_unix_seconds)
            .field("approved_by", &self.approved_by)
            .field("approval_nonce", &"<redacted>")
            .finish()
    }
}

#[derive(Clone)]
pub struct PolicyAuthorityClient {
    socket_path: PathBuf,
}

impl PolicyAuthorityClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Result<Self, PolicyAuthorityClientError> {
        let socket_path = socket_path.into();
        if !valid_absolute_path(&socket_path) {
            return Err(PolicyAuthorityClientError::InvalidConfiguration);
        }
        Ok(Self { socket_path })
    }

    pub fn approve(
        &self,
        request: PolicyAuthorityApprovalRequest,
    ) -> Result<PolicyAuthorityResult, PolicyAuthorityClientError> {
        codec::send_request(
            &self.socket_path,
            wire::PolicyAuthorityRequestWire {
                format_version: PROTOCOL_VERSION,
                candidate_id: request.candidate_id,
                expected_policy_hash: request.expected_policy_hash.to_vec(),
                expected_approval_epoch: request.expected_approval_epoch,
                valid_from_unix_seconds: request.valid_from_unix_seconds,
                valid_until_unix_seconds: request.valid_until_unix_seconds,
                approved_by: request.approved_by,
                approval_nonce: request.approval_nonce.to_vec(),
            },
        )
    }
}

impl fmt::Debug for PolicyAuthorityClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PolicyAuthorityClient")
            .field("socket_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyAuthorityClientError {
    InvalidConfiguration,
    InvalidRequest,
    Unavailable,
    Protocol,
    StaleEpoch,
    Conflict,
    CandidateRejected,
}

impl fmt::Display for PolicyAuthorityClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid policy-authority configuration",
            Self::InvalidRequest => "invalid policy-authority request",
            Self::Unavailable => "policy authority is unavailable",
            Self::Protocol => "policy-authority protocol failed",
            Self::StaleEpoch => "policy approval epoch is stale",
            Self::Conflict => "policy approval conflicts with stored state",
            Self::CandidateRejected => "policy candidate was rejected",
        })
    }
}

impl Error for PolicyAuthorityClientError {}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && !path.as_os_str().as_encoded_bytes().contains(&0)
}
