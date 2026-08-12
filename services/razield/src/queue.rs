//! BullMQ configuration shared by the capability API and proof-job supervisor.

use std::error::Error;
use std::fmt;

use bullmq::options::RedisConnectionOptions;
use bullmq::{JobOptions, QueueOptions, WorkerOptions};

/// The API and every proof worker use this queue-wide ceiling.
pub const GLOBAL_CONCURRENCY: u64 = 1;

/// One worker process can invoke at most one proof processor at a time.
pub const WORKER_CONCURRENCY: usize = 1;

/// Proof jobs never retry after failure, lock loss, timeout, or process exit.
pub const JOB_ATTEMPTS: u32 = 1;

/// BullMQ rejects a second processor invocation after an internal stalled-job claim.
pub const MAX_STARTED_ATTEMPTS: u32 = 1;

/// A stalled proof fails terminally instead of returning to the waiting set.
pub const MAX_STALLED_COUNT: u32 = 0;

/// Connection and keyspace settings for one owner-controlled BullMQ deployment.
#[derive(Clone)]
pub struct QueueConfig {
    redis_url: String,
    prefix: String,
}

impl QueueConfig {
    /// Validates queue connection settings without parsing or exposing credentials.
    pub fn new(
        redis_url: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Result<Self, QueueConfigError> {
        let redis_url = redis_url.into();
        if redis_url.trim().is_empty() {
            return Err(QueueConfigError::EmptyRedisUrl);
        }

        let prefix = prefix.into();
        if prefix.trim().is_empty() {
            return Err(QueueConfigError::EmptyPrefix);
        }

        Ok(Self { redis_url, prefix })
    }

    /// Builds options for publishers and queue-state readers.
    pub fn queue_options(&self) -> QueueOptions {
        QueueOptions::new()
            .connection(self.connection_options())
            .prefix(self.prefix.clone())
    }

    /// Builds the fail-once worker policy from ADR-0013.
    pub fn worker_options(&self) -> WorkerOptions {
        WorkerOptions::new()
            .connection(self.connection_options())
            .prefix(self.prefix.clone())
            .concurrency(WORKER_CONCURRENCY)
            .max_started_attempts(MAX_STARTED_ATTEMPTS)
            .max_stalled_count(MAX_STALLED_COUNT)
    }

    /// Builds the only attempt policy accepted for proof jobs.
    pub fn job_options(&self) -> JobOptions {
        JobOptions::new().attempts(JOB_ATTEMPTS)
    }

    fn connection_options(&self) -> RedisConnectionOptions {
        RedisConnectionOptions {
            url: self.redis_url.clone(),
            ..Default::default()
        }
    }
}

impl fmt::Debug for QueueConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueueConfig")
            .field("redis_url", &"[redacted]")
            .field("prefix", &self.prefix)
            .finish()
    }
}

/// Invalid queue configuration detected before a Redis connection is attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueConfigError {
    /// The Redis URL is absent.
    EmptyRedisUrl,
    /// The BullMQ key prefix is absent.
    EmptyPrefix,
}

impl fmt::Display for QueueConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRedisUrl => formatter.write_str("Redis URL must not be empty"),
            Self::EmptyPrefix => formatter.write_str("BullMQ key prefix must not be empty"),
        }
    }
}

impl Error for QueueConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_connection_fields() {
        assert_eq!(
            QueueConfig::new(" ", "raziel").unwrap_err(),
            QueueConfigError::EmptyRedisUrl
        );
        assert_eq!(
            QueueConfig::new("redis://example.invalid", " ").unwrap_err(),
            QueueConfigError::EmptyPrefix
        );
    }

    #[test]
    fn debug_output_redacts_redis_credentials() {
        let config = QueueConfig::new(
            "redis://raziel:private-password@example.invalid:6379/0",
            "raziel-test",
        )
        .expect("valid queue config");

        let debug = format!("{config:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("private-password"));
        assert!(!debug.contains("example.invalid"));
    }

    #[test]
    fn proof_jobs_and_workers_are_single_attempt() {
        let config =
            QueueConfig::new("redis://example.invalid", "raziel-test").expect("valid queue config");
        let job = config.job_options();
        let worker = config.worker_options();

        assert_eq!(job.attempts, Some(JOB_ATTEMPTS));
        assert_eq!(worker.concurrency, WORKER_CONCURRENCY);
        assert_eq!(worker.max_started_attempts, Some(MAX_STARTED_ATTEMPTS));
        assert_eq!(worker.max_stalled_count, MAX_STALLED_COUNT);
    }
}
