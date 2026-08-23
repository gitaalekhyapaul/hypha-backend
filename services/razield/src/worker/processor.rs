use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bullmq::Job;
use bullmq::worker::CancellationToken;
use serde_json::{Value, json};

use super::{
    CapabilityExecutionRequest, CapabilityExecutor, EXECUTOR_TERMINATION_GRACE, ExecutionControl,
    ExecutorFailureClass, PROOF_EXECUTION_DEADLINE,
};
use crate::admission::{LOCAL_PLONK_ADMISSION_WINDOW_SECONDS, MAX_PROTOBUF_TIMESTAMP_SECONDS};
use crate::private_envelope::{
    DeleteStatus, PrivateCapabilityEnvelope, PrivateEnvelopeError, PrivateEnvelopeRef,
    PrivateEnvelopeStore,
};
use crate::publication::{CAPABILITY_JOB_NAME, CapabilityJobPayload};
use crate::queue::{MAX_PUBLIC_JOB_PAYLOAD_BYTES, valid_claimed_job_options};
use crate::service::TrustedClock;

/// Storage operations available only after strict public job validation.
pub(crate) trait EnvelopeAccess: Send + Sync + 'static {
    fn read(
        &self,
        reference: &PrivateEnvelopeRef,
    ) -> Result<PrivateCapabilityEnvelope, PrivateEnvelopeError>;

    fn delete(&self, reference: &PrivateEnvelopeRef) -> Result<DeleteStatus, PrivateEnvelopeError>;
}

impl EnvelopeAccess for PrivateEnvelopeStore {
    fn read(
        &self,
        reference: &PrivateEnvelopeRef,
    ) -> Result<PrivateCapabilityEnvelope, PrivateEnvelopeError> {
        Self::read(self, reference)
    }

    fn delete(&self, reference: &PrivateEnvelopeRef) -> Result<DeleteStatus, PrivateEnvelopeError> {
        Self::delete(self, reference)
    }
}

/// Async progress boundary implemented by the BullMQ job adapter.
pub(crate) trait ProgressPublisher {
    fn publish<'a>(
        &'a mut self,
        phase: SupervisorProgressPhase,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>>;
}

/// Closed public progress states. No request or witness field can enter this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorProgressPhase {
    Executing,
    Proving,
}

impl SupervisorProgressPhase {
    /// Encodes the exact bounded JSON stored by BullMQ progress.
    pub fn to_public_value(self) -> Value {
        let phase = match self {
            Self::Executing => "EXECUTING",
            Self::Proving => "PROVING",
        };
        json!({
            "format_version": 1,
            "phase": phase,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExecutorHandoffComplete {
    result: crate::one_shot::OneShotSuccess,
}

impl ExecutorHandoffComplete {
    pub(crate) fn into_result(self) -> crate::one_shot::OneShotSuccess {
        self.result
    }
}

/// Closed public terminal classes written verbatim to BullMQ failure state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicFailureClass {
    InvalidRequest,
    ServiceUnavailable,
    Canceled,
}

impl PublicFailureClass {
    /// Returns the only strings permitted in BullMQ `failedReason`.
    pub const fn redis_reason(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::ServiceUnavailable => "SERVICE_UNAVAILABLE",
            Self::Canceled => "CANCELED",
        }
    }
}

/// Redacted private failure classes retained for later protected audit mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorFailureClass {
    InvalidJobName,
    InvalidPublicPayload,
    PublicPayloadTooLarge,
    JobIdentityMismatch,
    InvalidJobPolicy,
    TrustedTimeUnavailable,
    InsufficientValidityWindow,
    ProgressUnavailable,
    PrivateEnvelopeRead(PrivateEnvelopeError),
    PrivateEnvelopeCleanup(PrivateEnvelopeError),
    BlockingTaskFailed,
    Cancelled,
    Executor(ExecutorFailureClass),
}

/// One typed internal failure with a fixed public projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupervisorFailure {
    class: SupervisorFailureClass,
}

impl SupervisorFailure {
    const fn new(class: SupervisorFailureClass) -> Self {
        Self { class }
    }

    #[cfg(test)]
    pub const fn class(self) -> SupervisorFailureClass {
        self.class
    }

