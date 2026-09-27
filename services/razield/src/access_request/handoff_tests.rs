//! Public delivery/lifecycle tests. Controlled responses do not claim SP1 verification.
use super::issuance_tests::{proposal, submission};
use super::*;
use crate::access_request::DispatchReference;
use raziel_proto::raziel::v1::{
    AccessRequest, ListAccessRequestsRequest, ProofJob, ProofJobState, PublicErrorCode,
};

mod worker {
    use super::*;
    use crate::access_dispatch::processor::{PrepareSnapshot, Processor};
    use crate::access_dispatch::proof::{ProofService, ProofServiceError};
    use crate::request_preparation::PreparedSnapshotProposal;
    use raziel_proto::raziel::v1::CreateCapabilityRequest;
    use std::sync::Mutex;

    struct Clock(i64);
    impl crate::service::TrustedClock for Clock {
        fn now_unix_seconds(&self) -> Result<i64, crate::service::ClockError> {
            Ok(self.0)
        }
    }
    struct UnusedPreparation;
    impl PrepareSnapshot for UnusedPreparation {
        fn prepare<'a>(
            &'a self,
            _: &'a mut AccessCoordinator,
            _: &'a DispatchReference,
            _: u64,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<PreparedSnapshotProposal, AccessError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async { Err(AccessError::ServiceUnavailable) })
        }
    }
    struct Service {
        public: AccessRequest,
        retained: Mutex<Option<ProofJob>>,
        requests: Mutex<Vec<CreateCapabilityRequest>>,
        mode: AtomicUsize,
        reads: AtomicUsize,
        cancels: AtomicUsize,
    }
    impl Service {
        fn new(public: AccessRequest) -> Self {
            Self {
                public,
                retained: Mutex::new(None),
                requests: Mutex::new(vec![]),
                mode: AtomicUsize::new(0),
                reads: AtomicUsize::new(0),
                cancels: AtomicUsize::new(0),
            }
        }
    }
    #[tonic::async_trait]
    impl ProofService for Service {
        async fn get(&self, _: &str) -> Result<ProofJob, ProofServiceError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.retained
                .lock()
                .unwrap()
                .clone()
                .ok_or(ProofServiceError::NotFound)
        }
        async fn create(
            &self,
            request: CreateCapabilityRequest,
        ) -> Result<ProofJob, ProofServiceError> {
            self.requests.lock().unwrap().push(request);
            match self.mode.load(Ordering::SeqCst) {
                0 => Err(ProofServiceError::Unavailable),
                3 => Err(ProofServiceError::Capacity),
                _ => {
                    let job = job(&self.public, ProofJobState::Queued);
                    *self.retained.lock().unwrap() = Some(job.clone());
                    if self.mode.load(Ordering::SeqCst) == 2 {
                        Err(ProofServiceError::Unavailable)
                    } else {
                        Ok(job)
                    }
                }
            }
        }
        async fn cancel(&self, _: &str) -> Result<ProofJob, ProofServiceError> {
            self.cancels.fetch_add(1, Ordering::SeqCst);
            let job = job(&self.public, ProofJobState::Canceled);
            *self.retained.lock().unwrap() = Some(job.clone());
            Ok(job)
        }
    }

    async fn processor(live: &Live, service: Arc<Service>, now: i64) -> Processor {
        let queues = crate::queue::QueueConfig::new(&live.url, live.config.prefix()).unwrap();
        Processor {
            config: live.config.clone(),
            preparation: Arc::new(UnusedPreparation),
            proof_queue: Arc::new(
                bullmq::Queue::with_options(
                    crate::queue::CAPABILITY_QUEUE_NAME,
                    queues.queue_options(),
                )
                .await
                .unwrap(),
            ),
            clock: Arc::new(Clock(now)),
            proof_service: service,
        }
    }

    #[tokio::test]
    #[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
    async fn live_dispatcher_resends_only_exact_input_then_recovers_public_result_without_reproving()
     {
        let live = Live::new();
        let mut owner = live.owner();
        let (reference, public) = accepted(&mut owner);
        let service = Arc::new(Service::new(public.clone()));
        let runner = processor(&live, service.clone(), NOW + 10).await;
        assert!(runner.proof_step(&reference).await.is_err());
        service.mode.store(1, Ordering::SeqCst);
        assert!(!runner.proof_step(&reference).await.unwrap());
        {
            let calls = service.requests.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].encode_to_vec(), calls[1].encode_to_vec());
        }
        assert_eq!(service.reads.load(Ordering::SeqCst), 2);
        runner.proof_queue.close().await;
        drop(runner);
        let runner = processor(&live, service.clone(), NOW + 11).await;
        *service.retained.lock().unwrap() = Some(delivery_fixture(&public));
        runner.proof_step(&reference).await.unwrap();
        assert_eq!(
            owner.get(AGENT, REQUEST, NOW + 11).unwrap().state,
            State::Ready as i32
        );
        *service.retained.lock().unwrap() = None;
        assert!(
            !owner
                .get(AGENT, REQUEST, NOW + 12)
                .unwrap()
                .capability
                .is_empty()
        );
        assert_eq!(service.requests.lock().unwrap().len(), 2);
        runner.proof_queue.close().await;
    }

    #[tokio::test]
    #[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
    async fn live_cancellation_waits_for_uncertain_create_and_blocks_every_resend() {
        let live = Live::new();
        let mut owner = live.owner();
        let (reference, public) = accepted(&mut owner);
        let service = Arc::new(Service::new(public.clone()));
        let runner = processor(&live, service.clone(), NOW + 10).await;
        assert!(runner.proof_step(&reference).await.is_err());
        let current = owner.get(AGENT, REQUEST, NOW + 10).unwrap();
        owner
            .cancel(AGENT, REQUEST, current.generation, NOW + 10)
            .unwrap();
        assert!(runner.proof_step(&reference).await.is_err());
        assert_eq!(service.requests.lock().unwrap().len(), 1);
        assert_eq!(
            owner.get(AGENT, REQUEST, NOW + 10).unwrap().state,
            State::Cancelling as i32
        );
        *service.retained.lock().unwrap() = Some(job(&public, ProofJobState::Proving));
        runner.proof_step(&reference).await.unwrap();
        assert_eq!(service.cancels.load(Ordering::SeqCst), 1);
        assert_eq!(
            owner.get(AGENT, REQUEST, NOW + 10).unwrap().state,
            State::Canceled as i32
        );
        runner.proof_queue.close().await;
    }

    #[tokio::test]
    #[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
    async fn live_unsent_cancellation_and_missed_window_finish_without_proof_calls() {
        for cancel in [false, true] {
            let live = Live::new();
            let mut owner = live.owner();
            let (reference, public) = accepted(&mut owner);
            if cancel {
                owner
                    .cancel(AGENT, REQUEST, public.generation, NOW + 5)
                    .unwrap();
            }
            let service = Arc::new(Service::new(public));
            let runner = processor(&live, service.clone(), NOW + 500).await;
            assert!(runner.proof_step(&reference).await.unwrap());
            assert_eq!(
                owner.get(AGENT, REQUEST, NOW + 500).unwrap().state,
                if cancel {
                    State::Canceled
                } else {
                    State::Failed
                } as i32
            );
            assert!(service.requests.lock().unwrap().is_empty());
            assert_eq!(service.cancels.load(Ordering::SeqCst), 0);
            runner.proof_queue.close().await;
        }
    }
}

