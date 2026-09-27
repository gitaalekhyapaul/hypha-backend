use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bullmq::Queue;
use bullmq::worker::CancellationToken;
use raziel_proto::raziel::v1::AccessRequestState as State;

use crate::access_request::{
    AccessCoordinator, AccessError, AccessServiceConfig, DispatchReference,
};
use crate::queue::PROOF_CAPACITY_DEDUPLICATION_ID;
use crate::request_preparation::{PreparedSnapshotProposal, RequestPreparation};
use crate::service::{SystemClock, TrustedClock};

pub(crate) trait PrepareSnapshot: Send + Sync {
    fn prepare<'a>(
        &'a self,
        owner: &'a mut AccessCoordinator,
        reference: &'a DispatchReference,
        generation: u64,
    ) -> Pin<Box<dyn Future<Output = Result<PreparedSnapshotProposal, AccessError>> + Send + 'a>>;
}

impl PrepareSnapshot for RequestPreparation {
    fn prepare<'a>(
        &'a self,
        owner: &'a mut AccessCoordinator,
        reference: &'a DispatchReference,
        generation: u64,
    ) -> Pin<Box<dyn Future<Output = Result<PreparedSnapshotProposal, AccessError>> + Send + 'a>>
    {
        Box::pin(self.prepare(
            owner,
            &reference.agent_id,
            &reference.request_id,
            generation,
            reference.generation,
        ))
    }
}

pub(crate) struct Processor {
    pub config: AccessServiceConfig,
    pub preparation: Arc<dyn PrepareSnapshot>,
    pub proof_queue: Arc<Queue>,
    pub clock: Arc<dyn TrustedClock>,
    pub proof_service: Arc<dyn super::proof::ProofService>,
}

impl Processor {
    pub async fn process(
        &self,
        reference: DispatchReference,
        cancel: CancellationToken,
    ) -> Result<(), AccessError> {
        let mut prepared = None;
        loop {
            if cancel.is_cancelled() {
                return Err(AccessError::ServiceUnavailable);
            }
            match self.step(&reference, &mut prepared).await {
                Ok(true) => return Ok(()),
                Ok(false)
                | Err(
                    AccessError::ServiceUnavailable
                    | AccessError::CapacityExceeded
                    | AccessError::Conflict,
                ) => {}
                Err(AccessError::NotFound | AccessError::Expired) => return Ok(()),
                Err(error) => return Err(error),
            }
            tokio::select! {
                _ = cancel.cancelled() => return Err(AccessError::ServiceUnavailable),
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        }
    }

    async fn step(
        &self,
        reference: &DispatchReference,
        retained: &mut Option<PreparedSnapshotProposal>,
    ) -> Result<bool, AccessError> {
        let lookup = reference.clone();
        let Some(current) = self
            .owner(move |owner, now| owner.dispatch_request(&lookup, now))
            .await?
        else {
            return Ok(true);
        };
        let state = State::try_from(current.state).map_err(|_| AccessError::ServiceUnavailable)?;
        match state {
            State::ApprovedWaiting => {
                if current.generation != reference.generation {
                    return Ok(true);
                }
                // The sole M6 dispatcher is the proof producer. Existing jobs from
                // an earlier process must finish before another snapshot is built.
                if self
                    .proof_queue
                    .get_deduplication_job_id(PROOF_CAPACITY_DEDUPLICATION_ID)
                    .await
                    .map_err(unavailable)?
                    .is_some()
                    || self
                        .proof_queue
                        .get_job_count_by_types(&[
                            "wait",
                            "active",
                            "delayed",
                            "prioritized",
                            "waiting-children",
                        ])
                        .await
                        .map_err(unavailable)?
                        != 0
                {
                    return Ok(false);
                }
                let selected = reference.clone();
                self.owner(move |owner, now| owner.reserve_preparation(&selected, now))
                    .await?;
                Ok(false)
            }
            State::Preparing => {
                // A busy public-record lock must not repeat successful preparation.
                // Restart recovery still reads the immutable owner snapshot receipt.
                if let Some(prepared) = retained.as_ref() {
                    let selected = reference.clone();
                    let prepared = prepared.clone();
                    self.owner(move |owner, now| owner.publish_proposal(&selected, prepared, now))
                        .await?;
                    *retained = None;
                    return Ok(false);
                }
                // Open a separate coordinator so API cancel/read calls never wait
                // behind the networked preparation operation's process-local mutex.
                let config = self.config.clone();
                let mut owner =
                    tokio::task::spawn_blocking(move || AccessCoordinator::open(&config))
                        .await
                        .map_err(|_| AccessError::ServiceUnavailable)??;
                let prepared = self
                    .preparation
                    .prepare(&mut owner, reference, current.generation)
                    .await;
                let selected = reference.clone();
                match prepared {
                    Ok(prepared) => {
                        *retained = Some(prepared);
                    }
                    Err(AccessError::ServiceUnavailable) => return Ok(false),
                    Err(_) => {
                        self.owner(move |owner, now| {
                            owner.finish_preparation(&selected, true, now)
                        })
                        .await?;
                    }
                }
                Ok(false)
            }
            State::AwaitingSignature => Ok(false),
            State::Cancelling if current.proposal.is_none() => {
                let selected = reference.clone();
                self.owner(move |owner, now| owner.finish_preparation(&selected, false, now))
                    .await?;
                Ok(true)
            }
            State::Submitting
            | State::Queued
            | State::Executing
            | State::Proving
            | State::Cancelling => self.proof_step(reference).await,
            State::WaitingForAgent
            | State::Ready
            | State::Rejected
            | State::Canceled
            | State::Expired
            | State::Failed => Ok(true),
            // Signed-attempt states cannot release capacity without proof reconciliation.
            _ => Err(AccessError::ServiceUnavailable),
        }
    }

    pub async fn owner<T: Send + 'static>(
        &self,
        call: impl FnOnce(&mut AccessCoordinator, i64) -> Result<T, AccessError> + Send + 'static,
    ) -> Result<T, AccessError> {
        let config = self.config.clone();
        let clock = self.clock.clone();
        tokio::task::spawn_blocking(move || {
            let now = clock
                .now_unix_seconds()
                .map_err(|_| AccessError::ServiceUnavailable)?;
            call(&mut AccessCoordinator::open(&config)?, now)
        })
        .await
        .map_err(|_| AccessError::ServiceUnavailable)?
    }
}

pub(super) fn clock() -> Arc<dyn TrustedClock> {
    Arc::new(SystemClock)
}
fn unavailable(_: bullmq::Error) -> AccessError {
    AccessError::ServiceUnavailable
}
