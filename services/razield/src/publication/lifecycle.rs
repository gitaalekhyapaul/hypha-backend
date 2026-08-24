use std::error::Error;
use std::fmt::{self, Debug, Formatter};

use bullmq::JobProgress;
use bullmq::types::JobState;
use raziel_proto::raziel::v1::{ProofCapability, ProofMetrics};
use serde_json::Value;

use super::bullmq::{QueueReadError, RetainedQueueJob};
use super::{CAPABILITY_JOB_NAME, CapabilityJobPayload};
use crate::private_envelope::PrivateEnvelopeRef;
use crate::worker::{PublicFailureClass, SupervisorProgressPhase, TerminalCapabilityResult};

/// Public capability lifecycle states derived from authoritative queue records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicCapabilityJobState {
    Queued,
    Executing,
    Proving,
    Succeeded,
    Failed,
    Canceled,
}

/// Strict public projection of one retained capability job.
#[derive(Clone, PartialEq, Eq)]
pub struct RetainedCapabilityJob {
    job_id: String,
    state: PublicCapabilityJobState,
    submitted_at_unix_millis: u64,
    updated_at_unix_millis: u64,
    capability: Option<ProofCapability>,
    failure: Option<PublicFailureClass>,
    metrics: Option<ProofMetrics>,
}

impl RetainedCapabilityJob {
    pub(crate) fn new_nonterminal(
        job_id: String,
        state: PublicCapabilityJobState,
        submitted_at_unix_millis: u64,
        updated_at_unix_millis: u64,
    ) -> Self {
        Self {
            job_id,
            state,
            submitted_at_unix_millis,
            updated_at_unix_millis,
            capability: None,
            failure: None,
            metrics: None,
        }
    }

    pub(crate) fn new_canceled(
        job_id: String,
        submitted_at_unix_millis: u64,
        updated_at_unix_millis: u64,
    ) -> Self {
        Self {
            job_id,
            state: PublicCapabilityJobState::Canceled,
            submitted_at_unix_millis,
            updated_at_unix_millis,
            capability: None,
            failure: None,
            metrics: None,
        }
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
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

    pub const fn capability(&self) -> Option<&ProofCapability> {
        self.capability.as_ref()
    }

    pub const fn failure(&self) -> Option<PublicFailureClass> {
        self.failure
    }

    pub const fn metrics(&self) -> Option<&ProofMetrics> {
        self.metrics.as_ref()
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            PublicCapabilityJobState::Succeeded
                | PublicCapabilityJobState::Failed
                | PublicCapabilityJobState::Canceled
        )
    }
}

impl Debug for RetainedCapabilityJob {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedCapabilityJob")
            .field("job_id", &"<redacted>")
            .field("state", &self.state)
            .field("submitted_at", &"<public timestamp>")
            .field("updated_at", &"<public timestamp>")
            .field(
                "capability",
                &self.capability.as_ref().map(|_| "<redacted-proof>"),
            )
            .field("failure", &self.failure)
            .field("metrics", &self.metrics)
            .finish()
    }
}

/// Typed private causes for public Get and Cancel failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityLifecycleError {
    InvalidJobId,
    NotFound,
    QueueUnavailable,
    QueueRecordMismatch,
    CancellationPublicationRejected,
    CancellationPublicationUncertain,
}

impl fmt::Display for CapabilityLifecycleError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJobId => "capability job identifier is invalid",
            Self::NotFound => "capability job was not found",
            Self::QueueUnavailable => "capability queue is unavailable",
            Self::QueueRecordMismatch => "capability queue record is invalid",
            Self::CancellationPublicationRejected => {
                "capability cancellation publication was rejected"
            }
            Self::CancellationPublicationUncertain => {
                "capability cancellation publication is uncertain"
            }
        })
    }
}

impl Error for CapabilityLifecycleError {}

pub(crate) fn validate_job_id(
    job_id: &str,
) -> Result<PrivateEnvelopeRef, CapabilityLifecycleError> {
    if job_id.len() != 64
        || !job_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CapabilityLifecycleError::InvalidJobId);
    }
    PrivateEnvelopeRef::from_queue_token(&format!("pe1_{job_id}"))
        .map_err(|_| CapabilityLifecycleError::InvalidJobId)
}

