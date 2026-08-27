use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use super::codec::{read_request, write_response};
use super::store::{RegistryMutationStore, StoreMutationError, StoreMutationResult, trusted_now};
use super::wire::{RegistryWriterFailureWire, RegistryWriterResponseWire};
use super::{PROTOCOL_VERSION, valid_absolute_path};

const SOCKET_DIRECTORY_MODE: u32 = 0o700;
const SOCKET_MODE: u32 = 0o600;

pub fn run_from_env() -> Result<(), RegistryWriterServerError> {
    let registry_root = required_path("RAZIEL_REGISTRY_PATH")?;
    let socket_path = required_path("RAZIEL_REGISTRY_WRITER_SOCKET_PATH")?;
    let trust_domain_id = std::env::var("RAZIEL_TRUST_DOMAIN_ID")
        .map_err(|_| RegistryWriterServerError::Configuration)?;
    let store = RegistryMutationStore::new(registry_root, &trust_domain_id)
        .map_err(|_| RegistryWriterServerError::Configuration)?;
    let listener = bind_private_socket(&socket_path)?;
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            continue;
        };
        let response = handle_connection(&store, &mut stream);
        let _ = write_response(&mut stream, &response);
    }
    Err(RegistryWriterServerError::Listener)
}

pub(super) fn handle_connection(
    store: &RegistryMutationStore,
    stream: &mut UnixStream,
) -> RegistryWriterResponseWire {
    if stream
        .set_read_timeout(Some(super::SOCKET_TIMEOUT))
        .is_err()
        || stream
            .set_write_timeout(Some(super::SOCKET_TIMEOUT))
            .is_err()
    {
        return failure(RegistryWriterFailureWire::InvalidRequest);
    }
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(_) => return failure(RegistryWriterFailureWire::InvalidRequest),
    };
    let now = match trusted_now() {
        Ok(now) => now,
        Err(_) => return failure(RegistryWriterFailureWire::PublicationFailed),
    };
    handle_request(store, request, now)
}

pub(super) fn handle_request(
    store: &RegistryMutationStore,
    request: super::wire::RegistryWriterRequestWire,
    now: i64,
) -> RegistryWriterResponseWire {
    match store.apply(request, now) {
        Ok(StoreMutationResult::Root(root_record)) => RegistryWriterResponseWire {
            format_version: PROTOCOL_VERSION,
            failure: RegistryWriterFailureWire::Unspecified as i32,
            registry_generation: root_record.registry_generation,
            root_record: Some(root_record),
            policy_record: None,
        },
        Ok(StoreMutationResult::Policy(policy_record)) => RegistryWriterResponseWire {
            format_version: PROTOCOL_VERSION,
            failure: RegistryWriterFailureWire::Unspecified as i32,
            registry_generation: policy_record.registry_generation,
            root_record: None,
            policy_record: Some(policy_record),
        },
        Err(error) => failure(match error {
            StoreMutationError::StaleGeneration => RegistryWriterFailureWire::StaleGeneration,
            StoreMutationError::Conflict => RegistryWriterFailureWire::Conflict,
            StoreMutationError::InvalidRequest => RegistryWriterFailureWire::InvalidRequest,
            StoreMutationError::RegistryRejected => RegistryWriterFailureWire::RegistryRejected,
            StoreMutationError::PublicationFailed => RegistryWriterFailureWire::PublicationFailed,
        }),
    }
}

fn failure(code: RegistryWriterFailureWire) -> RegistryWriterResponseWire {
    RegistryWriterResponseWire {
        format_version: PROTOCOL_VERSION,
        failure: code as i32,
        registry_generation: 0,
        root_record: None,
        policy_record: None,
    }
}

pub(super) fn bind_private_socket(path: &Path) -> Result<UnixListener, RegistryWriterServerError> {
    let parent = path
        .parent()
        .ok_or(RegistryWriterServerError::Configuration)?;
    prepare_private_directory(parent)?;
    remove_safe_stale_socket(path, parent)?;
    let listener = UnixListener::bind(path).map_err(|_| RegistryWriterServerError::Listener)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))
        .map_err(|_| RegistryWriterServerError::Listener)?;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| RegistryWriterServerError::Listener)?;
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| RegistryWriterServerError::Listener)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(RegistryWriterServerError::Listener);
    }
    Ok(listener)
}

fn prepare_private_directory(path: &Path) -> Result<(), RegistryWriterServerError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => validate_private_directory(&metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(SOCKET_DIRECTORY_MODE);
            builder
                .create(path)
                .map_err(|_| RegistryWriterServerError::Configuration)?;
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|_| RegistryWriterServerError::Configuration)?;
            validate_private_directory(&metadata)
        }
        Err(_) => Err(RegistryWriterServerError::Configuration),
    }
}

fn validate_private_directory(
    metadata: &std::fs::Metadata,
) -> Result<(), RegistryWriterServerError> {
    if metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == SOCKET_DIRECTORY_MODE
    {
        Ok(())
    } else {
        Err(RegistryWriterServerError::Configuration)
    }
}

fn remove_safe_stale_socket(path: &Path, parent: &Path) -> Result<(), RegistryWriterServerError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(RegistryWriterServerError::Configuration),
    };
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| RegistryWriterServerError::Configuration)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(RegistryWriterServerError::Configuration);
    }
    std::fs::remove_file(path).map_err(|_| RegistryWriterServerError::Configuration)
}

fn required_path(name: &str) -> Result<PathBuf, RegistryWriterServerError> {
    let path =
        PathBuf::from(std::env::var(name).map_err(|_| RegistryWriterServerError::Configuration)?);
    if valid_absolute_path(&path) {
        Ok(path)
    } else {
        Err(RegistryWriterServerError::Configuration)
    }
}

#[derive(Debug)]
pub enum RegistryWriterServerError {
    Configuration,
    Listener,
}

impl std::fmt::Display for RegistryWriterServerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "registry-writer configuration is invalid",
            Self::Listener => "registry-writer local listener failed",
        })
    }
}

impl std::error::Error for RegistryWriterServerError {}
