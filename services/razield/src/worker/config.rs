use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::queue::QueueConfig;
use crate::service::config::{valid_private_root, valid_queue_prefix, valid_redis_url};

const REDIS_URL_ENV: &str = "RAZIEL_REDIS_URL";
const QUEUE_PREFIX_ENV: &str = "RAZIEL_QUEUE_PREFIX";
const PRIVATE_ENVELOPE_ROOT_ENV: &str = "RAZIEL_PRIVATE_ENVELOPE_ROOT";

/// Owner-controlled proof-supervisor settings validated before queue access.
#[derive(Clone)]
pub struct SupervisorConfig {
    queue_config: QueueConfig,
    queue_prefix: String,
    private_envelope_root: PathBuf,
}

impl SupervisorConfig {
    /// Reads the explicit supervisor contract from the process environment.
    pub fn from_env() -> Result<Self, SupervisorConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub(crate) fn from_lookup<F>(mut lookup: F) -> Result<Self, SupervisorConfigError>
    where
        F: FnMut(&'static str) -> Option<String>,
    {
        let redis_url = lookup(REDIS_URL_ENV).ok_or(SupervisorConfigError::InvalidRedisUrl)?;
        let queue_prefix =
            lookup(QUEUE_PREFIX_ENV).ok_or(SupervisorConfigError::InvalidQueuePrefix)?;
        let private_envelope_root = lookup(PRIVATE_ENVELOPE_ROOT_ENV)
            .ok_or(SupervisorConfigError::InvalidPrivateEnvelopeRoot)?;
        Self::from_values(&redis_url, &queue_prefix, &private_envelope_root)
    }

    /// Parses explicit values without opening storage or a Redis connection.
    pub fn from_values(
        redis_url: &str,
        queue_prefix: &str,
        private_envelope_root: &str,
    ) -> Result<Self, SupervisorConfigError> {
        if !valid_redis_url(redis_url) {
            return Err(SupervisorConfigError::InvalidRedisUrl);
        }
        if !valid_queue_prefix(queue_prefix) {
            return Err(SupervisorConfigError::InvalidQueuePrefix);
        }

        let private_envelope_root = PathBuf::from(private_envelope_root);
        if !valid_private_root(&private_envelope_root) {
            return Err(SupervisorConfigError::InvalidPrivateEnvelopeRoot);
        }

        let queue_config = QueueConfig::new(redis_url, queue_prefix)
            .map_err(|_| SupervisorConfigError::InvalidQueueConfiguration)?;
        Ok(Self {
            queue_config,
            queue_prefix: queue_prefix.to_owned(),
            private_envelope_root,
        })
    }

    /// Returns the public Redis key prefix, never the connection URL.
    pub fn queue_prefix(&self) -> &str {
        &self.queue_prefix
    }

    /// Returns the owner-only root used by the strict envelope store.
    pub fn private_envelope_root(&self) -> &Path {
        &self.private_envelope_root
    }

    pub(crate) fn queue_config(&self) -> &QueueConfig {
        &self.queue_config
    }
}

impl fmt::Debug for SupervisorConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupervisorConfig")
            .field("redis_url", &"<redacted>")
            .field("queue_prefix", &self.queue_prefix)
            .field("private_envelope_root", &"<redacted>")
            .finish()
    }
}

/// Invalid supervisor configuration detected before external state is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorConfigError {
    InvalidRedisUrl,
    InvalidQueuePrefix,
    InvalidPrivateEnvelopeRoot,
    InvalidQueueConfiguration,
}

impl fmt::Display for SupervisorConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRedisUrl => "invalid supervisor Redis URL",
            Self::InvalidQueuePrefix => "invalid supervisor BullMQ key prefix",
            Self::InvalidPrivateEnvelopeRoot => "invalid supervisor private-envelope root",
            Self::InvalidQueueConfiguration => "invalid supervisor queue configuration",
        })
    }
}

impl Error for SupervisorConfigError {}