    pub const fn public_class(self) -> PublicFailureClass {
        match self.class {
            SupervisorFailureClass::InvalidJobName
            | SupervisorFailureClass::InvalidPublicPayload
            | SupervisorFailureClass::PublicPayloadTooLarge
            | SupervisorFailureClass::JobIdentityMismatch
            | SupervisorFailureClass::InvalidJobPolicy
            | SupervisorFailureClass::InsufficientValidityWindow => {
                PublicFailureClass::InvalidRequest
            }
            SupervisorFailureClass::Cancelled
            | SupervisorFailureClass::Executor(ExecutorFailureClass::Cancelled) => {
                PublicFailureClass::Canceled
            }
            SupervisorFailureClass::TrustedTimeUnavailable
            | SupervisorFailureClass::ProgressUnavailable
            | SupervisorFailureClass::PrivateEnvelopeRead(_)
            | SupervisorFailureClass::PrivateEnvelopeCleanup(_)
            | SupervisorFailureClass::BlockingTaskFailed
            | SupervisorFailureClass::Executor(_) => PublicFailureClass::ServiceUnavailable,
        }
    }
}

impl fmt::Display for SupervisorFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("proof supervisor job failed")
    }
}

impl Error for SupervisorFailure {}

/// Validates one claimed public job and performs the private handoff exactly once.
pub(crate) struct ClaimedJobProcessor {
    store: Arc<dyn EnvelopeAccess>,
    executor: Arc<dyn CapabilityExecutor>,
    clock: Arc<dyn TrustedClock>,
}

impl ClaimedJobProcessor {
    pub(crate) fn new(
        store: Arc<dyn EnvelopeAccess>,
        executor: Arc<dyn CapabilityExecutor>,
        clock: Arc<dyn TrustedClock>,
    ) -> Self {
        Self {
            store,
            executor,
            clock,
        }
    }

    pub(crate) async fn process<P: ProgressPublisher>(
        &self,
        reference: PrivateEnvelopeRef,
        cancellation: CancellationToken,
        progress: &mut P,
    ) -> Result<ExecutorHandoffComplete, SupervisorFailure> {
        if cancellation.is_cancelled() {
            self.cleanup(reference).await?;
            return Err(SupervisorFailure::new(SupervisorFailureClass::Cancelled));
        }

        let issued_at_unix_seconds = self
            .clock
            .now_unix_seconds()
            .map_err(|_| SupervisorFailure::new(SupervisorFailureClass::TrustedTimeUnavailable));
        let issued_at_unix_seconds = match issued_at_unix_seconds {
            Ok(value) => value,
            Err(error) => return self.fail_after_cleanup(reference, error).await,
        };
        if !(0..=MAX_PROTOBUF_TIMESTAMP_SECONDS).contains(&issued_at_unix_seconds) {
            return self
                .fail_after_cleanup(
                    reference,
                    SupervisorFailure::new(SupervisorFailureClass::TrustedTimeUnavailable),
                )
                .await;
        }

        if progress
            .publish(SupervisorProgressPhase::Executing)
            .await
            .is_err()
        {
            return self
                .fail_after_cleanup(
                    reference,
                    SupervisorFailure::new(SupervisorFailureClass::ProgressUnavailable),
                )
                .await;
        }

        let envelope = match self.read(reference).await {
            Ok(envelope) => envelope,
            Err(error) => return self.fail_after_cleanup(reference, error).await,
        };

        if !request_window_covers_local_proof(&envelope, issued_at_unix_seconds) {
            return self
                .fail_after_cleanup(
                    reference,
                    SupervisorFailure::new(SupervisorFailureClass::InsufficientValidityWindow),
                )
                .await;
        }

        if cancellation.is_cancelled() {
            return self
                .fail_after_cleanup(
                    reference,
                    SupervisorFailure::new(SupervisorFailureClass::Cancelled),
                )
                .await;
        }

        let request = CapabilityExecutionRequest::new(envelope, issued_at_unix_seconds);
        let execution = execute_with_limits(
            self.executor.as_ref(),
            request,
            cancellation.clone(),
            PROOF_EXECUTION_DEADLINE,
            EXECUTOR_TERMINATION_GRACE,
        )
        .await;

        self.cleanup(reference).await?;

        match execution {
            Ok(result) => Ok(ExecutorHandoffComplete { result }),
            Err(ExecutorFailureClass::Cancelled) => {
                Err(SupervisorFailure::new(SupervisorFailureClass::Cancelled))
            }
            Err(error) => Err(SupervisorFailure::new(SupervisorFailureClass::Executor(
                error,
            ))),
        }
    }

