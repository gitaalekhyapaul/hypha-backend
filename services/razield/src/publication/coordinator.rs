use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use bullmq::types::JobState;
use tokio::sync::Mutex;

use super::bullmq::{
    BullMqCapabilityQueue, CapabilityQueue, QueueAddError, QueueReadError, RetainedQueueJob,
};
use super::payload::CapabilityJobPayload;
use super::reconciliation::{
    StartupReconciliationError, StartupReconciliationSummary, reconcile_startup_with_queue,
};
use crate::admission::AdmittedCapabilityRequest;
use crate::private_envelope::{
    DeleteStatus, PrivateEnvelopeError, PrivateEnvelopeStore, PublicationStage,
};
use crate::queue::QueueConfig;

/// Whether this call created a BullMQ record or returned one already retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationDisposition {
    Created,
    Retained,
}

/// Public lifecycle states available from the retained BullMQ record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicCapabilityJobState {
    Queued,
    Executing,
}

/// Public result of a durable capability-job publication.
#[derive(Clone, PartialEq, Eq)]
pub struct PublishedCapabilityJob {
    job_id: String,
    disposition: PublicationDisposition,
    state: PublicCapabilityJobState,
    submitted_at_unix_millis: u64,
    updated_at_unix_millis: u64,
}

impl PublishedCapabilityJob {
    pub(crate) fn from_authoritative_fields(
        job_id: String,
        disposition: PublicationDisposition,
        state: PublicCapabilityJobState,
        submitted_at_unix_millis: u64,
        updated_at_unix_millis: u64,
    ) -> Self {
        Self {
            job_id,
            disposition,
            state,
            submitted_at_unix_millis,
            updated_at_unix_millis,
        }
    }

    /// Returns the exact lowercase request commitment used as the custom job ID.
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Reports whether BullMQ created or retained the authoritative record.
    pub const fn disposition(&self) -> PublicationDisposition {
        self.disposition
    }

    pub const fn state(&self) -> PublicCapabilityJobState {
        self.state
    }

    pub const fn submitted_at_unix_millis(&self) -> u64 {
        self.submitted_at_unix_millis
    }

    pub const fn updated_at_unix_millis(&self) -> u64 {
        self.updated_at_unix_millis
    }
}

impl Debug for PublishedCapabilityJob {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedCapabilityJob")
            .field("job_id", &"<redacted>")
            .field("disposition", &self.disposition)
            .field("state", &self.state)
            .field("submitted_at", &"<public timestamp>")
            .field("updated_at", &"<public timestamp>")
            .finish()
    }
}

/// Redacted failure classes for later RPC and private-audit mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityPublicationError {
    PrivateEnvelope(PrivateEnvelopeError),
    QueueUnavailable,
    QueuePublicationRejected,
    QueuePublicationUncertain,
    QueueRecordMissing,
    QueueRecordMismatch,
    QueueTerminalResultUnavailable,
    EnvelopeCleanupFailed,
}

impl fmt::Display for CapabilityPublicationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PrivateEnvelope(_) => "private capability envelope failed",
            Self::QueueUnavailable => "capability queue is unavailable",
            Self::QueuePublicationRejected => "capability job publication was rejected",
            Self::QueuePublicationUncertain => "capability job publication is uncertain",
            Self::QueueRecordMissing => "published capability job is missing",
            Self::QueueRecordMismatch => "retained capability job does not match",
            Self::QueueTerminalResultUnavailable => {
                "retained capability job terminal result is unavailable"
            }
            Self::EnvelopeCleanupFailed => "private capability envelope cleanup failed",
        })
    }
}

impl Error for CapabilityPublicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PrivateEnvelope(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PrivateEnvelopeError> for CapabilityPublicationError {
    fn from(error: PrivateEnvelopeError) -> Self {
        Self::PrivateEnvelope(error)
    }
}

/// Serializes one process's filesystem-to-Redis publication transactions.
pub struct CapabilityJobPublisher {
    store: PrivateEnvelopeStore,
    queue: BullMqCapabilityQueue,
    publication_lock: Mutex<()>,
}

impl CapabilityJobPublisher {
    /// Connects to the approved BullMQ queue and applies global concurrency one.
    pub async fn connect(
        store: PrivateEnvelopeStore,
        config: &QueueConfig,
    ) -> Result<Self, CapabilityPublicationError> {
        let queue = BullMqCapabilityQueue::connect(config)
            .await
            .map_err(map_connect_error)?;
        Ok(Self {
            store,
            queue,
            publication_lock: Mutex::new(()),
        })
    }

    /// Durably stages one admitted request before publishing its public queue token.
    pub async fn publish(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
        let _guard = self.publication_lock.lock().await;
        publish_transaction(&self.store, &self.queue, admitted).await
    }

    /// Reconciles private files against exact authoritative BullMQ records.
    pub async fn reconcile_startup(
        &self,
        now: std::time::SystemTime,
        grace_period: std::time::Duration,
    ) -> Result<StartupReconciliationSummary, StartupReconciliationError> {
        reconcile_startup_with_queue(self.store.clone(), &self.queue, now, grace_period).await
    }

    /// Closes the BullMQ connection after all owners stop publishing.
    pub async fn close(self) {
        self.queue.close().await;
    }
}

impl Debug for CapabilityJobPublisher {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityJobPublisher")
            .field("private_store", &"<redacted>")
            .field("queue", &"<redacted>")
            .finish()
    }
}

