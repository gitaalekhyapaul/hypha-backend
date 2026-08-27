use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use super::codec::{read_request, write_response};
use super::store::{PolicyActivationStore, StoreActivationError};
use super::wire::{
    PolicyActivationFailureWire, PolicyActivationProjectionWire, PolicyActivationResponseWire,
};
use super::{PROTOCOL_VERSION, valid_absolute_path};

const SOCKET_DIRECTORY_MODE: u32 = 0o700;
const SOCKET_MODE: u32 = 0o600;

pub fn run_from_env() -> Result<(), PolicyActivationServerError> {
    let candidate_root = required_path("RAZIEL_POLICY_CANDIDATE_ROOT")?;
    let activation_root = required_path("RAZIEL_POLICY_ACTIVATION_ROOT")?;
    let registry_root = required_path("RAZIEL_REGISTRY_PATH")?;
    let authority_socket = required_path("RAZIEL_POLICY_AUTHORITY_SOCKET_PATH")?;
    let writer_socket = required_path("RAZIEL_REGISTRY_WRITER_SOCKET_PATH")?;
    let socket_path = required_path("RAZIEL_POLICY_ACTIVATION_SOCKET_PATH")?;
    let trust_domain_id = std::env::var("RAZIEL_TRUST_DOMAIN_ID")
        .map_err(|_| PolicyActivationServerError::Configuration)?;
    let store = PolicyActivationStore::new(
        candidate_root,
        activation_root,
        registry_root,
        authority_socket,
        writer_socket,
        &trust_domain_id,
    )
    .map_err(|_| PolicyActivationServerError::Configuration)?;
    let listener = bind_private_socket(&socket_path)?;
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            continue;
        };
        let response = handle_connection(&store, &mut stream);
        let _ = write_response(&mut stream, &response);
    }
    Err(PolicyActivationServerError::Listener)
}

fn handle_connection(
    store: &PolicyActivationStore,
    stream: &mut UnixStream,
) -> PolicyActivationResponseWire {
    if stream
        .set_read_timeout(Some(super::SOCKET_TIMEOUT))
        .is_err()
        || stream
            .set_write_timeout(Some(super::SOCKET_TIMEOUT))
            .is_err()
    {
        return failure(PolicyActivationFailureWire::InvalidRequest);
    }
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(_) => return failure(PolicyActivationFailureWire::InvalidRequest),
    };
    match store.activate(request) {
        Ok(projection) => PolicyActivationResponseWire {
            format_version: PROTOCOL_VERSION,
            failure: PolicyActivationFailureWire::Unspecified as i32,
            projection: Some(PolicyActivationProjectionWire {
                record_id: projection.record_id,
                policy_name: projection.policy_name,
                policy_version: projection.policy_version,
                policy_hash: projection.policy_hash,
                approval_epoch: projection.approval_epoch,
                registered_at_unix_seconds: projection.registered_at_unix_seconds,
                valid_from_unix_seconds: projection.valid_from_unix_seconds,
                valid_until_unix_seconds: projection.valid_until_unix_seconds,
                registry_generation: projection.registry_generation,
            }),
        },
        Err(error) => failure(match error {
            StoreActivationError::InvalidRequest => PolicyActivationFailureWire::InvalidRequest,
            StoreActivationError::Conflict => PolicyActivationFailureWire::Conflict,
            StoreActivationError::CandidateRejected => {
                PolicyActivationFailureWire::CandidateRejected
            }
            StoreActivationError::Unavailable => PolicyActivationFailureWire::Unavailable,
        }),
    }
}

fn failure(code: PolicyActivationFailureWire) -> PolicyActivationResponseWire {
    PolicyActivationResponseWire {
        format_version: PROTOCOL_VERSION,
        failure: code as i32,
        projection: None,
    }
}

fn bind_private_socket(path: &Path) -> Result<UnixListener, PolicyActivationServerError> {
    let parent = path
        .parent()
        .ok_or(PolicyActivationServerError::Configuration)?;
    prepare_private_directory(parent)?;
    remove_safe_stale_socket(path, parent)?;
    let listener = UnixListener::bind(path).map_err(|_| PolicyActivationServerError::Listener)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))
        .map_err(|_| PolicyActivationServerError::Listener)?;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| PolicyActivationServerError::Listener)?;
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| PolicyActivationServerError::Listener)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(PolicyActivationServerError::Listener);
    }
    Ok(listener)
}

fn prepare_private_directory(path: &Path) -> Result<(), PolicyActivationServerError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => validate_private_directory(&metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(SOCKET_DIRECTORY_MODE);
            builder
                .create(path)
                .map_err(|_| PolicyActivationServerError::Configuration)?;
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|_| PolicyActivationServerError::Configuration)?;
            validate_private_directory(&metadata)
        }
        Err(_) => Err(PolicyActivationServerError::Configuration),
    }
}

fn validate_private_directory(
    metadata: &std::fs::Metadata,
) -> Result<(), PolicyActivationServerError> {
    if metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == SOCKET_DIRECTORY_MODE
    {
        Ok(())
    } else {
        Err(PolicyActivationServerError::Configuration)
    }
}

fn remove_safe_stale_socket(path: &Path, parent: &Path) -> Result<(), PolicyActivationServerError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(PolicyActivationServerError::Configuration),
    };
    let parent_metadata = std::fs::symlink_metadata(parent)
        .map_err(|_| PolicyActivationServerError::Configuration)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(PolicyActivationServerError::Configuration);
    }
    std::fs::remove_file(path).map_err(|_| PolicyActivationServerError::Configuration)
}

fn required_path(name: &str) -> Result<PathBuf, PolicyActivationServerError> {
    let path =
        PathBuf::from(std::env::var(name).map_err(|_| PolicyActivationServerError::Configuration)?);
    if valid_absolute_path(&path) {
        Ok(path)
    } else {
        Err(PolicyActivationServerError::Configuration)
    }
}

#[derive(Debug)]
pub enum PolicyActivationServerError {
    Configuration,
    Listener,
}

impl std::fmt::Display for PolicyActivationServerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "policy-activation configuration is invalid",
            Self::Listener => "policy-activation local listener failed",
        })
    }
}

impl std::error::Error for PolicyActivationServerError {}
