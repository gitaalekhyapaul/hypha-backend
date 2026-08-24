use std::error::Error;
use std::fmt;
use std::future::Future;
use std::os::unix::fs::MetadataExt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bullmq::worker::{CancellationToken, WorkerEvent};
use bullmq::{Error as BullMqError, Job, JobProgress, Queue, Worker};
use tokio::task::JoinHandle;

use super::processor::{EnvelopeAccess, validate_claimed_job};
use super::{
    CancellationCommandProcessor, CancellationIntentRegistry, CapabilityExecutor,
    CapabilityResultValidator, ClaimedJobProcessor, EXECUTOR_TERMINATION_GRACE, ProgressPublisher,
    PublicFailureClass, SupervisorConfig, SupervisorProgressPhase, TerminalResultValidator,
    TerminalValidatorConfig,
};
use crate::one_shot::UnixSocketExecutor;
use crate::private_envelope::PrivateEnvelopeStore;
use crate::queue::{CANCELLATION_QUEUE_NAME, CAPABILITY_QUEUE_NAME, GLOBAL_CONCURRENCY};
use crate::service::{SystemClock, TrustedClock};

const WORKER_CLOSE_TIMEOUT: Duration =
    Duration::from_secs(EXECUTOR_TERMINATION_GRACE.as_secs() + 10);
const SUPERVISOR_CLOSE_TIMEOUT: Duration = Duration::from_secs(45);
const EVENT_BRIDGE_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Official BullMQ worker wrapper for one proof-supervisor process.
pub struct ProofSupervisor {
    shutdown: CancellationToken,
    lifecycle: Option<JoinHandle<Result<(), SupervisorRuntimeError>>>,
}

/// Crate-owned authority for test-only construction of the worker boundary.
pub(crate) struct ProofSupervisorActivation {
    _private: (),
}

impl ProofSupervisorActivation {
    #[cfg(test)]
    pub(crate) const fn library_plumbing() -> Self {
        Self { _private: () }
    }
}

impl ProofSupervisor {
    /// Starts the production worker with only the fixed local launcher and strict validator.
    pub async fn start_production(
        config: SupervisorConfig,
    ) -> Result<Self, SupervisorRuntimeError> {
        let owner_uid = std::fs::symlink_metadata(config.private_envelope_root())
            .map_err(|_| SupervisorRuntimeError::PrivateStorage)?
            .uid();
        let executor = UnixSocketExecutor::from_env(owner_uid)
            .map_err(|_| SupervisorRuntimeError::Configuration)?;
        let validator = TerminalResultValidator::new(
            TerminalValidatorConfig::from_env()
                .map_err(|_| SupervisorRuntimeError::Configuration)?,
        )
        .map_err(|_| SupervisorRuntimeError::Configuration)?;
        Self::start(
            config,
            Arc::new(executor),
            validator,
            ProofSupervisorActivation { _private: () },
        )
        .await
    }

    /// Starts the claim boundary only for code holding the crate-owned activation token.
    async fn start(
        config: SupervisorConfig,
        executor: Arc<dyn CapabilityExecutor>,
        validator: TerminalResultValidator,
        _activation: ProofSupervisorActivation,
    ) -> Result<Self, SupervisorRuntimeError> {
        Self::start_with_clock(
            config,
            executor,
            Arc::new(validator) as Arc<dyn CapabilityResultValidator>,
            Arc::new(SystemClock),
            _activation,
        )
        .await
    }

