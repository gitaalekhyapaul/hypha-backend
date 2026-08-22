use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bullmq::worker::{CancellationToken, WorkerEvent};
use bullmq::{Error as BullMqError, Job, JobProgress, Queue, Worker};
use tokio::task::JoinHandle;

use super::processor::{EnvelopeAccess, validate_claimed_job};
use super::{
    CapabilityExecutor, ClaimedJobProcessor, EXECUTOR_TERMINATION_GRACE, ProgressPublisher,
    PublicFailureClass, SupervisorConfig, SupervisorProgressPhase,
};
use crate::private_envelope::PrivateEnvelopeStore;
use crate::queue::{CAPABILITY_QUEUE_NAME, GLOBAL_CONCURRENCY};
use crate::service::{SystemClock, TrustedClock};

const WORKER_CLOSE_TIMEOUT: Duration =
    Duration::from_secs(EXECUTOR_TERMINATION_GRACE.as_secs() + 10);
const SUPERVISOR_CLOSE_TIMEOUT: Duration = Duration::from_secs(45);
const EVENT_BRIDGE_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Official BullMQ worker wrapper for one proof-supervisor process.
///
/// No production binary starts this wrapper until terminal capability validation
/// and the concrete one-shot prover exist.
pub struct ProofSupervisor {
    shutdown: CancellationToken,
    lifecycle: Option<JoinHandle<Result<(), SupervisorRuntimeError>>>,
}

/// Startup authority retained inside `razield` until terminal capability validation exists.
pub struct ProofSupervisorActivation {
    _private: (),
}

impl ProofSupervisorActivation {
    #[cfg(test)]
    pub(crate) const fn library_plumbing() -> Self {
        Self { _private: () }
    }
}

impl ProofSupervisor {
    /// Starts the claim boundary only for code holding the crate-owned activation token.
    pub async fn start(
        config: SupervisorConfig,
        executor: Arc<dyn CapabilityExecutor>,
        _activation: ProofSupervisorActivation,
    ) -> Result<Self, SupervisorRuntimeError> {
        Self::start_with_clock(config, executor, Arc::new(SystemClock), _activation).await
    }

    pub(crate) async fn start_with_clock(
        config: SupervisorConfig,
        executor: Arc<dyn CapabilityExecutor>,
        clock: Arc<dyn TrustedClock>,
        _activation: ProofSupervisorActivation,
    ) -> Result<Self, SupervisorRuntimeError> {
        let root = config.private_envelope_root().to_owned();
        let store = tokio::task::spawn_blocking(move || PrivateEnvelopeStore::open(root))
            .await
            .map_err(|_| SupervisorRuntimeError::BlockingTaskFailed)?
            .map_err(|_| SupervisorRuntimeError::PrivateStorage)?;

        let queue =
            Queue::with_options(CAPABILITY_QUEUE_NAME, config.queue_config().queue_options())
                .await
                .map_err(|_| SupervisorRuntimeError::QueueUnavailable)?;
        if queue
            .set_global_concurrency(GLOBAL_CONCURRENCY)
            .await
            .is_err()
        {
            queue.close().await;
            return Err(SupervisorRuntimeError::QueueUnavailable);
        }

        let processor = Arc::new(ClaimedJobProcessor::new(
            Arc::new(store) as Arc<dyn EnvelopeAccess>,
            executor,
            clock,
        ));
        let worker_processor = Arc::clone(&processor);
        let worker = Worker::with_options(
            CAPABILITY_QUEUE_NAME,
            move |mut job: Job, cancellation: CancellationToken| {
                let processor = Arc::clone(&worker_processor);
                async move {
                    let reference = validate_claimed_job(&job).map_err(|failure| {
                        BullMqError::Unrecoverable(failure.public_class().redis_reason().to_owned())
                    })?;
                    let mut progress = BullMqProgressPublisher { job: &mut job };
                    processor
                        .process(reference, cancellation, &mut progress)
                        .await
                        .map_or_else(
                            |failure| {
                                Err(BullMqError::Unrecoverable(
                                    failure.public_class().redis_reason().to_owned(),
                                ))
                            },
                            |_| {
                                Err(BullMqError::Unrecoverable(
                                    PublicFailureClass::ServiceUnavailable
                                        .redis_reason()
                                        .to_owned(),
                                ))
                            },
                        )
                }
            },
            config
                .queue_config()
                .worker_options()
                .name("razield-proof-supervisor")
                .manual_start(),
        )
        .await;
        queue.close().await;
        let worker = worker.map_err(|_| SupervisorRuntimeError::QueueUnavailable)?;

        let worker = Arc::new(worker);
        if worker.run().await.is_err() {
            let _ = worker.close(WORKER_CLOSE_TIMEOUT.as_millis() as u64).await;
            return Err(SupervisorRuntimeError::WorkerStartFailed);
        }
        let event_bridge = spawn_event_bridge(Arc::clone(&worker));
        let shutdown = CancellationToken::new();
        let lifecycle = spawn_lifecycle(Arc::clone(&worker), shutdown.clone(), event_bridge);

        Ok(Self {
            shutdown,
            lifecycle: Some(lifecycle),
        })
    }