fn accepted(owner: &mut AccessCoordinator) -> (DispatchReference, AccessRequest) {
    let public = proposal(owner);
    let accepted = owner
        .submit_issuance(submission(&public, 4), NOW + 4)
        .unwrap();
    (
        DispatchReference {
            agent_id: AGENT.into(),
            request_id: REQUEST.into(),
            generation: 2,
        },
        accepted,
    )
}

fn job(public: &AccessRequest, state: ProofJobState) -> ProofJob {
    ProofJob {
        job_id: public.proof_job_id.clone().unwrap(),
        state: state as i32,
        submitted_at: Some(prost_types::Timestamp {
            seconds: NOW + 5,
            nanos: 0,
        }),
        updated_at: Some(prost_types::Timestamp {
            seconds: NOW + 6,
            nanos: 0,
        }),
        public_error_code: if state == ProofJobState::Failed {
            PublicErrorCode::ServiceUnavailable
        } else {
            PublicErrorCode::Unspecified
        } as i32,
        ..Default::default()
    }
}

// Opaque bytes exercise delivery only. There is no production mock or READY shortcut.
fn delivery_fixture(public: &AccessRequest) -> ProofJob {
    use raziel_proto::raziel::v1::{ProofCapability, ProofMetrics, ProofSystem};
    let proposal = public.proposal.as_ref().unwrap();
    let binding = proposal.policy_binding.as_ref().unwrap();
    let id = public.proof_job_id.clone().unwrap();
    let commitment = id
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    ProofJob {
        capability: Some(ProofCapability {
            format_version: 1,
            capability_id: id,
            agent_key_thumbprint: raziel_authority::derive_ed25519_key_thumbprint(
                &proposal.agent_public_key.as_slice().try_into().unwrap(),
            )
            .to_vec(),
            task_id: proposal.task_id.clone(),
            purpose: proposal.purpose.clone(),
            action: proposal.action.clone(),
            resource_type: binding.resource_type.clone(),
            resource_scope: binding.resource_scope.clone(),
            policy_hash: binding.policy_hash.clone(),
            relationship_snapshot_root: proposal.snapshot_root.clone(),
            snapshot_id: proposal.snapshot_id.clone(),
            capability_request_commitment: commitment,
            issuance_nullifier: vec![4; 32],
            issued_at: Some(prost_types::Timestamp {
                seconds: NOW + 5,
                nanos: 0,
            }),
            expires_at: Some(prost_types::Timestamp {
                seconds: NOW + 4000,
                nanos: 0,
            }),
            proof_system: ProofSystem::Sp1 as i32,
            verification_key_id: format!("sp1-vk-v1:{}", "f".repeat(64)),
            zk_proof: b"opaque-delivery-fixture-not-an-sp1-proof".to_vec(),
            snapshot_attestation: Some(super::dispatch_tests::prepared(3).snapshot),
        }),
        metrics: Some(ProofMetrics {
            execution_duration: Some(prost_types::Duration {
                seconds: 1,
                nanos: 0,
            }),
            proving_duration: Some(prost_types::Duration {
                seconds: 900,
                nanos: 0,
            }),
            execution_cycle_count: 0,
            proof_size_bytes: b"opaque-delivery-fixture-not-an-sp1-proof".len() as u64,
        }),
        ..job(public, ProofJobState::Succeeded)
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_public_result_survives_restart_and_is_deleted_at_seven_day_retention() {
    let live = Live::new();
    let mut owner = live.owner();
    let (reference, public) = accepted(&mut owner);
    assert!(owner.begin_submission(&reference, NOW + 5).unwrap().first);
    owner
        .record_proof_job(&reference, job(&public, ProofJobState::Queued), NOW + 6)
        .unwrap();
    owner
        .record_proof_job(&reference, job(&public, ProofJobState::Proving), NOW + 7)
        .unwrap();
    assert!(
        owner
            .record_proof_job(&reference, job(&public, ProofJobState::Queued), NOW + 7)
            .is_err()
    );
    owner
        .record_proof_job(&reference, delivery_fixture(&public), NOW + 8)
        .unwrap();
    drop(owner);
    let mut owner = live.owner();
    let ready = owner.get(AGENT, REQUEST, NOW + 9).unwrap();
    assert_eq!(ready.state, State::Ready as i32);
    assert!(!ready.capability.is_empty());
    assert_eq!(ready.metrics, delivery_fixture(&public).metrics);
    let listed = owner
        .list(
            ListAccessRequestsRequest {
                agent_id: Some(AGENT.into()),
                limit: 50,
                cursor: None,
            },
            NOW + 9,
        )
        .unwrap();
    assert!(listed.requests[0].capability.is_empty());
    assert_eq!(listed.requests[0].metrics, ready.metrics);
    assert_eq!(
        owner
            .submit_issuance(submission(&public, 4), NOW + 9)
            .unwrap(),
        ready
    );
    assert_eq!(owner.cancel(AGENT, REQUEST, 1, NOW + 9).unwrap(), ready);
    assert!(owner.dispatch_candidates(NOW + 9).unwrap().is_empty());
    assert!(
        owner
            .record_proof_job(&reference, job(&public, ProofJobState::Failed), NOW + 9)
            .is_err()
    );
    owner.reconcile(NOW + 8 + RETENTION_SECONDS).unwrap();
    let results: i64 = redis::cmd("HLEN")
        .arg(live.key("results"))
        .query(&mut live.connection())
        .unwrap();
    assert_eq!(results, 0);
    assert_eq!(fs::read_dir(live.config.private_root()).unwrap().count(), 1);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_delivery_rejects_wrong_owner_scope_or_malformed_proof_service_result() {
    let live = Live::new();
    let mut owner = live.owner();
    let (reference, public) = accepted(&mut owner);
    owner.begin_submission(&reference, NOW + 5).unwrap();
    for mutation in 0..10 {
        let mut result = delivery_fixture(&public);
        match mutation {
            0 => result.job_id = "a".repeat(64),
            1 => result.capability.as_mut().unwrap().agent_key_thumbprint[0] ^= 1,
            2 => result.capability.as_mut().unwrap().policy_hash[0] ^= 1,
            3 => result.capability.as_mut().unwrap().resource_scope.push('x'),
            4 => result.capability.as_mut().unwrap().snapshot_id.push('x'),
            5 => {
                result
                    .capability
                    .as_mut()
                    .unwrap()
                    .capability_request_commitment[0] ^= 1
            }
            6 => result.metrics.as_mut().unwrap().proof_size_bytes += 1,
            7 => result.internal_error_ref = Some("private-trace".into()),
            8 => {
                result
                    .capability
                    .as_mut()
                    .unwrap()
                    .expires_at
                    .as_mut()
                    .unwrap()
                    .seconds += 5000
            }
            _ => result.state = 100,
        }
        assert!(
            owner.record_proof_job(&reference, result, NOW + 6).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(
            owner.get(AGENT, REQUEST, NOW + 6).unwrap().state,
            State::Submitting as i32
        );
    }
    owner
        .record_proof_job(&reference, job(&public, ProofJobState::Failed), NOW + 7)
        .unwrap();
    assert_eq!(
        owner
            .get(AGENT, REQUEST, NOW + 8)
            .unwrap()
            .error_code
            .as_deref(),
        Some("SERVICE_UNAVAILABLE")
    );
    let _: i64 = redis::cmd("HDEL")
        .arg(live.key("results"))
        .arg(format!("{AGENT}:{REQUEST}"))
        .query(&mut live.connection())
        .unwrap();
    assert_eq!(
        owner.get(AGENT, REQUEST, NOW + 8).unwrap_err(),
        AccessError::ServiceUnavailable
    );
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_uncertain_publication_cannot_be_canceled_or_cleared_by_an_older_response() {
    let live = Live::new();
    let mut owner = live.owner();
    let (reference, _) = accepted(&mut owner);
    let first = owner.begin_submission(&reference, NOW + 5).unwrap();
    let second = owner.begin_submission(&reference, NOW + 6).unwrap();
    assert!(first.first);
    assert!(!second.first);
    assert!(
        owner
            .reject_first_submission(&reference, first.sequence, NOW + 7)
            .is_err()
    );
    let record = owner.get(AGENT, REQUEST, NOW + 7).unwrap();
    owner
        .cancel(AGENT, REQUEST, record.generation, NOW + 7)
        .unwrap();
    assert!(owner.finish_unsubmitted(&reference, NOW + 8).is_err());
    assert!(owner.begin_submission(&reference, NOW + 8).is_err());
    // The review deadline cannot fabricate proof termination or discard the comparator.
    assert_eq!(
        owner
            .get(AGENT, REQUEST, NOW + REVIEW_SECONDS)
            .unwrap()
            .state,
        State::Cancelling as i32
    );
}