pub(super) async fn publish_transaction<Q: CapabilityQueue>(
    store: &PrivateEnvelopeStore,
    queue: &Q,
    admitted: &AdmittedCapabilityRequest,
) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
    let stage = store.stage_for_publication(admitted)?;
    let payload = CapabilityJobPayload::new(stage.reference());
    let job_id = admitted.job_id();

    match queue.get_job(job_id).await {
        Ok(Some(retained)) => {
            return validate_retained(retained, job_id, &payload, PublicationDisposition::Retained);
        }
        // Another publisher owns this envelope and may still be committing Redis.
        Ok(None) if !stage.was_created() => {
            return Err(CapabilityPublicationError::QueuePublicationUncertain);
        }
        Ok(None) => {}
        Err(QueueReadError::CorruptRecord) => {
            return Err(CapabilityPublicationError::QueueRecordMismatch);
        }
        Err(QueueReadError::Unavailable) => {
            if stage.was_created() {
                rollback_created(store, stage)?;
                return Err(CapabilityPublicationError::QueueUnavailable);
            }
            return Err(CapabilityPublicationError::QueuePublicationUncertain);
        }
    }

    match queue.add_job(job_id, &payload).await {
        Ok(()) => validate_after_add(store, queue, stage, job_id, &payload).await,
        Err(QueueAddError::Rejected) => {
            rollback_created(store, stage)?;
            Err(CapabilityPublicationError::QueuePublicationRejected)
        }
        Err(QueueAddError::Uncertain) => {
            reconcile_uncertain_add(store, queue, stage, job_id, &payload).await
        }
    }
}

async fn validate_after_add<Q: CapabilityQueue>(
    store: &PrivateEnvelopeStore,
    queue: &Q,
    stage: PublicationStage,
    job_id: &str,
    payload: &CapabilityJobPayload,
) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
    match queue.get_job(job_id).await {
        Ok(Some(retained)) => {
            validate_retained(retained, job_id, payload, PublicationDisposition::Created)
        }
        Ok(None) => {
            rollback_created(store, stage)?;
            Err(CapabilityPublicationError::QueueRecordMissing)
        }
        // Redis has a record, so preserve the envelope for audit and repair.
        Err(QueueReadError::CorruptRecord) => Err(CapabilityPublicationError::QueueRecordMismatch),
        Err(QueueReadError::Unavailable) => {
            Err(CapabilityPublicationError::QueuePublicationUncertain)
        }
    }
}

async fn reconcile_uncertain_add<Q: CapabilityQueue>(
    store: &PrivateEnvelopeStore,
    queue: &Q,
    stage: PublicationStage,
    job_id: &str,
    payload: &CapabilityJobPayload,
) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
    match queue.get_job(job_id).await {
        Ok(Some(retained)) => {
            validate_retained(retained, job_id, payload, PublicationDisposition::Retained)
        }
        Ok(None) => {
            rollback_created(store, stage)?;
            Err(CapabilityPublicationError::QueuePublicationRejected)
        }
        Err(QueueReadError::CorruptRecord) => Err(CapabilityPublicationError::QueueRecordMismatch),
        Err(QueueReadError::Unavailable) => {
            Err(CapabilityPublicationError::QueuePublicationUncertain)
        }
    }
}

fn validate_retained(
    retained: RetainedQueueJob,
    job_id: &str,
    payload: &CapabilityJobPayload,
    disposition: PublicationDisposition,
) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
    if !retained.matches(job_id, payload) {
        return Err(CapabilityPublicationError::QueueRecordMismatch);
    }

    let state = match retained.state {
        JobState::Waiting | JobState::Delayed | JobState::Prioritized => {
            PublicCapabilityJobState::Queued
        }
        JobState::Active => PublicCapabilityJobState::Executing,
        JobState::Completed | JobState::Failed => {
            return Err(CapabilityPublicationError::QueueTerminalResultUnavailable);
        }
        JobState::WaitingChildren | JobState::Unknown => {
            return Err(CapabilityPublicationError::QueueRecordMismatch);
        }
    };
    let updated_at_unix_millis = retained
        .finished_on_ms
        .or(retained.processed_on_ms)
        .unwrap_or(retained.timestamp_ms);
    if updated_at_unix_millis < retained.timestamp_ms {
        return Err(CapabilityPublicationError::QueueRecordMismatch);
    }

    Ok(PublishedCapabilityJob::from_authoritative_fields(
        retained.id,
        disposition,
        state,
        retained.timestamp_ms,
        updated_at_unix_millis,
    ))
}

fn rollback_created(
    store: &PrivateEnvelopeStore,
    stage: PublicationStage,
) -> Result<(), CapabilityPublicationError> {
    if !stage.was_created() {
        return Ok(());
    }
    match store.delete(&stage.reference()) {
        Ok(DeleteStatus::Deleted | DeleteStatus::NotFound) => Ok(()),
        Err(_) => Err(CapabilityPublicationError::EnvelopeCleanupFailed),
    }
}

const fn map_connect_error(_error: QueueReadError) -> CapabilityPublicationError {
    CapabilityPublicationError::QueueUnavailable
}