    pub(crate) async fn start_with_clock(
        config: SupervisorConfig,
        executor: Arc<dyn CapabilityExecutor>,
        validator: Arc<dyn CapabilityResultValidator>,
        clock: Arc<dyn TrustedClock>,
        _activation: ProofSupervisorActivation,
    ) -> Result<Self, SupervisorRuntimeError> {
        let root = config.private_envelope_root().to_owned();
        let store = tokio::task::spawn_blocking(move || PrivateEnvelopeStore::open(root))
            .await
            .map_err(|_| SupervisorRuntimeError::BlockingTaskFailed)?
            .map_err(|_| SupervisorRuntimeError::PrivateStorage)?;

        let proof_queue = Arc::new(
            Queue::with_options(CAPABILITY_QUEUE_NAME, config.queue_config().queue_options())
                .await
                .map_err(|_| SupervisorRuntimeError::QueueUnavailable)?,
        );
        if proof_queue
            .set_global_concurrency(GLOBAL_CONCURRENCY)
            .await
            .is_err()
        {
            proof_queue.close().await;
            return Err(SupervisorRuntimeError::QueueUnavailable);
        }

        let store: Arc<dyn EnvelopeAccess> = Arc::new(store);
        let processor = Arc::new(ClaimedJobProcessor::new(
            Arc::clone(&store),
            executor,
            validator,
            clock,
        ));
        let intents = Arc::new(CancellationIntentRegistry::default());
        let worker_processor = Arc::clone(&processor);
        let worker_intents = Arc::clone(&intents);
        let worker = Worker::with_options(
            CAPABILITY_QUEUE_NAME,
            move |mut job: Job, cancellation: CancellationToken| {
                let processor = Arc::clone(&worker_processor);
                let intents = Arc::clone(&worker_intents);
                async move {
                    let job_id = job.id().to_owned();
                    let reference = validate_claimed_job(&job).map_err(|failure| {
                        BullMqError::Unrecoverable(failure.public_class().redis_reason().to_owned())
                    })?;
                    let mut progress = BullMqProgressPublisher { job: &mut job };
                    match processor
                        .process(reference, cancellation, &mut progress)
                        .await
                    {
                        Ok(complete) => complete.into_result().to_public_value().map_err(|_| {
                            BullMqError::Unrecoverable(
                                PublicFailureClass::ServiceUnavailable
                                    .redis_reason()
                                    .to_owned(),
                            )
                        }),
                        Err(failure) => {
                            let mut public = failure.public_class();
                            if public == PublicFailureClass::Canceled
                                && !intents.take(&job_id).await
                            {
                                public = PublicFailureClass::ServiceUnavailable;
                            }
                            Err(BullMqError::Unrecoverable(public.redis_reason().to_owned()))
                        }
                    }
                }
            },
            config
                .queue_config()
                .worker_options()
                .name("razield-proof-supervisor")
                .manual_start(),
        )
        .await;
        let worker = worker.map_err(|_| SupervisorRuntimeError::QueueUnavailable)?;

        let worker = Arc::new(worker);
        let cancellation_queue = match Queue::with_options(
            CANCELLATION_QUEUE_NAME,
            config.queue_config().queue_options(),
        )
        .await
        {
            Ok(queue) => Arc::new(queue),
            Err(_) => {
                let _ = worker.close(WORKER_CLOSE_TIMEOUT.as_millis() as u64).await;
                proof_queue.close().await;
                return Err(SupervisorRuntimeError::QueueUnavailable);
            }
        };
        if cancellation_queue
            .set_global_concurrency(GLOBAL_CONCURRENCY)
            .await
            .is_err()
        {
            let _ = worker.close(WORKER_CLOSE_TIMEOUT.as_millis() as u64).await;
            proof_queue.close().await;
            cancellation_queue.close().await;
            return Err(SupervisorRuntimeError::QueueUnavailable);
        }
        let cancellation_processor = Arc::new(CancellationCommandProcessor::new(
            store,
            Arc::clone(&proof_queue),
            Arc::clone(&worker),
            intents,
        ));
        let cancellation_worker_processor = Arc::clone(&cancellation_processor);
        let cancellation_worker = Worker::with_options(
            CANCELLATION_QUEUE_NAME,
            move |job: Job, cancellation: CancellationToken| {
                let processor = Arc::clone(&cancellation_worker_processor);
                async move { processor.process(&job, cancellation).await }
            },
            config
                .queue_config()
                .cancellation_worker_options()
                .name("razield-cancellation-supervisor")
                .manual_start(),
        )
        .await;
        let cancellation_worker = match cancellation_worker {
            Ok(worker) => worker,
            Err(_) => {
                let _ = worker.close(WORKER_CLOSE_TIMEOUT.as_millis() as u64).await;
                proof_queue.close().await;
                cancellation_queue.close().await;
                return Err(SupervisorRuntimeError::QueueUnavailable);
            }
        };
        let cancellation_worker = Arc::new(cancellation_worker);
        if cancellation_worker.run().await.is_err() {
            let _ = cancellation_worker
                .close(WORKER_CLOSE_TIMEOUT.as_millis() as u64)
                .await;
            let _ = worker.close(WORKER_CLOSE_TIMEOUT.as_millis() as u64).await;
            proof_queue.close().await;
            cancellation_queue.close().await;
            return Err(SupervisorRuntimeError::WorkerStartFailed);
        }
        if worker.run().await.is_err() {
            let _ = cancellation_worker
                .close(WORKER_CLOSE_TIMEOUT.as_millis() as u64)
                .await;
            let _ = worker.close(WORKER_CLOSE_TIMEOUT.as_millis() as u64).await;
            proof_queue.close().await;
            cancellation_queue.close().await;
            return Err(SupervisorRuntimeError::WorkerStartFailed);
        }

        let proof_event_bridge = spawn_event_bridge(Arc::clone(&worker));
        let cancellation_event_bridge = spawn_event_bridge(Arc::clone(&cancellation_worker));
        let shutdown = CancellationToken::new();
        let lifecycle = spawn_lifecycle(
            Arc::clone(&worker),
            cancellation_worker,
            proof_queue,
            cancellation_queue,
            shutdown.clone(),
            proof_event_bridge,
            cancellation_event_bridge,
        );

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
    proof_worker: Arc<Worker>,
    cancellation_worker: Arc<Worker>,
    proof_queue: Arc<Queue>,
    cancellation_queue: Arc<Queue>,
    shutdown: CancellationToken,
    proof_event_bridge: JoinHandle<()>,
    cancellation_event_bridge: JoinHandle<()>,
) -> JoinHandle<Result<(), SupervisorRuntimeError>> {
    tokio::spawn(async move {
        shutdown.cancelled().await;
        cancellation_worker.cancel_all_jobs().await;
        proof_worker.cancel_all_jobs().await;
        cancellation_worker
            .close(WORKER_CLOSE_TIMEOUT.as_millis() as u64)
            .await
            .map_err(|_| SupervisorRuntimeError::WorkerCloseFailed)?;
        proof_worker
            .close(WORKER_CLOSE_TIMEOUT.as_millis() as u64)
            .await
            .map_err(|_| SupervisorRuntimeError::WorkerCloseFailed)?;
        proof_queue.close().await;
        cancellation_queue.close().await;
        if proof_worker.active_count().await != 0 || cancellation_worker.active_count().await != 0 {
            return Err(SupervisorRuntimeError::WorkerCloseIncomplete);
        }

        close_event_bridge(proof_event_bridge).await?;
        close_event_bridge(cancellation_event_bridge).await
    })
}

async fn close_event_bridge(
    mut event_bridge: JoinHandle<()>,
) -> Result<(), SupervisorRuntimeError> {
    match tokio::time::timeout(EVENT_BRIDGE_CLOSE_TIMEOUT, &mut event_bridge).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(SupervisorRuntimeError::WorkerLifecycleFailed),
        Err(_) => {
            event_bridge.abort();
            Err(SupervisorRuntimeError::WorkerLifecycleFailed)
        }
    }
}

/// Redacted process-boundary failures for supervisor startup and shutdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorRuntimeError {
    Configuration,
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
            Self::Configuration => "proof supervisor configuration is invalid",
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
