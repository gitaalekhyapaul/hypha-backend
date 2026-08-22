use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime};

use super::bullmq::{CapabilityQueue, QueueReadError};
use super::payload::CapabilityJobPayload;
use crate::private_envelope::{
    CleanupSummary, PrivateEnvelopeError, PrivateEnvelopeRef, PrivateEnvelopeStore,
};

/// Public counters from the owner-only startup reconciliation pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StartupReconciliationSummary {
    pub references_scanned: usize,
    pub live_references: usize,
    pub cleanup: CleanupSummary,
}

/// Redacted startup failure classes. No path, token, or Redis value is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupReconciliationError {
    PrivateEnvelope(PrivateEnvelopeError),
    QueueUnavailable,
    QueueRecordMismatch,
    InvalidTime,
    BlockingTaskFailed,
}

impl fmt::Display for StartupReconciliationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PrivateEnvelope(_) => "private-envelope startup reconciliation failed",
            Self::QueueUnavailable => "capability queue is unavailable during startup",
            Self::QueueRecordMismatch => "capability queue record failed startup validation",
            Self::InvalidTime => "startup reconciliation time is invalid",
            Self::BlockingTaskFailed => "startup reconciliation task failed",
        })
    }
}

impl Error for StartupReconciliationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PrivateEnvelope(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PrivateEnvelopeError> for StartupReconciliationError {
    fn from(error: PrivateEnvelopeError) -> Self {
        Self::PrivateEnvelope(error)
    }
}

pub(super) async fn reconcile_startup_with_queue<Q: CapabilityQueue + Sync>(
    store: PrivateEnvelopeStore,
    queue: &Q,
    now: SystemTime,
    grace_period: Duration,
) -> Result<StartupReconciliationSummary, StartupReconciliationError> {
    let scan_store = store.clone();
    let references = tokio::task::spawn_blocking(move || scan_store.scan_references())
        .await
        .map_err(|_| StartupReconciliationError::BlockingTaskFailed)??;

    let mut live_references = HashSet::new();
    for reference in &references {
        let job_id = reference.job_id();
        let expected_payload = CapabilityJobPayload::new(*reference);
        let retained = queue.get_job(&job_id).await.map_err(map_queue_error)?;
        let Some(retained) = retained else {
            continue;
        };
        if !retained.matches(&job_id, &expected_payload) {
            return Err(StartupReconciliationError::QueueRecordMismatch);
        }
        if retained.is_live().map_err(map_queue_error)? {
            live_references.insert(*reference);
        }
    }

    validate_live_envelopes(store.clone(), &live_references).await?;
    let cutoff = now
        .checked_sub(grace_period)
        .ok_or(StartupReconciliationError::InvalidTime)?;
    let cleanup_store = store;
    let cleanup_live_references = live_references.clone();
    let cleanup = tokio::task::spawn_blocking(move || {
        cleanup_store.cleanup_orphans(&cleanup_live_references, cutoff)
    })
    .await
    .map_err(|_| StartupReconciliationError::BlockingTaskFailed)??;

    Ok(StartupReconciliationSummary {
        references_scanned: references.len(),
        live_references: live_references.len(),
        cleanup,
    })
}

async fn validate_live_envelopes(
    store: PrivateEnvelopeStore,
    live_references: &HashSet<PrivateEnvelopeRef>,
) -> Result<(), StartupReconciliationError> {
    let live_references = live_references.iter().copied().collect::<Vec<_>>();
    tokio::task::spawn_blocking(move || {
        for reference in live_references {
            store.read(&reference)?;
        }
        Ok::<(), PrivateEnvelopeError>(())
    })
    .await
    .map_err(|_| StartupReconciliationError::BlockingTaskFailed)??;
    Ok(())
}

const fn map_queue_error(error: QueueReadError) -> StartupReconciliationError {
    match error {
        QueueReadError::CorruptRecord => StartupReconciliationError::QueueRecordMismatch,
        QueueReadError::Unavailable => StartupReconciliationError::QueueUnavailable,
    }
}
