use std::error::Error;
use std::fmt::{self, Debug, Formatter};
use std::time::Duration;

use tokio::sync::Mutex;

use super::bullmq::{
    BullMqCapabilityQueue, CapabilityQueue, QueueAddError, QueueAddOutcome, QueueReadError,
    RetainedQueueJob,
};
use super::cancellation::{
    BullMqCancellationQueue, CancellationCommandOutcome, CancellationCommandPayload,
    RetainedCancellationCommand,
};
use super::lifecycle::{
    CapabilityLifecycleError, PublicCapabilityJobState, RetainedCapabilityJob,
    map_queue_read_error, project_proof_job, validate_job_id,
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

const LIFECYCLE_READ_ATTEMPTS: usize = 16;
const LIFECYCLE_READ_RETRY_DELAY: Duration = Duration::from_millis(10);

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
    QueueCapacityOccupied,
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
            Self::QueueCapacityOccupied => "proof capacity is occupied",
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
    cancellations: BullMqCancellationQueue,
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
        let cancellations = match BullMqCancellationQueue::connect(config).await {
            Ok(queue) => queue,
            Err(error) => {
                queue.close().await;
                return Err(map_connect_error(error));
            }
        };
        Ok(Self {
            store,
            queue,
            cancellations,
            publication_lock: Mutex::new(()),
        })
    }

    /// Durably stages one admitted request before publishing its public queue token.
    pub async fn publish(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
        let _guard = self.publication_lock.lock().await;
        match self.queue.get_job(admitted.job_id()).await {
            Ok(Some(_)) => {}
            Ok(None) => match self.queue.capacity_holder().await {
                Ok(Some(holder)) if holder == admitted.job_id() => {
                    return Err(CapabilityPublicationError::QueueRecordMissing);
                }
                Ok(Some(_)) => return Err(CapabilityPublicationError::QueueCapacityOccupied),
                Ok(None) => {}
                Err(QueueReadError::CorruptRecord) => {
                    return Err(CapabilityPublicationError::QueueRecordMismatch);
                }
                Err(QueueReadError::Unavailable) => {
                    return Err(CapabilityPublicationError::QueueUnavailable);
                }
            },
            Err(QueueReadError::CorruptRecord) => {
                return Err(CapabilityPublicationError::QueueRecordMismatch);
            }
            Err(QueueReadError::Unavailable) => {
                return Err(CapabilityPublicationError::QueueUnavailable);
            }
        }
        publish_transaction(&self.store, &self.queue, admitted).await
    }

    /// Reads one authoritative public lifecycle projection without opening its envelope.
    pub async fn get(
        &self,
        job_id: &str,
    ) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        validate_job_id(job_id)?;
        for attempt in 1..=LIFECYCLE_READ_ATTEMPTS {
            let result = self.get_once(job_id).await;
            if result != Err(CapabilityLifecycleError::QueueRecordMismatch)
                || attempt == LIFECYCLE_READ_ATTEMPTS
            {
                return result;
            }
            tokio::time::sleep(LIFECYCLE_READ_RETRY_DELAY).await;
        }
        unreachable!("bounded lifecycle read loop always returns")
    }

    async fn get_once(
        &self,
        job_id: &str,
    ) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        let proof = self
            .queue
            .get_job(job_id)
            .await
            .map_err(map_queue_read_error)?;
        let cancellation = self
            .cancellations
            .get(job_id)
            .await
            .map_err(map_queue_read_error)?;
        project_lifecycle(job_id, proof, cancellation)
    }

    /// Publishes one idempotent cancellation command and returns the current state.
    pub async fn cancel(
        &self,
        job_id: &str,
    ) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        validate_job_id(job_id)?;
        let _guard = self.publication_lock.lock().await;
        let current = self.get(job_id).await?;
        if current.is_terminal() {
            return Ok(current);
        }

        let payload =
            CancellationCommandPayload::new(job_id.to_owned(), current.submitted_at_unix_millis());
        if !read_valid_cancellation(&self.cancellations, &payload).await? {
            publish_cancellation(&self.cancellations, &payload).await?;
        }
        self.get(job_id).await
    }

    /// Reconciles private files against exact authoritative BullMQ records.
    pub async fn reconcile_startup(
        &self,
        now: std::time::SystemTime,
        grace_period: std::time::Duration,
    ) -> Result<StartupReconciliationSummary, StartupReconciliationError> {
        self.cleanup_canceled_envelopes().await?;
        reconcile_startup_with_queue(self.store.clone(), &self.queue, now, grace_period).await
    }

    async fn cleanup_canceled_envelopes(&self) -> Result<(), StartupReconciliationError> {
        let scan_store = self.store.clone();
        let references = tokio::task::spawn_blocking(move || scan_store.scan_references())
            .await
            .map_err(|_| StartupReconciliationError::BlockingTaskFailed)??;
        let mut canceled = Vec::new();
        for reference in references {
            match self.get(&reference.job_id()).await {
                Ok(job) if job.state() == PublicCapabilityJobState::Canceled => {
                    canceled.push(reference);
                }
                Ok(_) | Err(CapabilityLifecycleError::NotFound) => {}
                Err(CapabilityLifecycleError::QueueUnavailable) => {
                    return Err(StartupReconciliationError::QueueUnavailable);
                }
                Err(_) => return Err(StartupReconciliationError::QueueRecordMismatch),
            }
        }

        let cleanup_store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            for reference in canceled {
                cleanup_store.delete(&reference)?;
            }
            Ok::<(), PrivateEnvelopeError>(())
        })
        .await
        .map_err(|_| StartupReconciliationError::BlockingTaskFailed)??;
        Ok(())
    }

    /// Closes the BullMQ connection after all owners stop publishing.
    pub async fn close(self) {
        self.cancellations.close().await;
        self.queue.close().await;
    }
}

