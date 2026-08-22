use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Maximum decoded request size accepted by the Tonic service wrapper.
pub const MAX_SERVICE_MESSAGE_BYTES: usize = 2 * 1024 * 1024;

/// Newly staged envelopes receive this grace period during startup cleanup.
pub const ORPHAN_GRACE_PERIOD: Duration = Duration::from_secs(10 * 60);

const MAX_QUEUE_PREFIX_BYTES: usize = 64;
const BIND_ADDRESS_ENV: &str = "RAZIEL_BIND_ADDRESS";
const REDIS_URL_ENV: &str = "RAZIEL_REDIS_URL";
const QUEUE_PREFIX_ENV: &str = "RAZIEL_QUEUE_PREFIX";
const PRIVATE_ENVELOPE_ROOT_ENV: &str = "RAZIEL_PRIVATE_ENVELOPE_ROOT";

/// Owner-controlled runtime settings validated before service side effects begin.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceConfig {
    bind_address: SocketAddr,
    redis_url: String,
    queue_prefix: String,
    private_envelope_root: PathBuf,
}

impl ServiceConfig {
    /// Reads the explicit service contract from the process environment.
    pub fn from_env() -> Result<Self, ServiceConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub(crate) fn from_lookup<F>(mut lookup: F) -> Result<Self, ServiceConfigError>
    where
        F: FnMut(&'static str) -> Option<String>,
    {
        let bind_address =
            lookup(BIND_ADDRESS_ENV).ok_or(ServiceConfigError::InvalidBindAddress)?;
        let redis_url = lookup(REDIS_URL_ENV).ok_or(ServiceConfigError::InvalidRedisUrl)?;
        let queue_prefix =
            lookup(QUEUE_PREFIX_ENV).ok_or(ServiceConfigError::InvalidQueuePrefix)?;
        let private_envelope_root = lookup(PRIVATE_ENVELOPE_ROOT_ENV)
            .ok_or(ServiceConfigError::InvalidPrivateEnvelopeRoot)?;
        Self::from_values(
            &bind_address,
            &redis_url,
            &queue_prefix,
            &private_envelope_root,
        )
    }

    /// Parses explicit startup values without opening a listener or Redis connection.
    pub fn from_values(
        bind_address: &str,
        redis_url: &str,
        queue_prefix: &str,
        private_envelope_root: &str,
    ) -> Result<Self, ServiceConfigError> {
        let bind_address = bind_address
            .parse::<SocketAddr>()
            .map_err(|_| ServiceConfigError::InvalidBindAddress)?;
        if bind_address.port() == 0 {
            return Err(ServiceConfigError::InvalidBindAddress);
        }

        if !valid_redis_url(redis_url) {
            return Err(ServiceConfigError::InvalidRedisUrl);
        }
        if !valid_queue_prefix(queue_prefix) {
            return Err(ServiceConfigError::InvalidQueuePrefix);
        }

        let private_envelope_root = PathBuf::from(private_envelope_root);
        if !valid_private_root(&private_envelope_root) {
            return Err(ServiceConfigError::InvalidPrivateEnvelopeRoot);
        }

        Ok(Self {
            bind_address,
            redis_url: redis_url.to_owned(),
            queue_prefix: queue_prefix.to_owned(),
            private_envelope_root,
        })
    }

    pub fn bind_address(&self) -> SocketAddr {
        self.bind_address
    }

    pub fn queue_prefix(&self) -> &str {
        &self.queue_prefix
    }

    pub fn private_envelope_root(&self) -> &Path {
        &self.private_envelope_root
    }

    pub(crate) fn redis_url(&self) -> &str {
        &self.redis_url
    }
}

pub(crate) fn valid_redis_url(value: &str) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return false;
    }

    let authority = value
        .strip_prefix("redis://")
        .or_else(|| value.strip_prefix("rediss://"));
    matches!(authority, Some(authority) if !authority.is_empty() && !authority.starts_with('/'))
}

pub(crate) fn valid_queue_prefix(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_QUEUE_PREFIX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn valid_private_root(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && path.components().all(|component| {
            matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::Normal(_)
            )
        })
}

impl fmt::Debug for ServiceConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceConfig")
            .field("bind_address", &self.bind_address)
            .field("redis_url", &"<redacted>")
            .field("queue_prefix", &self.queue_prefix)
            .field("private_envelope_root", &"<redacted>")
            .finish()
    }
}

/// Invalid service configuration detected before external state is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceConfigError {
    InvalidBindAddress,
    InvalidRedisUrl,
    InvalidQueuePrefix,
    InvalidPrivateEnvelopeRoot,
}

impl fmt::Display for ServiceConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidBindAddress => "invalid service bind address",
            Self::InvalidRedisUrl => "invalid Redis URL",
            Self::InvalidQueuePrefix => "invalid BullMQ key prefix",
            Self::InvalidPrivateEnvelopeRoot => "invalid private-envelope root",
        };
        formatter.write_str(message)
    }
}

impl Error for ServiceConfigError {}