pub(crate) fn project_proof_job(
    retained: RetainedQueueJob,
) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
    let reference = validate_job_id(&retained.id)?;
    let expected_payload = CapabilityJobPayload::new(reference);
    if retained.name != CAPABILITY_JOB_NAME
        || retained.payload.as_ref().ok() != Some(&expected_payload)
        || !retained.policy_matches
    {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }

    match retained.state {
        JobState::Waiting | JobState::Delayed | JobState::Prioritized => {
            validate_nonterminal_fields(&retained, false)?;
            Ok(RetainedCapabilityJob::new_nonterminal(
                retained.id,
                PublicCapabilityJobState::Queued,
                retained.timestamp_ms,
                retained.timestamp_ms,
            ))
        }
        JobState::Active => {
            validate_nonterminal_fields(&retained, true)?;
            let phase = match &retained.progress {
                JobProgress::Object(value) => SupervisorProgressPhase::from_public_value(value)
                    .map_err(|_| CapabilityLifecycleError::QueueRecordMismatch)?,
                JobProgress::Number(_) => {
                    return Err(CapabilityLifecycleError::QueueRecordMismatch);
                }
            };
            let state = match phase {
                SupervisorProgressPhase::Executing => PublicCapabilityJobState::Executing,
                SupervisorProgressPhase::Proving => PublicCapabilityJobState::Proving,
            };
            let updated_at = retained
                .processed_on_ms
                .ok_or(CapabilityLifecycleError::QueueRecordMismatch)?;
            validate_timestamp_order(retained.timestamp_ms, updated_at)?;
            Ok(RetainedCapabilityJob::new_nonterminal(
                retained.id,
                state,
                retained.timestamp_ms,
                updated_at,
            ))
        }
        JobState::Completed => project_completed(retained),
        JobState::Failed => project_failed(retained),
        JobState::WaitingChildren | JobState::Unknown => {
            Err(CapabilityLifecycleError::QueueRecordMismatch)
        }
    }
}

fn project_completed(
    retained: RetainedQueueJob,
) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
    validate_terminal_fields(&retained, false)?;
    let finished_at = retained
        .finished_on_ms
        .ok_or(CapabilityLifecycleError::QueueRecordMismatch)?;
    validate_timestamp_order(retained.timestamp_ms, finished_at)?;
    let value: Value = serde_json::from_str(&retained.return_value)
        .map_err(|_| CapabilityLifecycleError::QueueRecordMismatch)?;
    let result = TerminalCapabilityResult::from_public_value(&retained.id, &value)
        .map_err(|_| CapabilityLifecycleError::QueueRecordMismatch)?;
    Ok(RetainedCapabilityJob {
        job_id: retained.id,
        state: PublicCapabilityJobState::Succeeded,
        submitted_at_unix_millis: retained.timestamp_ms,
        updated_at_unix_millis: finished_at,
        capability: Some(result.capability().clone()),
        failure: None,
        metrics: Some(result.proof_job_metrics()),
    })
}

fn project_failed(
    retained: RetainedQueueJob,
) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
    validate_terminal_fields(&retained, true)?;
    let finished_at = retained
        .finished_on_ms
        .ok_or(CapabilityLifecycleError::QueueRecordMismatch)?;
    validate_timestamp_order(retained.timestamp_ms, finished_at)?;
    let (state, failure) = match retained.failed_reason.as_str() {
        "CANCELED" => (PublicCapabilityJobState::Canceled, None),
        "INVALID_REQUEST" => (
            PublicCapabilityJobState::Failed,
            Some(PublicFailureClass::InvalidRequest),
        ),
        "SERVICE_UNAVAILABLE" => (
            PublicCapabilityJobState::Failed,
            Some(PublicFailureClass::ServiceUnavailable),
        ),
        _ => return Err(CapabilityLifecycleError::QueueRecordMismatch),
    };
    Ok(RetainedCapabilityJob {
        job_id: retained.id,
        state,
        submitted_at_unix_millis: retained.timestamp_ms,
        updated_at_unix_millis: finished_at,
        capability: None,
        failure,
        metrics: None,
    })
}

fn validate_nonterminal_fields(
    retained: &RetainedQueueJob,
    active: bool,
) -> Result<(), CapabilityLifecycleError> {
    if retained.finished_on_ms.is_some()
        || !retained.failed_reason.is_empty()
        || !retained.return_value.is_empty()
        || active != retained.processed_on_ms.is_some()
    {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }
    Ok(())
}

fn validate_terminal_fields(
    retained: &RetainedQueueJob,
    failed: bool,
) -> Result<(), CapabilityLifecycleError> {
    if retained.processed_on_ms.is_none()
        || retained.finished_on_ms.is_none()
        || failed == retained.failed_reason.is_empty()
        || failed != retained.return_value.is_empty()
    {
        return Err(CapabilityLifecycleError::QueueRecordMismatch);
    }
    let processed_at = retained
        .processed_on_ms
        .ok_or(CapabilityLifecycleError::QueueRecordMismatch)?;
    let finished_at = retained
        .finished_on_ms
        .ok_or(CapabilityLifecycleError::QueueRecordMismatch)?;
    validate_timestamp_order(retained.timestamp_ms, processed_at)?;
    validate_timestamp_order(processed_at, finished_at)
}

fn validate_timestamp_order(start: u64, end: u64) -> Result<(), CapabilityLifecycleError> {
    if end < start {
        Err(CapabilityLifecycleError::QueueRecordMismatch)
    } else {
        Ok(())
    }
}

pub(crate) const fn map_queue_read_error(error: QueueReadError) -> CapabilityLifecycleError {
    match error {
        QueueReadError::CorruptRecord => CapabilityLifecycleError::QueueRecordMismatch,
        QueueReadError::Unavailable => CapabilityLifecycleError::QueueUnavailable,
    }
}
