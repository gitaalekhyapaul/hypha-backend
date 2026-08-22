use bullmq::{Error as BullMqError, Job, JobOptions, Queue};

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

pub(super) trait CapabilityQueue {
    async fn get_job(&self, job_id: &str) -> Result<Option<RetainedQueueJob>, QueueReadError>;

    async fn add_job(
        &self,
        job_id: &str,
        payload: &CapabilityJobPayload,
    ) -> Result<(), QueueAddError>;
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
}

impl CapabilityQueue for BullMqCapabilityQueue {
    async fn get_job(&self, job_id: &str) -> Result<Option<RetainedQueueJob>, QueueReadError> {
        self.queue
            .get_job(job_id)
            .await
            .map(|job| job.map(RetainedQueueJob::from))
            .map_err(map_read_error)
    }

    async fn add_job(
        &self,
        job_id: &str,
        payload: &CapabilityJobPayload,
    ) -> Result<(), QueueAddError> {
        let data = payload
            .to_json_value()
            .map_err(|_| QueueAddError::Rejected)?;
        self.queue
            .add(CAPABILITY_JOB_NAME, data)
            .options(self.job_options.clone())
            .job_id(job_id)
            .await
            .map(|_| ())
            .map_err(map_add_error)
    }
}

impl From<Job> for RetainedQueueJob {
    fn from(job: Job) -> Self {
        Self {
            id: job.id().to_string(),
            name: job.name().to_string(),
            payload: CapabilityJobPayload::from_json_value(job.data()),
            policy_matches: job_policy_matches(&job),
        }
    }
}

fn job_policy_matches(job: &Job) -> bool {
    serde_json::to_value(job.opts()).ok() == serde_json::to_value(retained_job_options()).ok()
}

fn map_read_error(error: BullMqError) -> QueueReadError {
    match error {
        BullMqError::Serialization(_) | BullMqError::InvalidConfig(_) => {
            QueueReadError::CorruptRecord
        }
        _ => QueueReadError::Unavailable,
    }
}

fn map_add_error(error: BullMqError) -> QueueAddError {
    match error {
        BullMqError::Serialization(_)
        | BullMqError::InvalidConfig(_)
        | BullMqError::Script { .. } => QueueAddError::Rejected,
        _ => QueueAddError::Uncertain,
    }
}
