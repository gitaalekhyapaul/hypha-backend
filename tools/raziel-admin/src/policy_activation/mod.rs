//! Safe UI adapter for explicit policy approval and registry publication.

mod codec;
mod server;
pub(crate) mod store;
pub(crate) mod wire;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use crate::policy_publication::roles::{AgentRolesProjection, AgentRolesRequest};
use serde::Serialize;
use std::io::Read;

pub use crate::policy_publication::catalogue::{
    CatalogueIdentity, CataloguePolicy, CatalogueProjection,
};
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
    timeout: Duration,
}

impl PolicyActivationClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Result<Self, PolicyActivationClientError> {
        let socket_path = socket_path.into();
        if !valid_absolute_path(&socket_path) {
            return Err(PolicyActivationClientError::InvalidConfiguration);
        }
        Ok(Self {
            socket_path,
            timeout: SOCKET_TIMEOUT,
        })
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
                operation: wire::PolicyOperationWire::Activate as i32,
                role_request: Vec::new(),
            },
            self.timeout,
        )
    }

    pub fn list_catalogue(&self) -> Result<CatalogueProjection, PolicyActivationClientError> {
        codec::send_catalogue_request(&self.socket_path, self.timeout)
    }

    pub fn agent_roles(
        &self,
        request: AgentRolesRequest,
    ) -> Result<AgentRolesProjection, PolicyActivationClientError> {
        request
            .validate()
            .map_err(|_| PolicyActivationClientError::InvalidRequest)?;
        codec::send_roles_request(&self.socket_path, request, self.timeout)
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
    let operation = parse_client_arguments(std::env::args().skip(1))?;
    let socket_path = std::env::var("RAZIEL_POLICY_ACTIVATION_SOCKET_PATH")
        .map(PathBuf::from)
        .map_err(|_| PolicyActivationClientError::InvalidConfiguration)?;
    let mut client = PolicyActivationClient::new(socket_path)?;
    match std::env::var("RAZIEL_POLICY_PUBLICATION_MODE").as_deref() {
        Ok("composed") => client.timeout = Duration::from_secs(120),
        Err(std::env::VarError::NotPresent) | Ok("legacy")
            if matches!(operation, ClientOperation::Activate(_)) => {}
        _ => return Err(PolicyActivationClientError::InvalidConfiguration),
    }
    let output = match operation {
        ClientOperation::Activate(request) => serde_json::to_string(&client.activate(request)?),
        ClientOperation::ListCatalogue => serde_json::to_string(&client.list_catalogue()?),
        ClientOperation::AgentRoles => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(4097)
                .read_to_end(&mut bytes)
                .map_err(|_| PolicyActivationClientError::InvalidRequest)?;
            if bytes.len() > 4096 {
                return Err(PolicyActivationClientError::InvalidRequest);
            }
            let request = serde_json::from_slice(&bytes)
                .map_err(|_| PolicyActivationClientError::InvalidRequest)?;
            serde_json::to_string(&client.agent_roles(request)?)
        }
    }
    .map_err(|_| PolicyActivationClientError::Protocol)?;
    if output.len() > MAX_FRAME_BYTES {
        return Err(PolicyActivationClientError::Protocol);
    }
    println!("{output}");
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum ClientOperation {
    Activate(PolicyActivationRequest),
    ListCatalogue,
    AgentRoles,
}

fn parse_client_arguments(
    arguments: impl Iterator<Item = String>,
) -> Result<ClientOperation, PolicyActivationClientError> {
    let mut arguments = arguments.peekable();
    if arguments
        .peek()
        .is_some_and(|value| value == "--agent-roles")
    {
        arguments.next();
        return if arguments.next().is_none() {
            Ok(ClientOperation::AgentRoles)
        } else {
            Err(PolicyActivationClientError::InvalidRequest)
        };
    }
    if arguments
        .peek()
        .is_some_and(|value| value == "--list-approved")
    {
        arguments.next();
        return if arguments.next().is_none() {
            Ok(ClientOperation::ListCatalogue)
        } else {
            Err(PolicyActivationClientError::InvalidRequest)
        };
    }
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
    Ok(ClientOperation::Activate(PolicyActivationRequest {
        candidate_id: candidate_id.ok_or(PolicyActivationClientError::InvalidRequest)?,
        approved_by: approved_by.ok_or(PolicyActivationClientError::InvalidRequest)?,
    }))
}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && !path.as_os_str().as_encoded_bytes().contains(&0)
}
