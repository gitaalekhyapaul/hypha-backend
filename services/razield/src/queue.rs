//! BullMQ configuration shared by the capability API and proof-job supervisor.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use bullmq::options::{DeduplicationOptions, RedisConnectionOptions};
use bullmq::types::{BackoffStrategy, KeepJobs, RemoveOnFinish};
use bullmq::{JobOptions, QueueOptions, WorkerOptions};

/// BullMQ queue shared by the capability publisher and proof supervisor.
pub const CAPABILITY_QUEUE_NAME: &str = "proof-jobs";

/// Durable internal control queue from ADR-0017.
pub const CANCELLATION_QUEUE_NAME: &str = "cancellation-commands";

/// The API and every proof worker use this queue-wide ceiling.
pub const GLOBAL_CONCURRENCY: u64 = 1;

/// Atomic BullMQ slot held by the one nonterminal PoC proof job.
pub const PROOF_CAPACITY_DEDUPLICATION_ID: &str = "raziel-proof-capacity-v1";

/// One worker process can invoke at most one proof processor at a time.
pub const WORKER_CONCURRENCY: usize = 1;

/// Active-job lock lifetime renewed by the official BullMQ worker.
pub const WORKER_LOCK_DURATION: Duration = Duration::from_secs(30);

/// Renewal runs often enough to survive one delayed tick before lock expiry.
pub const WORKER_LOCK_RENEW_TIME: Duration = Duration::from_secs(10);

/// Lost locks enter BullMQ's stalled path after one lock-duration interval.
pub const WORKER_STALLED_INTERVAL: Duration = Duration::from_secs(30);

/// Empty queues are polled at a bounded whole-second interval.
pub const WORKER_DRAIN_DELAY: Duration = Duration::from_secs(5);

/// Proof jobs never retry after failure, lock loss, timeout, or process exit.
pub const JOB_ATTEMPTS: u32 = 1;

/// Control operations retry without ever retrying proof execution.
pub const CANCELLATION_JOB_ATTEMPTS: u32 = 3;
pub const CANCELLATION_RETRY_DELAY_MILLIS: u64 = 1_000;

/// BullMQ rejects a second processor invocation after an internal stalled-job claim.
pub const MAX_STARTED_ATTEMPTS: u32 = 1;

/// A stalled proof fails terminally instead of returning to the waiting set.
pub const MAX_STALLED_COUNT: u32 = 0;

/// The public queue handoff is deliberately much smaller than private ingress.
pub const MAX_PUBLIC_JOB_PAYLOAD_BYTES: usize = 256;

/// Retain terminal records long enough to cover the 75-minute capability window.
///
/// BullMQ's finish script multiplies this wire value by 1,000 before comparing it
/// with millisecond timestamps, so the value is expressed in seconds.
pub const TERMINAL_RETENTION_AGE_SECONDS: u64 = 24 * 60 * 60;

/// A second bound prevents an unbounded terminal Redis set.
pub const TERMINAL_RETENTION_COUNT: usize = 100;

/// One terminal transition can evict at most this many aged records.
pub const TERMINAL_RETENTION_EVICTION_LIMIT: usize = 100;

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
            .lock_duration(WORKER_LOCK_DURATION)
            .lock_renew_time(WORKER_LOCK_RENEW_TIME)
            .stalled_interval(WORKER_STALLED_INTERVAL)
            .drain_delay(WORKER_DRAIN_DELAY)
            .max_started_attempts(MAX_STARTED_ATTEMPTS)
            .max_stalled_count(MAX_STALLED_COUNT)
            .remove_on_complete(terminal_retention())
            .remove_on_fail(terminal_retention())
    }

    /// Builds the only attempt policy accepted for proof jobs.
    pub fn job_options(&self) -> JobOptions {
        retained_job_options().size_limit(MAX_PUBLIC_JOB_PAYLOAD_BYTES)
    }

    /// Builds the retryable, retained policy for public cancellation commands.
    pub fn cancellation_job_options(&self) -> JobOptions {
        retained_cancellation_job_options().size_limit(MAX_PUBLIC_JOB_PAYLOAD_BYTES)
    }

    /// Builds the one-at-a-time worker policy for idempotent cancellation commands.
    pub fn cancellation_worker_options(&self) -> WorkerOptions {
        WorkerOptions::new()
            .connection(self.connection_options())
            .prefix(self.prefix.clone())
            .concurrency(WORKER_CONCURRENCY)
            .lock_duration(WORKER_LOCK_DURATION)
            .lock_renew_time(WORKER_LOCK_RENEW_TIME)
            .stalled_interval(WORKER_STALLED_INTERVAL)
            .drain_delay(WORKER_DRAIN_DELAY)
            .max_started_attempts(CANCELLATION_JOB_ATTEMPTS)
            .max_stalled_count(CANCELLATION_JOB_ATTEMPTS - 1)
            .remove_on_complete(terminal_retention())
            .remove_on_fail(terminal_retention())
    }

    fn connection_options(&self) -> RedisConnectionOptions {
        RedisConnectionOptions {
            url: self.redis_url.clone(),
            ..Default::default()
        }
    }
}

fn terminal_retention() -> RemoveOnFinish {
    RemoveOnFinish::Options(KeepJobs {
        age: Some(TERMINAL_RETENTION_AGE_SECONDS),
        count: Some(TERMINAL_RETENTION_COUNT),
        limit: Some(TERMINAL_RETENTION_EVICTION_LIMIT),
    })
}

pub(crate) fn retained_job_options() -> JobOptions {
    JobOptions::new()
        .attempts(JOB_ATTEMPTS)
        .deduplication(proof_capacity_deduplication())
        .remove_on_complete(terminal_retention())
        .remove_on_fail(terminal_retention())
}

