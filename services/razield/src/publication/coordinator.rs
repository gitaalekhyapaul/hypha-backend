use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use tokio::sync::Mutex;

use super::CAPABILITY_JOB_NAME;
use super::bullmq::{
    BullMqCapabilityQueue, CapabilityQueue, QueueAddError, QueueReadError, RetainedQueueJob,
};
use super::payload::CapabilityJobPayload;
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

/// Public result of a durable capability-job publication.
#[derive(Clone, PartialEq, Eq)]
pub struct PublishedCapabilityJob {
    job_id: String,
    disposition: PublicationDisposition,
}

impl PublishedCapabilityJob {
    /// Returns the exact lowercase request commitment used as the custom job ID.
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Reports whether BullMQ created or retained the authoritative record.
    pub const fn disposition(&self) -> PublicationDisposition {
        self.disposition
    }
}

impl Debug for PublishedCapabilityJob {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedCapabilityJob")
            .field("job_id", &"<redacted>")
            .field("disposition", &self.disposition)
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
    if retained.id != job_id
        || retained.name != CAPABILITY_JOB_NAME
        || retained.payload.as_ref().ok() != Some(payload)
        || !retained.policy_matches
    {
        return Err(CapabilityPublicationError::QueueRecordMismatch);
    }

    Ok(PublishedCapabilityJob {
        job_id: retained.id,
        disposition,
    })
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