    async fn read(
        &self,
        reference: PrivateEnvelopeRef,
    ) -> Result<PrivateCapabilityEnvelope, SupervisorFailure> {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || store.read(&reference))
            .await
            .map_err(|_| SupervisorFailure::new(SupervisorFailureClass::BlockingTaskFailed))?
            .map_err(|error| {
                SupervisorFailure::new(SupervisorFailureClass::PrivateEnvelopeRead(error))
            })
    }

    async fn cleanup(&self, reference: PrivateEnvelopeRef) -> Result<(), SupervisorFailure> {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || store.delete(&reference))
            .await
            .map_err(|_| SupervisorFailure::new(SupervisorFailureClass::BlockingTaskFailed))?
            .map(|_| ())
            .map_err(|error| {
                SupervisorFailure::new(SupervisorFailureClass::PrivateEnvelopeCleanup(error))
            })
    }

    async fn fail_after_cleanup<T>(
        &self,
        reference: PrivateEnvelopeRef,
        failure: SupervisorFailure,
    ) -> Result<T, SupervisorFailure> {
        self.cleanup(reference).await?;
        Err(failure)
    }
}

pub(crate) fn validate_claimed_job(job: &Job) -> Result<PrivateEnvelopeRef, SupervisorFailure> {
    let reference = validate_public_job(job.name(), job.id(), job.data())?;
    if !valid_claimed_job_options(job.opts())
        || job.attempts_made() != 0
        || job.attempts_started() != 1
        || job.parent().is_some()
        || job.parent_key().is_some()
        || job.repeat_job_key().is_some()
        || job.deferred_failure().is_some()
        || job.delay() != 0
    {
        return Err(SupervisorFailure::new(
            SupervisorFailureClass::InvalidJobPolicy,
        ));
    }
    Ok(reference)
}

fn request_window_covers_local_proof(
    envelope: &PrivateCapabilityEnvelope,
    issued_at_unix_seconds: i64,
) -> bool {
    issued_at_unix_seconds
        .checked_add(LOCAL_PLONK_ADMISSION_WINDOW_SECONDS)
        .is_some_and(|minimum_expiry| {
            envelope.request().requested_expires_at_unix_seconds >= minimum_expiry
        })
}

pub(crate) async fn execute_with_limits(
    executor: &dyn CapabilityExecutor,
    request: CapabilityExecutionRequest,
    cancellation: CancellationToken,
    deadline: Duration,
    termination_grace: Duration,
) -> Result<crate::one_shot::OneShotSuccess, ExecutorFailureClass> {
    let control = ExecutionControl::new(deadline, cancellation.clone());
    let mut execution = executor.execute(request, control);
    let deadline_sleep = tokio::time::sleep(deadline);
    tokio::pin!(deadline_sleep);

    tokio::select! {
        result = &mut execution => {
            if cancellation.is_cancelled() {
                Err(ExecutorFailureClass::Cancelled)
            } else {
                result
            }
        },
        () = cancellation.cancelled() => {
            match tokio::time::timeout(termination_grace, &mut execution).await {
                Ok(_) => Err(ExecutorFailureClass::Cancelled),
                Err(_) => Err(ExecutorFailureClass::TerminationUnconfirmed),
            }
        }
        () = &mut deadline_sleep => {
            cancellation.cancel();
            match tokio::time::timeout(termination_grace, &mut execution).await {
                Ok(_) => Err(ExecutorFailureClass::TimedOut),
                Err(_) => Err(ExecutorFailureClass::TerminationUnconfirmed),
            }
        }
    }
}

pub(crate) fn validate_public_job(
    job_name: &str,
    job_id: &str,
    public_payload: &Value,
) -> Result<PrivateEnvelopeRef, SupervisorFailure> {
    if job_name != CAPABILITY_JOB_NAME {
        return Err(SupervisorFailure::new(
            SupervisorFailureClass::InvalidJobName,
        ));
    }
    let payload_bytes = serde_json::to_vec(public_payload)
        .map_err(|_| SupervisorFailure::new(SupervisorFailureClass::InvalidPublicPayload))?;
    if payload_bytes.len() > MAX_PUBLIC_JOB_PAYLOAD_BYTES {
        return Err(SupervisorFailure::new(
            SupervisorFailureClass::PublicPayloadTooLarge,
        ));
    }

    let payload = CapabilityJobPayload::from_json_value(public_payload)
        .map_err(|_| SupervisorFailure::new(SupervisorFailureClass::InvalidPublicPayload))?;
    let reference = payload.private_envelope_ref();
    if reference.job_id() != job_id {
        return Err(SupervisorFailure::new(
            SupervisorFailureClass::JobIdentityMismatch,
        ));
    }
    Ok(reference)
}