pub(crate) fn retained_cancellation_job_options() -> JobOptions {
    JobOptions::new()
        .attempts(CANCELLATION_JOB_ATTEMPTS)
        .backoff(BackoffStrategy::Fixed(CANCELLATION_RETRY_DELAY_MILLIS))
        .remove_on_complete(terminal_retention())
        .remove_on_fail(terminal_retention())
}

/// Accepts only the exact publisher-owned proof-job profile.
pub(crate) fn valid_claimed_job_options(options: &JobOptions) -> bool {
    options.delay.is_none()
        && options.priority.is_none()
        && options.attempts == Some(JOB_ATTEMPTS)
        && options.backoff.is_none()
        && options.lifo.is_none()
        && valid_terminal_retention(options.remove_on_complete.as_ref())
        && valid_terminal_retention(options.remove_on_fail.as_ref())
        && options.keep_logs.is_none()
        && options.job_id.is_none()
        && options.timestamp.is_none()
        && options.stack_trace_limit.is_none()
        && options.parent.is_none()
        && valid_proof_capacity_deduplication(options.deduplication.as_ref())
        && options.repeat.is_none()
        && options.prev_millis.is_none()
        && options.repeat_job_key.is_none()
        && options.fail_parent_on_failure.is_none()
        && options.ignore_dependency_on_failure.is_none()
        && options.remove_dependency_on_failure.is_none()
        && options.continue_parent_on_failure.is_none()
        && options.size_limit.is_none()
}

fn proof_capacity_deduplication() -> DeduplicationOptions {
    DeduplicationOptions {
        id: PROOF_CAPACITY_DEDUPLICATION_ID.to_owned(),
        ttl: None,
        extend: None,
        replace: None,
        keep_last_if_active: None,
    }
}

fn valid_proof_capacity_deduplication(options: Option<&DeduplicationOptions>) -> bool {
    matches!(
        options,
        Some(DeduplicationOptions {
            id,
            ttl: None,
            extend: None,
            replace: None,
            keep_last_if_active: None,
        }) if id == PROOF_CAPACITY_DEDUPLICATION_ID
    )
}

pub(crate) fn valid_cancellation_job_options(options: &JobOptions) -> bool {
    serde_json::to_value(options).ok()
        == serde_json::to_value(retained_cancellation_job_options()).ok()
}

fn valid_terminal_retention(policy: Option<&RemoveOnFinish>) -> bool {
    matches!(
        policy,
        Some(RemoveOnFinish::Options(KeepJobs {
            age: Some(TERMINAL_RETENTION_AGE_SECONDS),
            count: Some(TERMINAL_RETENTION_COUNT),
            limit: Some(TERMINAL_RETENTION_EVICTION_LIMIT),
        }))
    )
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
        assert_eq!(job.keep_logs, None);
        assert_eq!(job.stack_trace_limit, None);
        assert_eq!(job.size_limit, Some(MAX_PUBLIC_JOB_PAYLOAD_BYTES));
        assert!(valid_proof_capacity_deduplication(
            job.deduplication.as_ref()
        ));
        assert_terminal_retention(job.remove_on_complete.as_ref());
        assert_terminal_retention(job.remove_on_fail.as_ref());
        assert_terminal_retention(worker.remove_on_complete.as_ref());
        assert_terminal_retention(worker.remove_on_fail.as_ref());
        // BullMQ applies `job_id` and `size_limit` while publishing, then omits
        // both client-only fields from the Redis job options read by a worker.
        let stored_job = retained_job_options();
        assert!(valid_claimed_job_options(&stored_job));

        let mut repeated = stored_job;
        repeated.repeat_job_key = Some("not-accepted".into());
        assert!(!valid_claimed_job_options(&repeated));

        let mut wrong_capacity = retained_job_options();
        wrong_capacity.deduplication = Some(DeduplicationOptions {
            id: "another-capacity-slot".into(),
            ..Default::default()
        });
        assert!(!valid_claimed_job_options(&wrong_capacity));
    }

    #[test]
    fn cancellation_commands_retry_without_retrying_proofs() {
        let config =
            QueueConfig::new("redis://example.invalid", "raziel-test").expect("valid queue config");
        let job = config.cancellation_job_options();
        let worker = config.cancellation_worker_options();

        assert_eq!(job.attempts, Some(CANCELLATION_JOB_ATTEMPTS));
        assert!(matches!(
            job.backoff,
            Some(BackoffStrategy::Fixed(CANCELLATION_RETRY_DELAY_MILLIS))
        ));
        assert_eq!(worker.concurrency, WORKER_CONCURRENCY);
        assert_eq!(worker.max_started_attempts, Some(CANCELLATION_JOB_ATTEMPTS));
        assert_eq!(worker.max_stalled_count, CANCELLATION_JOB_ATTEMPTS - 1);
        assert!(valid_cancellation_job_options(
            &retained_cancellation_job_options()
        ));
        assert!(!valid_cancellation_job_options(&retained_job_options()));
    }

    fn assert_terminal_retention(policy: Option<&RemoveOnFinish>) {
        let Some(RemoveOnFinish::Options(keep)) = policy else {
            panic!("expected age-and-count terminal retention");
        };
        assert_eq!(keep.age, Some(TERMINAL_RETENTION_AGE_SECONDS));
        assert_eq!(keep.count, Some(TERMINAL_RETENTION_COUNT));
        assert_eq!(keep.limit, Some(TERMINAL_RETENTION_EVICTION_LIMIT));
    }
}
