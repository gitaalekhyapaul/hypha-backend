//! Safe UI adapter for private relationship publication and snapshot creation.

mod codec;
mod server;
mod wire;

use std::error::Error;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use wire::{
    AccessGraphRequestWire, RelationshipWire, StoredCaveatWire, StoredContextWire,
    stored_context_wire,
};

pub use server::{AccessGraphServerError, run_from_env};

const PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 256 * 1024;
const MAX_STDIN_BYTES: u64 = 256 * 1024;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicTimestamp {
    pub seconds: String,
    pub nanos: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AccessGraphProjection {
    pub policy_record_id: String,
    pub policy_hash: String,
    pub snapshot_id: String,
    pub relationship_root: String,
    pub epoch: String,
    pub issued_at: PublicTimestamp,
    pub expires_at: PublicTimestamp,
    pub accepted_relationship_count: u32,
    pub registry_generation: String,
    pub status: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessGraphInput {
    candidate_id: String,
    operator_id: String,
    valid_for_seconds: u32,
    relationships: Vec<RelationshipInput>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipInput {
    resource_type: String,
    resource_id: String,
    relation: String,
    subject_type: String,
    subject_id: String,
    #[serde(default)]
    subject_relation: Option<String>,
    #[serde(default)]
    caveat: Option<StoredCaveatInput>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCaveatInput {
    name: String,
    context: Vec<StoredContextInput>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredContextInput {
    name: String,
    value: StoredValueInput,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum StoredValueInput {
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Clone)]
pub struct AccessGraphClient {
    socket_path: PathBuf,
}

impl AccessGraphClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Result<Self, AccessGraphClientError> {
        let socket_path = socket_path.into();
        if !valid_absolute_path(&socket_path) {
            return Err(AccessGraphClientError::InvalidConfiguration);
        }
        Ok(Self { socket_path })
    }

    fn publish(
        &self,
        request: AccessGraphRequestWire,
    ) -> Result<AccessGraphProjection, AccessGraphClientError> {
        codec::send_request(&self.socket_path, request)
    }
}

impl fmt::Debug for AccessGraphClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccessGraphClient")
            .field("socket_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessGraphClientError {
    InvalidConfiguration,
    InvalidRequest,
    Unavailable,
    Protocol,
    Conflict,
    CandidateRejected,
}

impl fmt::Display for AccessGraphClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid access-graph configuration",
            Self::InvalidRequest => "invalid access-graph request",
            Self::Unavailable => "access-graph publication is unavailable",
            Self::Protocol => "access-graph protocol failed",
            Self::Conflict => "access graph conflicts with stored candidate state",
            Self::CandidateRejected => "access graph was rejected",
        })
    }
}

impl Error for AccessGraphClientError {}

pub fn run_client_from_env() -> Result<(), AccessGraphClientError> {
    let input = read_input()?;
    let socket_path = std::env::var("RAZIEL_ACCESS_GRAPH_SOCKET_PATH")
        .map(PathBuf::from)
        .map_err(|_| AccessGraphClientError::InvalidConfiguration)?;
    let projection = AccessGraphClient::new(socket_path)?.publish(input_to_wire(input))?;
    let output =
        serde_json::to_string(&projection).map_err(|_| AccessGraphClientError::Protocol)?;
    println!("{output}");
    Ok(())
}

fn read_input() -> Result<AccessGraphInput, AccessGraphClientError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_STDIN_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AccessGraphClientError::InvalidRequest)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_STDIN_BYTES {
        return Err(AccessGraphClientError::InvalidRequest);
    }
    serde_json::from_slice(&bytes).map_err(|_| AccessGraphClientError::InvalidRequest)
}

fn input_to_wire(input: AccessGraphInput) -> AccessGraphRequestWire {
    AccessGraphRequestWire {
        format_version: PROTOCOL_VERSION,
        candidate_id: input.candidate_id,
        operator_id: input.operator_id,
        valid_for_seconds: input.valid_for_seconds,
        relationships: input
            .relationships
            .into_iter()
            .map(|relationship| RelationshipWire {
                resource_type: relationship.resource_type,
                resource_id: relationship.resource_id,
                relation: relationship.relation,
                subject_type: relationship.subject_type,
                subject_id: relationship.subject_id,
                subject_relation: relationship.subject_relation,
                caveat: relationship.caveat.map(|caveat| StoredCaveatWire {
                    name: caveat.name,
                    context: caveat
                        .context
                        .into_iter()
                        .map(|entry| StoredContextWire {
                            name: entry.name,
                            value: Some(match entry.value {
                                StoredValueInput::Bool(value) => {
                                    stored_context_wire::Value::Bool(value)
                                }
                                StoredValueInput::Int(value) => {
                                    stored_context_wire::Value::Int(value)
                                }
                                StoredValueInput::String(value) => {
                                    stored_context_wire::Value::String(value)
                                }
                            }),
                        })
                        .collect(),
                }),
            })
            .collect(),
    }
}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && !path.as_os_str().as_encoded_bytes().contains(&0)
}
