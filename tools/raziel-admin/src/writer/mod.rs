//! Owner-only Unix-socket client and server for accepted-root mutations.

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

const PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 1 << 20;
const MAX_REVOCATION_REASON_BYTES: usize = 512;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(10);

/// Public result of one committed registry mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryWriterResult {
    pub registry_generation: u64,
    pub root_record: proto::AcceptedRootRecord,
}

/// Synchronous client for the local owner-only writer socket.
#[derive(Clone)]
pub struct RegistryWriterClient {
    socket_path: PathBuf,
}

impl RegistryWriterClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Result<Self, RegistryWriterClientError> {
        let socket_path = socket_path.into();
        if !valid_absolute_path(&socket_path) {
            return Err(RegistryWriterClientError::InvalidConfiguration);
        }
        Ok(Self { socket_path })
    }

    pub fn append_snapshot(
        &self,
        expected_registry_generation: u64,
        descriptor: proto::RelationshipSnapshotDescriptor,
    ) -> Result<RegistryWriterResult, RegistryWriterClientError> {
        self.send(wire::RegistryWriterRequestWire {
            format_version: PROTOCOL_VERSION,
            expected_registry_generation,
            operation: Some(
                wire::registry_writer_request_wire::Operation::AppendSnapshot(Box::new(descriptor)),
            ),
        })
    }

    pub fn revoke_snapshot(
        &self,
        expected_registry_generation: u64,
        snapshot_id: String,
        reason: String,
    ) -> Result<RegistryWriterResult, RegistryWriterClientError> {
        if reason.len() > MAX_REVOCATION_REASON_BYTES {
            return Err(RegistryWriterClientError::InvalidRequest);
        }
        self.send(wire::RegistryWriterRequestWire {
            format_version: PROTOCOL_VERSION,
            expected_registry_generation,
            operation: Some(
                wire::registry_writer_request_wire::Operation::RevokeSnapshot(
                    wire::RevokeSnapshotWire {
                        snapshot_id,
                        reason,
                    },
                ),
            ),
        })
    }

    fn send(
        &self,
        request: wire::RegistryWriterRequestWire,
    ) -> Result<RegistryWriterResult, RegistryWriterClientError> {
        codec::send_request(&self.socket_path, request)
    }
}

impl fmt::Debug for RegistryWriterClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryWriterClient")
            .field("socket_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryWriterClientError {
    InvalidConfiguration,
    InvalidRequest,
    Unavailable,
    Protocol,
    StaleGeneration,
    Conflict,
    RegistryRejected,
    PublicationFailed,
}

impl fmt::Display for RegistryWriterClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid registry-writer configuration",
            Self::InvalidRequest => "invalid registry-writer request",
            Self::Unavailable => "registry writer is unavailable",
            Self::Protocol => "registry-writer protocol failed",
            Self::StaleGeneration => "registry generation is stale",
            Self::Conflict => "registry mutation conflicts with current state",
            Self::RegistryRejected => "registry rejected the mutation",
            Self::PublicationFailed => "registry publication failed",
        })
    }
}

impl Error for RegistryWriterClientError {}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && !path.as_os_str().as_encoded_bytes().contains(&0)
}
