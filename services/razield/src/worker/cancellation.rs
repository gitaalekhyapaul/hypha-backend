use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bullmq::types::JobState;
use bullmq::worker::CancellationToken;
use bullmq::{Error as BullMqError, Job, Queue, Worker};
use serde_json::Value;
use tokio::sync::RwLock;

use super::processor::{EnvelopeAccess, validate_public_job};
use crate::private_envelope::{DeleteStatus, PrivateEnvelopeRef};
use crate::publication::{
    CANCELLATION_JOB_NAME, CancellationCommandPayload, CancellationCommandResult,
};
use crate::queue::{
    CANCELLATION_JOB_ATTEMPTS, MAX_PUBLIC_JOB_PAYLOAD_BYTES, valid_cancellation_job_options,
    valid_claimed_job_options,
};

const TARGET_POLL_INTERVAL: Duration = Duration::from_millis(100);
const TARGET_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(45);
const TARGET_SNAPSHOT_READ_ATTEMPTS: usize = 16;
const TARGET_SNAPSHOT_RETRY_DELAY: Duration = Duration::from_millis(10);
const PUBLIC_CANCELLATION_FAILURE: &str = "SERVICE_UNAVAILABLE";

/// In-process proof-job cancellation authority. Job identifiers never enter logs.
#[derive(Default)]
pub(crate) struct CancellationIntentRegistry {
    active: RwLock<HashSet<String>>,
}

impl CancellationIntentRegistry {
    pub(crate) async fn authorize(&self, job_id: &str) {
        self.active.write().await.insert(job_id.to_owned());
    }

    pub(crate) async fn revoke(&self, job_id: &str) {
        self.active.write().await.remove(job_id);
    }

    pub(crate) async fn take(&self, job_id: &str) -> bool {
        self.active.write().await.remove(job_id)
    }
}

pub(crate) struct CancellationCommandProcessor {
    store: Arc<dyn EnvelopeAccess>,
    proof_queue: Arc<Queue>,
    proof_worker: Arc<Worker>,
    intents: Arc<CancellationIntentRegistry>,
}

impl CancellationCommandProcessor {
    pub(crate) fn new(
        store: Arc<dyn EnvelopeAccess>,
        proof_queue: Arc<Queue>,
        proof_worker: Arc<Worker>,
        intents: Arc<CancellationIntentRegistry>,
    ) -> Self {
        Self {
            store,
            proof_queue,
            proof_worker,
            intents,
        }
    }

    pub(crate) async fn process(
        &self,
        job: &Job,
        cancellation: CancellationToken,
    ) -> Result<Value, BullMqError> {
        let payload = validate_command_job(job)?;
        let deadline = tokio::time::Instant::now() + TARGET_CONFIRMATION_TIMEOUT;

        loop {
            if cancellation.is_cancelled() || tokio::time::Instant::now() >= deadline {
                return Err(retryable_failure());
            }
            let Some((target, state, reference)) = self.read_target(&payload).await? else {
                self.cleanup(reference_from_job_id(payload.target_job_id())?)
                    .await?;
                return cancellation_result(&payload, current_unix_millis()?);
            };

            match state {
                JobState::Waiting | JobState::Delayed | JobState::Prioritized => {
                    let removed = self
                        .proof_queue
                        .remove(payload.target_job_id())
                        .await
                        .map_err(|_| retryable_failure())?;
                    if !removed {
                        tokio::task::yield_now().await;
                        continue;
                    }
                    self.cleanup(reference).await?;
                    return cancellation_result(&payload, current_unix_millis()?);
                }
                JobState::Active => {
                    self.intents.authorize(payload.target_job_id()).await;
                    if !self.proof_worker.cancel_job(payload.target_job_id()).await {
                        self.intents.revoke(payload.target_job_id()).await;
                        tokio::task::yield_now().await;
                        continue;
                    }
                    return self
                        .wait_for_active_terminal(&payload, cancellation, deadline)
                        .await;
                }
                JobState::Completed => {
                    return terminal_unchanged_result(
                        &payload,
                        target.finished_on().ok_or_else(invalid_command)?,
                    );
                }
                JobState::Failed if target.failed_reason() == "CANCELED" => {
                    return cancellation_result(
                        &payload,
                        target.finished_on().ok_or_else(invalid_command)?,
                    );
                }
                JobState::Failed => {
                    return terminal_unchanged_result(
                        &payload,
                        target.finished_on().ok_or_else(invalid_command)?,
                    );
                }
                JobState::WaitingChildren | JobState::Unknown => {
                    return Err(invalid_command());
                }
            }
        }
    }

    async fn wait_for_active_terminal(
        &self,
        payload: &CancellationCommandPayload,
        cancellation: CancellationToken,
        deadline: tokio::time::Instant,
    ) -> Result<Value, BullMqError> {
        loop {
            if cancellation.is_cancelled() || tokio::time::Instant::now() >= deadline {
                return Err(retryable_failure());
            }
            let Some((target, state, _reference)) = self.read_target(payload).await? else {
                return Err(retryable_failure());
            };
            match state {
                JobState::Active => {
                    tokio::time::sleep(TARGET_POLL_INTERVAL).await;
                }
                JobState::Failed if target.failed_reason() == "CANCELED" => {
                    return cancellation_result(
                        payload,
                        target.finished_on().ok_or_else(invalid_command)?,
                    );
                }
                JobState::Completed | JobState::Failed => {
                    self.intents.revoke(payload.target_job_id()).await;
                    return terminal_unchanged_result(
                        payload,
                        target.finished_on().ok_or_else(invalid_command)?,
                    );
                }
                JobState::Waiting | JobState::Delayed | JobState::Prioritized => {
                    return Err(retryable_failure());
                }
                JobState::WaitingChildren | JobState::Unknown => {
                    return Err(invalid_command());
                }
            }
        }
    }

