use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use raziel_verifier::default_registry_config;

const SPICEDB_ENDPOINT_REF_ENV: &str = "RAZIEL_SPICEDB_ENDPOINT_REF";
const SPICEDB_ENDPOINT_ENV: &str = "RAZIEL_SPICEDB_ENDPOINT";
const SPICEDB_AUTH_TOKEN_PATH_ENV: &str = "RAZIEL_SPICEDB_AUTH_TOKEN_PATH";
const ZED_TOKEN_SECRET_REF_ENV: &str = "RAZIEL_ZED_TOKEN_SECRET_REF";
const ZED_TOKEN_PATH_ENV: &str = "RAZIEL_ZED_TOKEN_PATH";
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
    zed_token_secret_ref: String,
    zed_token_path: PathBuf,
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
        let zed_token_secret_ref = required_identifier(
            lookup(ZED_TOKEN_SECRET_REF_ENV),
            SnapshotServiceConfigError::InvalidZedTokenReference,
        )?;
        let zed_token_path = required_path(
            lookup(ZED_TOKEN_PATH_ENV),
            SnapshotServiceConfigError::InvalidZedTokenPath,
        )?;
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
            zed_token_secret_ref,
            zed_token_path,
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

    pub fn zed_token_secret_ref(&self) -> &str {
        &self.zed_token_secret_ref
    }

    pub fn zed_token_path(&self) -> &Path {
        &self.zed_token_path
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
