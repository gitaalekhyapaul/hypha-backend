use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use raziel_verifier::default_registry_config;

const SPICEDB_ENDPOINT_REF_ENV: &str = "RAZIEL_SPICEDB_ENDPOINT_REF";
const SPICEDB_ENDPOINT_ENV: &str = "RAZIEL_SPICEDB_ENDPOINT";
const SPICEDB_AUTH_TOKEN_PATH_ENV: &str = "RAZIEL_SPICEDB_AUTH_TOKEN_PATH";
const ZED_TOKEN_SECRET_REF_ENV: &str = "RAZIEL_ZED_TOKEN_SECRET_REF";
const ZED_TOKEN_PATH_ENV: &str = "RAZIEL_ZED_TOKEN_PATH";
const ZED_TOKEN_CANDIDATE_ROOT_ENV: &str = "RAZIEL_ZED_TOKEN_CANDIDATE_ROOT";
const PRIVATE_SNAPSHOT_ROOT_ENV: &str = "RAZIEL_PRIVATE_SNAPSHOT_ROOT";
const REGISTRY_WRITER_SOCKET_PATH_ENV: &str = "RAZIEL_REGISTRY_WRITER_SOCKET_PATH";
const REGISTRY_PATH_ENV: &str = "RAZIEL_REGISTRY_PATH";
const SNAPSHOT_KEY_PATH_ENV: &str = "RAZIEL_SNAPSHOT_KEY_PATH";
const TRUST_DOMAIN_ID_ENV: &str = "RAZIEL_TRUST_DOMAIN_ID";

#[derive(Clone, PartialEq, Eq)]
pub struct SnapshotServiceConfig {
    spicedb_endpoint_ref: String,
    spicedb_endpoint: String,
    spicedb_auth_token_path: PathBuf,
    zed_token_source: ZedTokenSourceConfig,
    private_snapshot_root: PathBuf,
    registry_writer_socket_path: PathBuf,
    registry_path: PathBuf,
    snapshot_key_path: PathBuf,
    trust_domain_id: String,
}