    async fn read_target(
        &self,
        payload: &CancellationCommandPayload,
    ) -> Result<Option<(Job, JobState, PrivateEnvelopeRef)>, BullMqError> {
        for attempt in 1..=TARGET_SNAPSHOT_READ_ATTEMPTS {
            let target = self
                .proof_queue
                .get_job(payload.target_job_id())
                .await
                .map_err(|_| retryable_failure())?;
            let Some(target) = target else {
                return Ok(None);
            };
            let state = target.get_state().await.map_err(|_| retryable_failure())?;
            let reference = validate_target_job(&target, state, payload)?;
            if target_snapshot_matches_state(&target, state) {
                return Ok(Some((target, state, reference)));
            }
            if attempt < TARGET_SNAPSHOT_READ_ATTEMPTS {
                tokio::time::sleep(TARGET_SNAPSHOT_RETRY_DELAY).await;
            }
        }
        Err(retryable_failure())
    }

    async fn cleanup(&self, reference: PrivateEnvelopeRef) -> Result<(), BullMqError> {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || store.delete(&reference))
            .await
            .map_err(|_| retryable_failure())?
            .map(|status| match status {
                DeleteStatus::Deleted | DeleteStatus::NotFound => (),
            })
            .map_err(|_| retryable_failure())
    }
}

fn validate_command_job(job: &Job) -> Result<CancellationCommandPayload, BullMqError> {
    let payload_bytes = serde_json::to_vec(job.data()).map_err(|_| invalid_command())?;
    if job.name() != CANCELLATION_JOB_NAME
        || job.id().len() != 64
        || payload_bytes.len() > MAX_PUBLIC_JOB_PAYLOAD_BYTES
        || !valid_cancellation_job_options(job.opts())
        || !(1..=CANCELLATION_JOB_ATTEMPTS).contains(&job.attempts_started())
        || job.attempts_made() >= CANCELLATION_JOB_ATTEMPTS
        || job.parent().is_some()
        || job.parent_key().is_some()
        || job.repeat_job_key().is_some()
        || job.deferred_failure().is_some()
        || job.delay() != 0
    {
        return Err(invalid_command());
    }
    let payload =
        CancellationCommandPayload::from_json_value(job.data()).map_err(|_| invalid_command())?;
    if payload.target_job_id() != job.id() {
        return Err(invalid_command());
    }
    Ok(payload)
}

fn validate_target_job(
    job: &Job,
    state: JobState,
    payload: &CancellationCommandPayload,
) -> Result<PrivateEnvelopeRef, BullMqError> {
    let reference =
        validate_public_job(job.name(), job.id(), job.data()).map_err(|_| invalid_command())?;
    if job.id() != payload.target_job_id()
        || job.timestamp() != payload.target_submitted_at_unix_millis()
        || !valid_claimed_job_options(job.opts())
        || matches!(state, JobState::WaitingChildren | JobState::Unknown)
    {
        return Err(invalid_command());
    }
    Ok(reference)
}

fn target_snapshot_matches_state(job: &Job, state: JobState) -> bool {
    let has_processed = job.processed_on().is_some();
    let has_finished = job.finished_on().is_some();
    let has_failure = !job.failed_reason().is_empty();
    let has_result = !job.returnvalue().is_empty();

    match state {
        JobState::Waiting | JobState::Delayed | JobState::Prioritized => {
            !has_processed && !has_finished && !has_failure && !has_result
        }
        JobState::Active => has_processed && !has_finished && !has_failure && !has_result,
        JobState::Completed => has_processed && has_finished && !has_failure && has_result,
        JobState::Failed => has_processed && has_finished && has_failure && !has_result,
        JobState::WaitingChildren | JobState::Unknown => false,
    }
}

fn reference_from_job_id(job_id: &str) -> Result<PrivateEnvelopeRef, BullMqError> {
    PrivateEnvelopeRef::from_queue_token(&format!("pe1_{job_id}")).map_err(|_| invalid_command())
}

fn cancellation_result(
    payload: &CancellationCommandPayload,
    confirmed_at_unix_millis: u64,
) -> Result<Value, BullMqError> {
    CancellationCommandResult::canceled(
        payload.target_submitted_at_unix_millis(),
        confirmed_at_unix_millis,
    )
    .and_then(CancellationCommandResult::to_json_value)
    .map_err(|_| invalid_command())
}

fn terminal_unchanged_result(
    payload: &CancellationCommandPayload,
    confirmed_at_unix_millis: u64,
) -> Result<Value, BullMqError> {
    CancellationCommandResult::terminal_unchanged(
        payload.target_submitted_at_unix_millis(),
        confirmed_at_unix_millis,
    )
    .and_then(CancellationCommandResult::to_json_value)
    .map_err(|_| invalid_command())
}

fn current_unix_millis() -> Result<u64, BullMqError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .map_err(|_| retryable_failure())
}

fn invalid_command() -> BullMqError {
    BullMqError::Unrecoverable("INVALID_REQUEST".to_owned())
}

fn retryable_failure() -> BullMqError {
    BullMqError::ProcessingError(PUBLIC_CANCELLATION_FAILURE.to_owned())
}
