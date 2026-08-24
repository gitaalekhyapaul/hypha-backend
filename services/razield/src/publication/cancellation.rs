use std::fmt::{self, Debug, Formatter};

use bullmq::types::JobState;
use bullmq::{Job, JobOptions, Queue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::bullmq::{QueueAddError, QueueReadError, map_add_error, map_read_error};
use crate::queue::{
    CANCELLATION_QUEUE_NAME, GLOBAL_CONCURRENCY, QueueConfig, retained_cancellation_job_options,
    valid_cancellation_job_options,
};

pub(crate) const CANCELLATION_JOB_NAME: &str = "cancel-capability";
pub(crate) const CANCELLATION_COMMAND_FORMAT_VERSION: u32 = 1;
const CANCELLATION_RESULT_FORMAT_VERSION: u32 = 1;

/// Public, bounded command sent to the internal ADR-0017 control queue.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CancellationCommandPayload {
    target_job_id: String,
    target_submitted_at_unix_millis: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CancellationCommandWireV1 {
    format_version: u32,
    target_job_id: String,
    target_submitted_at_unix_millis: u64,
}

impl CancellationCommandPayload {
    pub(crate) fn new(target_job_id: String, target_submitted_at_unix_millis: u64) -> Self {
        Self {
            target_job_id,
            target_submitted_at_unix_millis,
        }
    }

    pub(crate) fn target_job_id(&self) -> &str {
        &self.target_job_id
    }

    pub(crate) const fn target_submitted_at_unix_millis(&self) -> u64 {
        self.target_submitted_at_unix_millis
    }

    pub(crate) fn to_json_value(&self) -> Result<Value, ()> {
        serde_json::to_value(CancellationCommandWireV1 {
            format_version: CANCELLATION_COMMAND_FORMAT_VERSION,
            target_job_id: self.target_job_id.clone(),
            target_submitted_at_unix_millis: self.target_submitted_at_unix_millis,
        })
        .map_err(|_| ())
    }

    pub(crate) fn from_json_value(value: &Value) -> Result<Self, ()> {
        let wire: CancellationCommandWireV1 =
            serde_json::from_value(value.clone()).map_err(|_| ())?;
        if wire.format_version != CANCELLATION_COMMAND_FORMAT_VERSION {
            return Err(());
        }
        super::lifecycle::validate_job_id(&wire.target_job_id).map_err(|_| ())?;
        Ok(Self {
            target_job_id: wire.target_job_id,
            target_submitted_at_unix_millis: wire.target_submitted_at_unix_millis,
        })
    }
}

impl Debug for CancellationCommandPayload {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancellationCommandPayload")
            .field("target_job_id", &"<redacted>")
            .field("target_submitted_at", &"<public timestamp>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CancellationCommandOutcome {
    Canceled,
    TerminalUnchanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CancellationCommandResult {
    outcome: CancellationCommandOutcome,
    target_submitted_at_unix_millis: u64,
    confirmed_at_unix_millis: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CancellationResultWireV1 {
    format_version: u32,
    outcome: String,
    target_submitted_at_unix_millis: u64,
    confirmed_at_unix_millis: u64,
}

impl CancellationCommandResult {
    pub(crate) fn canceled(
        target_submitted_at_unix_millis: u64,
        confirmed_at_unix_millis: u64,
    ) -> Result<Self, ()> {
        Self::new(
            CancellationCommandOutcome::Canceled,
            target_submitted_at_unix_millis,
            confirmed_at_unix_millis,
        )
    }

    pub(crate) fn terminal_unchanged(
        target_submitted_at_unix_millis: u64,
        confirmed_at_unix_millis: u64,
    ) -> Result<Self, ()> {
        Self::new(
            CancellationCommandOutcome::TerminalUnchanged,
            target_submitted_at_unix_millis,
            confirmed_at_unix_millis,
        )
    }

    fn new(
        outcome: CancellationCommandOutcome,
        target_submitted_at_unix_millis: u64,
        confirmed_at_unix_millis: u64,
    ) -> Result<Self, ()> {
        if confirmed_at_unix_millis < target_submitted_at_unix_millis {
            return Err(());
        }
        Ok(Self {
            outcome,
            target_submitted_at_unix_millis,
            confirmed_at_unix_millis,
        })
    }

    pub(crate) const fn outcome(self) -> CancellationCommandOutcome {
        self.outcome
    }

    pub(crate) const fn target_submitted_at_unix_millis(self) -> u64 {
        self.target_submitted_at_unix_millis
    }

    pub(crate) const fn confirmed_at_unix_millis(self) -> u64 {
        self.confirmed_at_unix_millis
    }

    pub(crate) fn to_json_value(self) -> Result<Value, ()> {
        let outcome = match self.outcome {
            CancellationCommandOutcome::Canceled => "CANCELED",
            CancellationCommandOutcome::TerminalUnchanged => "TERMINAL_UNCHANGED",
        };
        serde_json::to_value(CancellationResultWireV1 {
            format_version: CANCELLATION_RESULT_FORMAT_VERSION,
            outcome: outcome.to_owned(),
            target_submitted_at_unix_millis: self.target_submitted_at_unix_millis,
            confirmed_at_unix_millis: self.confirmed_at_unix_millis,
        })
        .map_err(|_| ())
    }

    fn from_json_string(value: &str) -> Result<Self, ()> {
        let wire: CancellationResultWireV1 = serde_json::from_str(value).map_err(|_| ())?;
        if wire.format_version != CANCELLATION_RESULT_FORMAT_VERSION {
            return Err(());
        }
        let outcome = match wire.outcome.as_str() {
            "CANCELED" => CancellationCommandOutcome::Canceled,
            "TERMINAL_UNCHANGED" => CancellationCommandOutcome::TerminalUnchanged,
            _ => return Err(()),
        };
        Self::new(
            outcome,
            wire.target_submitted_at_unix_millis,
            wire.confirmed_at_unix_millis,
        )
    }
}

pub(super) struct BullMqCancellationQueue {
    queue: Queue,
    job_options: JobOptions,
}

#[derive(Clone)]
pub(super) struct RetainedCancellationCommand {
    pub id: String,
    pub name: String,
    pub payload: Result<CancellationCommandPayload, ()>,
    pub policy_matches: bool,
    pub state: JobState,
    pub timestamp_ms: u64,
    pub processed_on_ms: Option<u64>,
    pub finished_on_ms: Option<u64>,
    pub failed_reason: String,
    pub return_value: String,
}

impl RetainedCancellationCommand {
    pub(super) fn matches(&self, expected: &CancellationCommandPayload) -> bool {
        self.id == expected.target_job_id
            && self.name == CANCELLATION_JOB_NAME
            && self.payload.as_ref().ok() == Some(expected)
            && self.policy_matches
    }

    pub(super) fn terminal_result(&self) -> Result<Option<CancellationCommandResult>, ()> {
        match self.state {
            JobState::Waiting | JobState::Delayed | JobState::Prioritized | JobState::Active => {
                if self.finished_on_ms.is_some()
                    || !self.failed_reason.is_empty()
                    || !self.return_value.is_empty()
                {
                    return Err(());
                }
                Ok(None)
            }
            JobState::Completed => {
                if self.finished_on_ms.is_none()
                    || !self.failed_reason.is_empty()
                    || self.return_value.is_empty()
                {
                    return Err(());
                }
                CancellationCommandResult::from_json_string(&self.return_value).map(Some)
            }
            JobState::Failed | JobState::WaitingChildren | JobState::Unknown => Err(()),
        }
    }
}

impl BullMqCancellationQueue {
    pub(super) async fn connect(config: &QueueConfig) -> Result<Self, QueueReadError> {
        let queue = Queue::with_options(CANCELLATION_QUEUE_NAME, config.queue_options())
            .await
            .map_err(map_read_error)?;
        queue
            .set_global_concurrency(GLOBAL_CONCURRENCY)
            .await
            .map_err(map_read_error)?;
        Ok(Self {
            queue,
            job_options: config.cancellation_job_options(),
        })
    }

    pub(super) async fn get(
        &self,
        job_id: &str,
    ) -> Result<Option<RetainedCancellationCommand>, QueueReadError> {
        let job = self.queue.get_job(job_id).await.map_err(map_read_error)?;
        let Some(job) = job else {
            return Ok(None);
        };
        let state = job.get_state().await.map_err(map_read_error)?;
        Ok(Some(RetainedCancellationCommand::from((job, state))))
    }

    pub(super) async fn add(
        &self,
        payload: &CancellationCommandPayload,
    ) -> Result<(), QueueAddError> {
        let data = payload
            .to_json_value()
            .map_err(|_| QueueAddError::Rejected)?;
        self.queue
            .add(CANCELLATION_JOB_NAME, data)
            .options(self.job_options.clone())
            .job_id(payload.target_job_id())
            .await
            .map(|_| ())
            .map_err(map_add_error)
    }

    pub(super) async fn close(&self) {
        self.queue.close().await;
    }
}

impl From<(Job, JobState)> for RetainedCancellationCommand {
    fn from((job, state): (Job, JobState)) -> Self {
        Self {
            id: job.id().to_owned(),
            name: job.name().to_owned(),
            payload: CancellationCommandPayload::from_json_value(job.data()),
            policy_matches: serde_json::to_value(job.opts()).ok()
                == serde_json::to_value(retained_cancellation_job_options()).ok()
                && valid_cancellation_job_options(job.opts()),
            state,
            timestamp_ms: job.timestamp(),
            processed_on_ms: job.processed_on(),
            finished_on_ms: job.finished_on(),
            failed_reason: job.failed_reason().to_owned(),
            return_value: job.returnvalue().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::queue::MAX_PUBLIC_JOB_PAYLOAD_BYTES;

    #[test]
    fn command_payload_is_strict_bounded_public_data() {
        let job_id = "ab".repeat(32);
        let payload = CancellationCommandPayload::new(job_id.clone(), 1_800_000_000_000);
        let value = payload.to_json_value().unwrap();
        assert_eq!(
            CancellationCommandPayload::from_json_value(&value),
            Ok(payload)
        );
        assert!(serde_json::to_vec(&value).unwrap().len() <= MAX_PUBLIC_JOB_PAYLOAD_BYTES);
        assert!(!format!("{value:?}").contains("private_envelope"));

        let mut extra = value;
        extra["private"] = json!("not-accepted");
        assert!(CancellationCommandPayload::from_json_value(&extra).is_err());

        let uppercase = json!({
            "format_version": 1,
            "target_job_id": "AB".repeat(32),
            "target_submitted_at_unix_millis": 1_800_000_000_000_u64,
        });
        assert!(CancellationCommandPayload::from_json_value(&uppercase).is_err());
    }

    #[test]
    fn terminal_results_use_closed_outcomes_and_ordered_timestamps() {
        for result in [
            CancellationCommandResult::canceled(100, 101).unwrap(),
            CancellationCommandResult::terminal_unchanged(100, 102).unwrap(),
        ] {
            let encoded = result.to_json_value().unwrap().to_string();
            assert_eq!(
                CancellationCommandResult::from_json_string(&encoded),
                Ok(result)
            );
        }
        assert!(CancellationCommandResult::canceled(101, 100).is_err());
        assert!(
            CancellationCommandResult::from_json_string(
                r#"{"format_version":1,"outcome":"OPEN","target_submitted_at_unix_millis":100,"confirmed_at_unix_millis":101}"#,
            )
            .is_err()
        );
    }
}