impl SnapshotServiceConfig {
    pub fn from_env() -> Result<Self, SnapshotServiceConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub(crate) fn from_lookup(
        mut lookup: impl FnMut(&'static str) -> Option<String>,
    ) -> Result<Self, SnapshotServiceConfigError> {
        let spicedb_endpoint_ref = required_identifier(
            lookup(SPICEDB_ENDPOINT_REF_ENV),
            SnapshotServiceConfigError::InvalidSpiceDbEndpointReference,
        )?;
        let spicedb_endpoint = lookup(SPICEDB_ENDPOINT_ENV)
            .filter(|value| valid_plaintext_endpoint(value))
            .ok_or(SnapshotServiceConfigError::InvalidSpiceDbEndpoint)?;
        let spicedb_auth_token_path = required_path(
            lookup(SPICEDB_AUTH_TOKEN_PATH_ENV),
            SnapshotServiceConfigError::InvalidSpiceDbTokenPath,
        )?;
        let zed_token_source = match (
            lookup(ZED_TOKEN_SECRET_REF_ENV),
            lookup(ZED_TOKEN_PATH_ENV),
            lookup(ZED_TOKEN_CANDIDATE_ROOT_ENV),
        ) {
            (Some(secret_ref), Some(path), None) => ZedTokenSourceConfig::Fixed {
                secret_ref: required_identifier(
                    Some(secret_ref),
                    SnapshotServiceConfigError::InvalidZedTokenReference,
                )?,
                path: required_path(Some(path), SnapshotServiceConfigError::InvalidZedTokenPath)?,
            },
            (None, None, Some(root)) => ZedTokenSourceConfig::CandidateRoot {
                root: required_path(
                    Some(root),
                    SnapshotServiceConfigError::InvalidZedTokenCandidateRoot,
                )?,
            },
            _ => return Err(SnapshotServiceConfigError::InvalidZedTokenSource),
        };
        let private_snapshot_root = required_path(
            lookup(PRIVATE_SNAPSHOT_ROOT_ENV),
            SnapshotServiceConfigError::InvalidPrivateSnapshotRoot,
        )?;
        let registry_writer_socket_path = required_path(
            lookup(REGISTRY_WRITER_SOCKET_PATH_ENV),
            SnapshotServiceConfigError::InvalidRegistryWriterSocket,
        )?;
        if registry_writer_socket_path
            .as_os_str()
            .as_encoded_bytes()
            .len()
            > 100
        {
            return Err(SnapshotServiceConfigError::InvalidRegistryWriterSocket);
        }
        let registry_path = required_path(
            lookup(REGISTRY_PATH_ENV),
            SnapshotServiceConfigError::InvalidRegistryPath,
        )?;
        let snapshot_key_path = required_path(
            lookup(SNAPSHOT_KEY_PATH_ENV),
            SnapshotServiceConfigError::InvalidSnapshotKeyPath,
        )?;
        let trust_domain_id = lookup(TRUST_DOMAIN_ID_ENV)
            .filter(|value| default_registry_config(value).is_ok())
            .ok_or(SnapshotServiceConfigError::InvalidTrustDomain)?;
        Ok(Self {
            spicedb_endpoint_ref,
            spicedb_endpoint,
            spicedb_auth_token_path,
            zed_token_source,
            private_snapshot_root,
            registry_writer_socket_path,
            registry_path,
            snapshot_key_path,
            trust_domain_id,
        })
    }

    pub fn spicedb_endpoint_ref(&self) -> &str {
        &self.spicedb_endpoint_ref
    }

    pub fn spicedb_endpoint(&self) -> &str {
        &self.spicedb_endpoint
    }

    pub fn spicedb_auth_token_path(&self) -> &Path {
        &self.spicedb_auth_token_path
    }

    pub fn fixed_zed_token_secret_ref(&self) -> Option<&str> {
        match &self.zed_token_source {
            ZedTokenSourceConfig::Fixed { secret_ref, .. } => Some(secret_ref),
            ZedTokenSourceConfig::CandidateRoot { .. } => None,
        }
    }

    pub fn candidate_revision_root(&self) -> Option<&Path> {
        match &self.zed_token_source {
            ZedTokenSourceConfig::Fixed { .. } => None,
            ZedTokenSourceConfig::CandidateRoot { root } => Some(root),
        }
    }

    pub(super) fn accepts_zed_token_reference(&self, value: &str) -> bool {
        match &self.zed_token_source {
            ZedTokenSourceConfig::Fixed { secret_ref, .. } => value == secret_ref,
            ZedTokenSourceConfig::CandidateRoot { .. } => valid_candidate_reference(value),
        }
    }

    pub(super) const fn zed_token_source(&self) -> &ZedTokenSourceConfig {
        &self.zed_token_source
    }

    pub fn private_snapshot_root(&self) -> &Path {
        &self.private_snapshot_root
    }

    pub fn registry_writer_socket_path(&self) -> &Path {
        &self.registry_writer_socket_path
    }

    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }

    pub fn snapshot_key_path(&self) -> &Path {
        &self.snapshot_key_path
    }

    pub fn trust_domain_id(&self) -> &str {
        &self.trust_domain_id
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum ZedTokenSourceConfig {
    Fixed { secret_ref: String, path: PathBuf },
    CandidateRoot { root: PathBuf },
}

fn required_identifier(
    value: Option<String>,
    error: SnapshotServiceConfigError,
) -> Result<String, SnapshotServiceConfigError> {
    value
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 256
                && value.is_ascii()
                && !value.bytes().any(|byte| byte.is_ascii_control())
        })
        .ok_or(error)
}

fn valid_candidate_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
}

fn required_path(
    value: Option<String>,
    error: SnapshotServiceConfigError,
) -> Result<PathBuf, SnapshotServiceConfigError> {
    let path = PathBuf::from(value.ok_or(error)?);
    if path.is_absolute()
        && path != Path::new("/")
        && path.components().all(|component| {
            matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::Normal(_)
            )
        })
    {
        Ok(path)
    } else {
        Err(error)
    }
}

fn valid_plaintext_endpoint(value: &str) -> bool {
    value.starts_with("http://")
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_whitespace())
        && value.len() <= 512
}

impl fmt::Debug for SnapshotServiceConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotServiceConfig")
            .field("owner_configuration", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotServiceConfigError {
    InvalidSpiceDbEndpointReference,
    InvalidSpiceDbEndpoint,
    InvalidSpiceDbTokenPath,
    InvalidZedTokenReference,
    InvalidZedTokenPath,
    InvalidZedTokenCandidateRoot,
    InvalidZedTokenSource,
    InvalidPrivateSnapshotRoot,
    InvalidRegistryWriterSocket,
    InvalidRegistryPath,
    InvalidSnapshotKeyPath,
    InvalidTrustDomain,
}

impl fmt::Display for SnapshotServiceConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid snapshot service configuration")
    }
}

impl Error for SnapshotServiceConfigError {}