    /// Cancels active work and closes the worker without exposing worker errors.
    pub async fn close(mut self) -> Result<(), SupervisorRuntimeError> {
        self.shutdown.cancel();
        let mut lifecycle = self
            .lifecycle
            .take()
            .ok_or(SupervisorRuntimeError::WorkerLifecycleFailed)?;
        match tokio::time::timeout(SUPERVISOR_CLOSE_TIMEOUT, &mut lifecycle).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(SupervisorRuntimeError::WorkerLifecycleFailed),
            Err(_) => Err(SupervisorRuntimeError::WorkerCloseFailed),
        }
    }
}

impl Drop for ProofSupervisor {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

impl fmt::Debug for ProofSupervisor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProofSupervisor")
            .field("worker", &"<redacted>")
            .finish()
    }
}

struct BullMqProgressPublisher<'a> {
    job: &'a mut Job,
}

impl ProgressPublisher for BullMqProgressPublisher<'_> {
    fn publish<'a>(
        &'a mut self,
        phase: SupervisorProgressPhase,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
        Box::pin(async move {
            self.job
                .update_progress(JobProgress::Object(phase.to_public_value()))
                .await
                .map_err(|_| ())
        })
    }
}

fn spawn_event_bridge(worker: Arc<Worker>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match worker.next_event().await {
                Some(WorkerEvent::Stalled { job_id }) => {
                    let _ = worker.cancel_job(&job_id).await;
                }
                Some(WorkerEvent::Closed) | None => break,
                Some(_) => {}
            }
        }
    })
}

fn spawn_lifecycle(
    worker: Arc<Worker>,
    shutdown: CancellationToken,
    event_bridge: JoinHandle<()>,
) -> JoinHandle<Result<(), SupervisorRuntimeError>> {
    tokio::spawn(async move {
        shutdown.cancelled().await;
        worker.cancel_all_jobs().await;
        worker
            .close(WORKER_CLOSE_TIMEOUT.as_millis() as u64)
            .await
            .map_err(|_| SupervisorRuntimeError::WorkerCloseFailed)?;
        if worker.active_count().await != 0 {
            return Err(SupervisorRuntimeError::WorkerCloseIncomplete);
        }

        let mut event_bridge = event_bridge;
        match tokio::time::timeout(EVENT_BRIDGE_CLOSE_TIMEOUT, &mut event_bridge).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(SupervisorRuntimeError::WorkerLifecycleFailed),
            Err(_) => {
                event_bridge.abort();
                Err(SupervisorRuntimeError::WorkerLifecycleFailed)
            }
        }
    })
}

/// Redacted process-boundary failures for supervisor startup and shutdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorRuntimeError {
    QueueUnavailable,
    PrivateStorage,
    BlockingTaskFailed,
    WorkerStartFailed,
    WorkerCloseFailed,
    WorkerCloseIncomplete,
    WorkerLifecycleFailed,
}

impl fmt::Display for SupervisorRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::QueueUnavailable => "proof supervisor queue is unavailable",
            Self::PrivateStorage => "proof supervisor private storage failed",
            Self::BlockingTaskFailed => "proof supervisor blocking task failed",
            Self::WorkerStartFailed => "proof supervisor failed to start",
            Self::WorkerCloseFailed => "proof supervisor failed to close",
            Self::WorkerCloseIncomplete => "proof supervisor shutdown left active work",
            Self::WorkerLifecycleFailed => "proof supervisor lifecycle failed",
        })
    }
}

impl Error for SupervisorRuntimeError {}
