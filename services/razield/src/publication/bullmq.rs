use bullmq::types::JobState;
use bullmq::{Error as BullMqError, Job, JobOptions, JobProgress, Queue};

use super::CAPABILITY_JOB_NAME;
use super::payload::{CapabilityJobPayload, PayloadError};
use crate::queue::{GLOBAL_CONCURRENCY, QueueConfig, retained_job_options};

pub(super) struct BullMqCapabilityQueue {
    queue: Queue,
    job_options: JobOptions,
}

#[derive(Clone)]
pub(super) struct RetainedQueueJob {
    pub id: String,
    pub name: String,
    pub payload: Result<CapabilityJobPayload, PayloadError>,
    pub policy_matches: bool,
    pub state: JobState,
    pub timestamp_ms: u64,
    pub processed_on_ms: Option<u64>,
    pub finished_on_ms: Option<u64>,
    pub progress: JobProgress,
    pub failed_reason: String,
    pub return_value: String,
}

impl RetainedQueueJob {
    pub(super) fn matches(&self, job_id: &str, payload: &CapabilityJobPayload) -> bool {
        self.id == job_id
            && self.name == CAPABILITY_JOB_NAME
            && self.payload.as_ref().ok() == Some(payload)
            && self.policy_matches
    }

    pub(super) const fn is_live(&self) -> Result<bool, QueueReadError> {
        match self.state {
            JobState::Waiting | JobState::Active | JobState::Delayed | JobState::Prioritized => {
                Ok(true)
            }
            JobState::Completed | JobState::Failed => Ok(false),
            JobState::WaitingChildren | JobState::Unknown => Err(QueueReadError::CorruptRecord),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueueReadError {
    CorruptRecord,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueueAddError {
    Rejected,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueueAddOutcome {
    Added,
    CapacityOccupied,
}

pub(super) trait CapabilityQueue {
    async fn get_job(&self, job_id: &str) -> Result<Option<RetainedQueueJob>, QueueReadError>;

    async fn add_job(
        &self,
        job_id: &str,
        payload: &CapabilityJobPayload,
    ) -> Result<QueueAddOutcome, QueueAddError>;
}

impl BullMqCapabilityQueue {
    pub(super) async fn connect(config: &QueueConfig) -> Result<Self, QueueReadError> {
        let queue =
            Queue::with_options(crate::queue::CAPABILITY_QUEUE_NAME, config.queue_options())
                .await
                .map_err(map_read_error)?;
        queue
            .set_global_concurrency(GLOBAL_CONCURRENCY)
            .await
            .map_err(map_read_error)?;
        Ok(Self {
            queue,
            job_options: config.job_options(),
        })
    }

    pub(super) async fn close(&self) {
        self.queue.close().await;
    }

    pub(super) async fn capacity_holder(&self) -> Result<Option<String>, QueueReadError> {
        self.queue
            .get_deduplication_job_id(crate::queue::PROOF_CAPACITY_DEDUPLICATION_ID)
            .await
            .map_err(map_read_error)
    }
}

impl CapabilityQueue for BullMqCapabilityQueue {
    async fn get_job(&self, job_id: &str) -> Result<Option<RetainedQueueJob>, QueueReadError> {
        let job = self.queue.get_job(job_id).await.map_err(map_read_error)?;
        let Some(job) = job else {
            return Ok(None);
        };
        let state = job.get_state().await.map_err(map_read_error)?;
        Ok(Some(RetainedQueueJob::from((job, state))))
    }

    async fn add_job(
        &self,
        job_id: &str,
        payload: &CapabilityJobPayload,
    ) -> Result<QueueAddOutcome, QueueAddError> {
        let data = payload
            .to_json_value()
            .map_err(|_| QueueAddError::Rejected)?;
        self.queue
            .add(CAPABILITY_JOB_NAME, data)
            .options(self.job_options.clone())
            .job_id(job_id)
            .await
            .map(|job| {
                if job.id() == job_id {
                    QueueAddOutcome::Added
                } else {
                    QueueAddOutcome::CapacityOccupied
                }
            })
            .map_err(map_add_error)
    }
}

impl From<(Job, JobState)> for RetainedQueueJob {
    fn from((job, state): (Job, JobState)) -> Self {
        Self {
            id: job.id().to_string(),
            name: job.name().to_string(),
            payload: CapabilityJobPayload::from_json_value(job.data()),
            policy_matches: job_policy_matches(&job),
            state,
            timestamp_ms: job.timestamp(),
            processed_on_ms: job.processed_on(),
            finished_on_ms: job.finished_on(),
            progress: job.progress().clone(),
            failed_reason: job.failed_reason().to_owned(),
            return_value: job.returnvalue().to_owned(),
        }
    }
}

fn job_policy_matches(job: &Job) -> bool {
    serde_json::to_value(job.opts()).ok() == serde_json::to_value(retained_job_options()).ok()
}

pub(super) fn map_read_error(error: BullMqError) -> QueueReadError {
    match error {
        BullMqError::Serialization(_) | BullMqError::InvalidConfig(_) => {
            QueueReadError::CorruptRecord
        }
        _ => QueueReadError::Unavailable,
    }
}

pub(super) fn map_add_error(error: BullMqError) -> QueueAddError {
    match error {
        BullMqError::Serialization(_)
        | BullMqError::InvalidConfig(_)
        | BullMqError::Script { .. } => QueueAddError::Rejected,
        _ => QueueAddError::Uncertain,
    }
}