impl Debug for CapabilityJobPublisher {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityJobPublisher")
            .field("private_store", &"<redacted>")
            .field("queue", &"<redacted>")
            .field("cancellations", &"<redacted>")
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
        Ok(QueueAddOutcome::Added) => {
            validate_after_add(store, queue, stage, job_id, &payload).await
        }
        Ok(QueueAddOutcome::CapacityOccupied) => {
            rollback_created(store, stage)?;
            Err(CapabilityPublicationError::QueueCapacityOccupied)
        }
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
    if matches!(
        retained.state,
        bullmq::types::JobState::Completed | bullmq::types::JobState::Failed
    ) {
        return Err(CapabilityPublicationError::QueueTerminalResultUnavailable);
    }

    let projected =
        project_proof_job(retained).map_err(|_| CapabilityPublicationError::QueueRecordMismatch)?;

    Ok(PublishedCapabilityJob::from_authoritative_fields(
        projected.job_id().to_owned(),
        disposition,
        projected.state(),
        projected.submitted_at_unix_millis(),
        projected.updated_at_unix_millis(),
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

async fn publish_cancellation(
    queue: &BullMqCancellationQueue,
    payload: &CancellationCommandPayload,
) -> Result<(), CapabilityLifecycleError> {
    match queue.add(payload).await {
        Ok(()) => match read_valid_cancellation(queue, payload).await? {
            true => Ok(()),
            false => Err(CapabilityLifecycleError::CancellationPublicationRejected),
        },
        Err(QueueAddError::Rejected) => {
            Err(CapabilityLifecycleError::CancellationPublicationRejected)
        }
        Err(QueueAddError::Uncertain) => match read_valid_cancellation(queue, payload).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(CapabilityLifecycleError::CancellationPublicationRejected),
            Err(CapabilityLifecycleError::QueueUnavailable) => {
                Err(CapabilityLifecycleError::CancellationPublicationUncertain)
            }
            Err(error) => Err(error),
        },
    }
}

async fn read_valid_cancellation(
    queue: &BullMqCancellationQueue,
    payload: &CancellationCommandPayload,
) -> Result<bool, CapabilityLifecycleError> {
    for attempt in 1..=LIFECYCLE_READ_ATTEMPTS {
        let retained = queue
            .get(payload.target_job_id())
            .await
            .map_err(map_queue_read_error)?;
        let Some(retained) = retained else {
            return Ok(false);
        };
        match validate_retained_cancellation(&retained, payload) {
            Ok(()) => return Ok(true),
            Err(CapabilityLifecycleError::QueueRecordMismatch)
                if attempt < LIFECYCLE_READ_ATTEMPTS =>
            {
                tokio::time::sleep(LIFECYCLE_READ_RETRY_DELAY).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("bounded cancellation read loop always returns")
}

fn project_lifecycle(
    job_id: &str,
    proof: Option<RetainedQueueJob>,
    cancellation: Option<RetainedCancellationCommand>,
) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
    let projected_proof = proof.map(project_proof_job).transpose()?;
    let projected_cancellation = cancellation
        .as_ref()
        .map(|record| project_cancellation(job_id, record))
        .transpose()?;

    match (projected_proof, projected_cancellation) {
        (None, None) => Err(CapabilityLifecycleError::NotFound),
        (Some(proof), None) => {
            if proof.state() == PublicCapabilityJobState::Canceled {
                return Err(CapabilityLifecycleError::QueueRecordMismatch);
            }
            Ok(proof)
        }
        (
            None,
            Some(CancellationProjection::Pending {
                submitted_at,
                updated_at,
            }),
        ) => Ok(RetainedCapabilityJob::new_nonterminal(
            job_id.to_owned(),
            PublicCapabilityJobState::Queued,
            submitted_at,
            updated_at,
        )),
        (
            None,
            Some(CancellationProjection::Canceled {
                submitted_at,
                updated_at,
            }),
        ) => Ok(RetainedCapabilityJob::new_canceled(
            job_id.to_owned(),
            submitted_at,
            updated_at,
        )),
        (None, Some(CancellationProjection::TerminalUnchanged)) => {
            Err(CapabilityLifecycleError::QueueRecordMismatch)
        }
        (None, Some(CancellationProjection::Failed)) => {
            Err(CapabilityLifecycleError::QueueUnavailable)
        }
        (Some(proof), Some(CancellationProjection::Pending { submitted_at, .. })) => {
            if proof.submitted_at_unix_millis() != submitted_at {
                return Err(CapabilityLifecycleError::QueueRecordMismatch);
            }
            Ok(proof)
        }
        (Some(proof), Some(CancellationProjection::Canceled { submitted_at, .. })) => {
            if proof.submitted_at_unix_millis() != submitted_at
                || proof.state() != PublicCapabilityJobState::Canceled
            {
                return Err(CapabilityLifecycleError::QueueRecordMismatch);
            }
            Ok(proof)
        }
        (Some(proof), Some(CancellationProjection::TerminalUnchanged)) => {
            if !proof.is_terminal() || proof.state() == PublicCapabilityJobState::Canceled {
                return Err(CapabilityLifecycleError::QueueRecordMismatch);
            }
            Ok(proof)
        }
        (Some(proof), Some(CancellationProjection::Failed)) => Ok(proof),
    }
}

enum CancellationProjection {
    Pending { submitted_at: u64, updated_at: u64 },
    Canceled { submitted_at: u64, updated_at: u64 },
    TerminalUnchanged,
    Failed,
}

fn project_cancellation(
    job_id: &str,
    retained: &RetainedCancellationCommand,
) -> Result<CancellationProjection, CapabilityLifecycleError> {
    let payload = retained
        .payload
        .as_ref()
        .map_err(|_| CapabilityLifecycleError::QueueRecordMismatch)?;
    if payload.target_job_id() != job_id || !retained.matches(payload) {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }
    if retained.timestamp_ms < payload.target_submitted_at_unix_millis() {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }
    if retained.state == bullmq::types::JobState::Failed {
        let finished_at = retained
            .finished_on_ms
            .ok_or(CapabilityLifecycleError::QueueRecordMismatch)?;
        if retained.processed_on_ms.is_none()
            || finished_at < retained.timestamp_ms
            || retained.failed_reason != "SERVICE_UNAVAILABLE"
            || !retained.return_value.is_empty()
        {
            return Err(CapabilityLifecycleError::QueueRecordMismatch);
        }
        return Ok(CancellationProjection::Failed);
    }
    let result = retained
        .terminal_result()
        .map_err(|_| CapabilityLifecycleError::QueueRecordMismatch)?;
    let Some(result) = result else {
        let updated_at = retained.processed_on_ms.unwrap_or(retained.timestamp_ms);
        if updated_at < retained.timestamp_ms {
            return Err(CapabilityLifecycleError::QueueRecordMismatch);
        }
        return Ok(CancellationProjection::Pending {
            submitted_at: payload.target_submitted_at_unix_millis(),
            updated_at,
        });
    };
    if result.target_submitted_at_unix_millis() != payload.target_submitted_at_unix_millis()
        || retained
            .finished_on_ms
            .is_none_or(|finished| finished < result.confirmed_at_unix_millis())
    {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }
    match result.outcome() {
        CancellationCommandOutcome::Canceled => Ok(CancellationProjection::Canceled {
            submitted_at: result.target_submitted_at_unix_millis(),
            updated_at: result.confirmed_at_unix_millis(),
        }),
        CancellationCommandOutcome::TerminalUnchanged => {
            Ok(CancellationProjection::TerminalUnchanged)
        }
    }
}

fn validate_retained_cancellation(
    retained: &RetainedCancellationCommand,
    expected: &CancellationCommandPayload,
) -> Result<(), CapabilityLifecycleError> {
    if !retained.matches(expected) {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }
    if retained.state == bullmq::types::JobState::Failed {
        return Err(CapabilityLifecycleError::QueueUnavailable);
    }
    project_cancellation(expected.target_job_id(), retained).map(|_| ())
}
