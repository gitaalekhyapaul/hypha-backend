use std::sync::Arc;
use std::time::Duration;

use bullmq::worker::{CancellationToken, WorkerEvent};
use bullmq::{Queue, Worker};
use raziel_admin::request_provisioning::ProvisioningConfig;
use tokio::task::JoinHandle;

use super::{
    processor::{Processor, clock},
    queue,
};
use crate::access_request::{AccessError, AccessServiceConfig};
use crate::queue::{CAPABILITY_QUEUE_NAME, QueueConfig};
use crate::request_preparation::RequestPreparation;
use crate::service::SnapshotServiceConfig;

/// Separate from the proof supervisor. Construction requires the real private
/// snapshot preparer. No mock-proof switch exists in the production API.
pub struct AccessDispatcher {
    stop: CancellationToken,
    lifecycle: Option<JoinHandle<Result<(), AccessError>>>,
}

impl AccessDispatcher {
    pub async fn start(
        config: AccessServiceConfig,
        queues: QueueConfig,
        preparation: RequestPreparation,
        proof_endpoint: String,
    ) -> Result<Self, AccessError> {
        let proof_service = Arc::new(super::proof::RpcProofService::new(proof_endpoint)?);
        let proof_queue = Arc::new(
            Queue::with_options(CAPABILITY_QUEUE_NAME, queues.queue_options())
                .await
                .map_err(unavailable)?,
        );
        let processor = Arc::new(Processor {
            config,
            preparation: Arc::new(preparation),
            proof_queue,
            clock: clock(),
            proof_service,
        });
        Self::start_processor(queues, processor).await
    }

    pub(crate) async fn start_processor(
        queues: QueueConfig,
        processor: Arc<Processor>,
    ) -> Result<Self, AccessError> {
        let proof_queue = processor.proof_queue.clone();
        let queue = match Queue::with_options(queue::NAME, queues.queue_options()).await {
            Ok(queue) => Arc::new(queue),
            Err(error) => {
                proof_queue.close().await;
                return Err(unavailable(error));
            }
        };
        if let Err(error) = queue.set_global_concurrency(1).await {
            queue.close().await;
            proof_queue.close().await;
            return Err(unavailable(error));
        }
        let runner = processor.clone();
        let worker = Worker::with_options(
            queue::NAME,
            move |job, cancel| {
                let runner = runner.clone();
                async move {
                    let reference = queue::reference(&job).map_err(worker_error)?;
                    runner
                        .process(reference, cancel)
                        .await
                        .map_err(worker_error)?;
                    Ok(serde_json::Value::Null)
                }
            },
            queues
                .worker_options()
                .name("razield-access-dispatcher")
                .manual_start(),
        )
        .await;
        let worker = match worker {
            Ok(worker) => Arc::new(worker),
            Err(error) => {
                queue.close().await;
                proof_queue.close().await;
                return Err(unavailable(error));
            }
        };
        if let Err(error) = worker.run().await {
            let _ = worker.close(30_000).await;
            queue.close().await;
            proof_queue.close().await;
            return Err(unavailable(error));
        }
        let stop = CancellationToken::new();
        let monitor_worker = worker.clone();
        let events = tokio::spawn(async move {
            loop {
                match monitor_worker.next_event().await {
                    Some(WorkerEvent::Stalled { job_id }) => {
                        let _ = monitor_worker.cancel_job(&job_id).await;
                    }
                    Some(WorkerEvent::Closed) | None => break,
                    _ => {}
                }
            }
        });
        let scanning_queue = queue.clone();
        let stopping = stop.clone();
        let maintenance = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                tokio::select! {
                    _ = stopping.cancelled() => break,
                    _ = interval.tick() => {
                        if repair(&processor, &scanning_queue).await.is_err() {
                            eprintln!("access dispatch reconciliation unavailable");
                        }
                    }
                }
            }
        });
        let shutdown = stop.clone();
        let lifecycle = tokio::spawn(async move {
            shutdown.cancelled().await;
            let maintenance_result = maintenance
                .await
                .map_err(|_| AccessError::ServiceUnavailable);
            worker.cancel_all_jobs().await;
            let worker_result = worker.close(30_000).await.map_err(unavailable);
            events.abort();
            let _ = events.await;
            queue.close().await;
            proof_queue.close().await;
            maintenance_result.and(worker_result)
        });
        Ok(Self {
            stop,
            lifecycle: Some(lifecycle),
        })
    }

    pub async fn close(mut self) -> Result<(), AccessError> {
        self.stop.cancel();
        self.lifecycle
            .take()
            .ok_or(AccessError::ServiceUnavailable)?
            .await
            .map_err(|_| AccessError::ServiceUnavailable)?
    }
}

impl Drop for AccessDispatcher {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

pub async fn run_from_env() -> Result<(), AccessError> {
    let access = AccessServiceConfig::from_env()?;
    let approval = access.approval().ok_or(AccessError::Configuration)?;
    let snapshots = SnapshotServiceConfig::from_env().map_err(|_| AccessError::Configuration)?;
    let required = |key| std::env::var(key).map_err(|_| AccessError::Configuration);
    let queues = QueueConfig::new(access.redis_url(), required("RAZIEL_QUEUE_PREFIX")?)
        .map_err(|_| AccessError::Configuration)?;
    let provisioning = ProvisioningConfig {
        publication_root: approval.publication_root.clone(),
        package_root: approval.package_root.clone(),
        registry_root: approval.registry_root.clone(),
        binding_root: approval.binding_root.clone(),
        compiler: required("RAZIEL_POLICY_COMPILER_EXECUTABLE")?.into(),
        token_path: snapshots.spicedb_auth_token_path().into(),
        endpoint: snapshots.spicedb_endpoint().into(),
        trust_domain_id: approval.trust_domain_id.clone(),
    };
    let preparation =
        tokio::task::spawn_blocking(move || RequestPreparation::open(provisioning, snapshots))
            .await
            .map_err(|_| AccessError::ServiceUnavailable)??;
    let dispatcher = AccessDispatcher::start(
        access,
        queues,
        preparation,
        required("RAZIEL_RAZIELD_ENDPOINT")?,
    )
    .await?;
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
    dispatcher.close().await
}

async fn repair(processor: &Processor, queue: &Queue) -> Result<(), AccessError> {
    let candidates = processor
        .owner(|owner, now| owner.dispatch_candidates(now))
        .await?;
    // Waiting requests are already durable. Enqueue one reference at a time so a
    // recovered reservation cannot sit behind jobs it must keep from preparing.
    if let Some(reference) = candidates.first() {
        queue::enqueue(queue, reference).await?;
    }
    Ok(())
}

fn unavailable(_: bullmq::Error) -> AccessError {
    AccessError::ServiceUnavailable
}
fn worker_error(_: AccessError) -> bullmq::Error {
    bullmq::Error::Unrecoverable("SERVICE_UNAVAILABLE".into())
}
