use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use raziel_authority::{AuthoritySigner, PolicyApprovalRole};

use super::codec::{read_request, write_response};
use super::store::{PolicyApprovalStore, StoreApprovalError, trusted_now};
use super::wire::{PolicyAuthorityFailureWire, PolicyAuthorityResponseWire};
use super::{PROTOCOL_VERSION, valid_absolute_path};

const SOCKET_DIRECTORY_MODE: u32 = 0o700;
const SOCKET_MODE: u32 = 0o600;

pub fn run_from_env() -> Result<(), PolicyAuthorityServerError> {
    let candidate_root = required_path("RAZIEL_POLICY_CANDIDATE_ROOT")?;
    let registry_root = required_path("RAZIEL_REGISTRY_PATH")?;
    let socket_path = required_path("RAZIEL_POLICY_AUTHORITY_SOCKET_PATH")?;
    let key_path = required_path("RAZIEL_POLICY_KEY_PATH")?;
    let trust_domain_id = std::env::var("RAZIEL_TRUST_DOMAIN_ID")
        .map_err(|_| PolicyAuthorityServerError::Configuration)?;
    let signer = AuthoritySigner::<PolicyApprovalRole>::from_read_only_pkcs8_file(&key_path)
        .map_err(|_| PolicyAuthorityServerError::Configuration)?;
    let store = PolicyApprovalStore::new(candidate_root, registry_root, &trust_domain_id, signer)
        .map_err(|_| PolicyAuthorityServerError::Configuration)?;
    let listener = bind_private_socket(&socket_path)?;
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            continue;
        };
        let response = handle_connection(&store, &mut stream);
        let _ = write_response(&mut stream, &response);
    }
    Err(PolicyAuthorityServerError::Listener)
}

pub(super) fn handle_connection(
    store: &PolicyApprovalStore,
    stream: &mut UnixStream,
) -> PolicyAuthorityResponseWire {
    if stream
        .set_read_timeout(Some(super::SOCKET_TIMEOUT))
        .is_err()
        || stream
            .set_write_timeout(Some(super::SOCKET_TIMEOUT))
            .is_err()
    {
        return failure(PolicyAuthorityFailureWire::InvalidRequest);
    }
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(_) => return failure(PolicyAuthorityFailureWire::InvalidRequest),
    };
    let now = match trusted_now() {
        Ok(now) => now,
        Err(_) => return failure(PolicyAuthorityFailureWire::Unavailable),
    };
    match store.apply(request, now) {
        Ok(staged_policy_record) => PolicyAuthorityResponseWire {
            format_version: PROTOCOL_VERSION,
            failure: PolicyAuthorityFailureWire::Unspecified as i32,
            staged_policy_record: Some(staged_policy_record),
        },
        Err(error) => failure(match error {
            StoreApprovalError::StaleEpoch => PolicyAuthorityFailureWire::StaleEpoch,
            StoreApprovalError::Conflict => PolicyAuthorityFailureWire::Conflict,
            StoreApprovalError::InvalidRequest => PolicyAuthorityFailureWire::InvalidRequest,
            StoreApprovalError::CandidateRejected => PolicyAuthorityFailureWire::CandidateRejected,
            StoreApprovalError::Unavailable => PolicyAuthorityFailureWire::Unavailable,
        }),
    }
}

fn failure(code: PolicyAuthorityFailureWire) -> PolicyAuthorityResponseWire {
    PolicyAuthorityResponseWire {
        format_version: PROTOCOL_VERSION,
        failure: code as i32,
        staged_policy_record: None,
    }
}

pub(super) fn bind_private_socket(path: &Path) -> Result<UnixListener, PolicyAuthorityServerError> {
    let parent = path
        .parent()
        .ok_or(PolicyAuthorityServerError::Configuration)?;
    prepare_private_directory(parent)?;
    remove_safe_stale_socket(path, parent)?;
    let listener = UnixListener::bind(path).map_err(|_| PolicyAuthorityServerError::Listener)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))
        .map_err(|_| PolicyAuthorityServerError::Listener)?;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| PolicyAuthorityServerError::Listener)?;
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| PolicyAuthorityServerError::Listener)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(PolicyAuthorityServerError::Listener);
    }
    Ok(listener)
}

fn prepare_private_directory(path: &Path) -> Result<(), PolicyAuthorityServerError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => validate_private_directory(&metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(SOCKET_DIRECTORY_MODE);
            builder
                .create(path)
                .map_err(|_| PolicyAuthorityServerError::Configuration)?;
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|_| PolicyAuthorityServerError::Configuration)?;
            validate_private_directory(&metadata)
        }
        Err(_) => Err(PolicyAuthorityServerError::Configuration),
    }
}

fn validate_private_directory(
    metadata: &std::fs::Metadata,
) -> Result<(), PolicyAuthorityServerError> {
    if metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == SOCKET_DIRECTORY_MODE
    {
        Ok(())
    } else {
        Err(PolicyAuthorityServerError::Configuration)
    }
}

fn remove_safe_stale_socket(path: &Path, parent: &Path) -> Result<(), PolicyAuthorityServerError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(PolicyAuthorityServerError::Configuration),
    };
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| PolicyAuthorityServerError::Configuration)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(PolicyAuthorityServerError::Configuration);
    }
    std::fs::remove_file(path).map_err(|_| PolicyAuthorityServerError::Configuration)
}

fn required_path(name: &str) -> Result<PathBuf, PolicyAuthorityServerError> {
    let path =
        PathBuf::from(std::env::var(name).map_err(|_| PolicyAuthorityServerError::Configuration)?);
    if valid_absolute_path(&path) {
        Ok(path)
    } else {
        Err(PolicyAuthorityServerError::Configuration)
    }
}

#[derive(Debug)]
pub enum PolicyAuthorityServerError {
    Configuration,
    Listener,
}

impl std::fmt::Display for PolicyAuthorityServerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "policy-authority configuration is invalid",
            Self::Listener => "policy-authority local listener failed",
        })
    }
}

impl std::error::Error for PolicyAuthorityServerError {}
