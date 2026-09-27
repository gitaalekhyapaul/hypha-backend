use super::*;
use crate::access_request::DispatchReference;
use crate::request_preparation::PreparedSnapshotProposal;

const SECOND: &str = "00000000-0000-4000-8000-000000000002";

pub(super) fn approve(owner: &mut AccessCoordinator, request: &str) -> DispatchReference {
    let mut input = input();
    input.intent.as_mut().unwrap().request_id = request.into();
    owner.create(input, NOW).unwrap();
    let mut command = command();
    command.request_id = request.into();
    owner.approve(command, NOW + 1).unwrap();
    DispatchReference {
        agent_id: AGENT.into(),
        request_id: request.into(),
        generation: 2,
    }
}

pub(super) fn prepared(generation: u64) -> PreparedSnapshotProposal {
    PreparedSnapshotProposal {
        request_generation: generation,
        proposal_generation: 2,
        agent_public_key: input().owner.unwrap().agent_public_key,
        task_id: "reconcile".into(),
        purpose: "settlement review".into(),
        action: "query".into(),
        policy_binding: binding(),
        snapshot: raziel_proto::raziel::v1::RelationshipSnapshotDescriptor {
            snapshot_id: format!("snapshot-v1:{}", "09".repeat(32)),
            relationship_snapshot_root: vec![9; 32],
            policy_record_id: binding().policy_record_id,
            policy_hash: binding().policy_hash,
            expires_at: Some(prost_types::Timestamp {
                seconds: NOW + REVIEW_SECONDS,
                nanos: 0,
            }),
            ..Default::default()
        },
    }
}

#[test]
fn dispatch_payload_is_owner_qualified_and_bounded() {
    let first = DispatchReference {
        agent_id: AGENT.into(),
        request_id: REQUEST.into(),
        generation: u64::MAX,
    };
    first.validate().unwrap();
    assert!(serde_json::to_vec(&first).unwrap().len() <= 256);
    let second = DispatchReference {
        agent_id: "2".repeat(32),
        ..first.clone()
    };
    assert_ne!(first.job_id(), second.job_id());
    assert!(!first.job_id().contains(':'));
    assert!(serde_json::from_str::<DispatchReference>(
        &format!("{{\"agent_id\":\"{AGENT}\",\"request_id\":\"{REQUEST}\",\"generation\":2,\"context\":\"secret\"}}")
    ).is_err());
}

#[test]
fn proposal_record_fits_the_bound_with_maximum_public_fields() {
    use crate::access_request::approval::PublicBinding;
    use crate::access_request::dispatch_model::PublicProposal;
    use crate::access_request::model::Phase;

    let mut original = input();
    let intent = original.intent.as_mut().unwrap();
    intent.task_id = "\"".repeat(256);
    intent.purpose = "\\".repeat(256);
    intent.dataset_id = "d".repeat(128);
    original.dataset.as_mut().unwrap().dataset_id = intent.dataset_id.clone();
    let original = NormalizedIntent::parse(original).unwrap();
    let mut record = Record::reserved(&original, NOW).unwrap();
    record.phase = Phase::Published;
    record.state = State::AwaitingSignature as i32;
    record.generation = 4;
    record.dispatch_generation = Some(2);
    record.updated = NOW + 3;
    record.policy_binding = Some(
        PublicBinding::from_proto(&AccessPolicyBinding {
            policy_record_id: "p".repeat(128),
            resource_scope: record.dataset_id.clone(),
            ..binding()
        })
        .unwrap(),
    );
    record.proposal = Some(PublicProposal {
        proposal_id: "a".repeat(32),
        generation: 4,
        created: NOW + 3,
        signature_deadline: NOW + 123,
        agent_public_key: original.0.owner.unwrap().agent_public_key,
        snapshot_id: format!("snapshot-v1:{}", "ff".repeat(32)),
        snapshot_root: vec![255; 32],
        requested_expiry: NOW + 4803,
    });
    let encoded = record.encode().unwrap();
    let decoded = Record::decode(&record.identity(), &encoded).unwrap();
    assert_eq!(decoded.public(), record.public());
    assert_eq!(decoded.public().proposal.unwrap().purpose, record.purpose);
    record.state = State::Submitting as i32;
    record.generation += 1;
    record.attempt = Some(crate::access_request::issuance_model::PublicAttempt {
        job_id: "f".repeat(64),
        accepted_at: NOW + 3,
        submission_started_at: None,
        proof_state: 0,
        submission_count: 0,
    });
    Record::decode(&record.identity(), &record.encode().unwrap()).unwrap();
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_serial_reservation_and_signing_timeout_survive_restart() {
    let live = Live::new();
    let mut owner = live.owner();
    let first = approve(&mut owner, REQUEST);
    let second = approve(&mut owner, SECOND);
    assert_eq!(
        owner.dispatch_candidates(NOW + 2).unwrap(),
        vec![first.clone(), second.clone()]
    );
    let preparing = owner.reserve_preparation(&first, NOW + 2).unwrap();
    assert_eq!(preparing.state, State::Preparing as i32);
    assert_eq!(preparing.generation, 3);
    assert_eq!(
        owner.reserve_preparation(&second, NOW + 2).unwrap_err(),
        AccessError::CapacityExceeded
    );
    drop(owner);
    let mut owner = live.owner();
    assert_eq!(
        owner.reserve_preparation(&first, NOW + 3).unwrap(),
        preparing
    );
    let public = owner
        .publish_proposal(&first, prepared(3), NOW + 3)
        .unwrap();
    let proposal = public.proposal.as_ref().unwrap();
    assert_eq!(proposal.generation, public.generation);
    assert_eq!(proposal.requested_expires_at_unix_seconds, NOW + 4803);
    assert_eq!(proposal.signature_deadline_unix_seconds, NOW + 123);
    assert!(public.capability.is_empty());
    assert_eq!(owner.get(AGENT, REQUEST, NOW + 122).unwrap(), public);
    let waiting = owner.get(AGENT, REQUEST, NOW + 123).unwrap();
    assert_eq!(waiting.state, State::WaitingForAgent as i32);
    assert!(waiting.proposal.is_none());
    assert_eq!(
        owner.dispatch_candidates(NOW + 123).unwrap(),
        vec![second.clone()]
    );
    owner.reserve_preparation(&second, NOW + 123).unwrap();
    assert!(owner.dispatch_request(&first, NOW + 123).unwrap().is_none());
    let resumed = owner
        .resume(AGENT, REQUEST, waiting.generation, NOW + 124)
        .unwrap();
    assert_eq!(resumed.state, State::ApprovedWaiting as i32);
    assert_eq!(
        owner
            .resume(AGENT, REQUEST, waiting.generation, NOW + 125)
            .unwrap(),
        resumed
    );
    assert_eq!(
        owner
            .publish_proposal(&first, prepared(3), NOW + 125)
            .unwrap_err(),
        AccessError::Conflict
    );
    assert_eq!(owner.dispatch_candidates(NOW + 125).unwrap()[0], second);
    assert_eq!(
        owner.reserve_preparation(&first, NOW + 125).unwrap_err(),
        AccessError::Conflict
    );
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_cancel_during_preparation_fences_snapshot_and_holds_capacity_until_ack() {
    let live = Live::new();
    let mut owner = live.owner();
    let first = approve(&mut owner, REQUEST);
    let second = approve(&mut owner, SECOND);
    owner.reserve_preparation(&first, NOW + 2).unwrap();
    let retained = owner
        .approved_preparation(AGENT, REQUEST, 3, NOW + 2)
        .unwrap();
    let cancelled = owner.cancel(AGENT, REQUEST, 3, NOW + 3).unwrap();
    assert_eq!(cancelled.state, State::Cancelling as i32);
    assert_eq!(owner.cancel(AGENT, REQUEST, 3, NOW + 4).unwrap(), cancelled);
    assert_eq!(
        owner.confirm_preparation(&retained, NOW + 4),
        Err(AccessError::Conflict)
    );
    assert_eq!(
        owner
            .publish_proposal(&first, prepared(3), NOW + 4)
            .unwrap_err(),
        AccessError::Conflict
    );
    assert_eq!(
        owner.reserve_preparation(&second, NOW + 4).unwrap_err(),
        AccessError::CapacityExceeded
    );
    assert_eq!(
        owner
            .finish_preparation(&first, false, NOW + 5)
            .unwrap()
            .state,
        State::Canceled as i32
    );
    owner.reserve_preparation(&second, NOW + 5).unwrap();
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_proposal_rejects_changed_key_scope_generation_and_short_snapshot() {
    let live = Live::new();
    let mut owner = live.owner();
    let first = approve(&mut owner, REQUEST);
    owner.reserve_preparation(&first, NOW + 2).unwrap();
    for mutation in 0..6 {
        let mut value = prepared(3);
        match mutation {
            0 => {
                value.agent_public_key = SigningKey::from_bytes(&[8; 32])
                    .verifying_key()
                    .to_bytes()
                    .to_vec()
            }
            1 => value.policy_binding.resource_scope = "another-ledger".into(),
            2 => value.proposal_generation += 1,
            3 => value.request_generation += 1,
            4 => value.snapshot.expires_at.as_mut().unwrap().seconds = NOW + 4799,
            _ => value.snapshot.relationship_snapshot_root[0] ^= 1,
        }
        assert!(owner.publish_proposal(&first, value, NOW + 3).is_err());
        assert_eq!(
            owner.get(AGENT, REQUEST, NOW + 3).unwrap().state,
            State::Preparing as i32
        );
    }
    owner
        .publish_proposal(&first, prepared(3), NOW + 3)
        .unwrap();
    let raw = live.raw();
    for private in [
        "private-approval-team",
        "private-approval-org",
        "approved_amount",
        "request_context",
    ] {
        assert!(!raw.contains(private));
    }
    let current = owner.get(AGENT, REQUEST, NOW + 4).unwrap();
    assert_eq!(
        owner
            .cancel(AGENT, REQUEST, current.generation, NOW + 4)
            .unwrap()
            .state,
        State::Canceled as i32
    );
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_preparation_expiry_cannot_be_revived_by_late_result() {
    let live = Live::new();
    let mut owner = live.owner();
    let reference = approve(&mut owner, REQUEST);
    owner.reserve_preparation(&reference, NOW + 2).unwrap();
    let expired = owner.get(AGENT, REQUEST, NOW + REVIEW_SECONDS).unwrap();
    assert_eq!(expired.state, State::Expired as i32);
    assert!(
        owner
            .publish_proposal(&reference, prepared(3), NOW + REVIEW_SECONDS)
            .is_err()
    );
    assert!(
        owner
            .dispatch_candidates(NOW + REVIEW_SECONDS)
            .unwrap()
            .is_empty()
    );
}

#[cfg(feature = "access-dispatch")]
mod worker {
    use super::*;
    use crate::access_dispatch::{
        AccessDispatcher,
        processor::{PrepareSnapshot, Processor},
    };
    use crate::queue::{CAPABILITY_QUEUE_NAME, QueueConfig};
    use crate::service::{ClockError, TrustedClock};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::AtomicI64;
    use std::time::Duration;

    struct Clock(Arc<AtomicI64>);
    impl TrustedClock for Clock {
        fn now_unix_seconds(&self) -> Result<i64, ClockError> {
            Ok(self.0.load(Ordering::SeqCst))
        }
    }

    // Only the snapshot boundary is controlled. This never creates a proof or READY state.
    struct Preparation(Arc<AtomicUsize>, Arc<AtomicI64>);
    impl PrepareSnapshot for Preparation {
        fn prepare<'a>(
            &'a self,
            owner: &'a mut AccessCoordinator,
            reference: &'a DispatchReference,
            generation: u64,
        ) -> Pin<Box<dyn Future<Output = Result<PreparedSnapshotProposal, AccessError>> + Send + 'a>>
        {
            Box::pin(async move {
                owner.approved_preparation(
                    &reference.agent_id,
                    &reference.request_id,
                    generation,
                    self.1.load(Ordering::SeqCst),
                )?;
                self.0.fetch_add(1, Ordering::SeqCst);
                let mut value = prepared(generation);
                value.proposal_generation = reference.generation;
                Ok(value)
            })
        }
    }

    async fn start(
        live: &Live,
        clock: Arc<AtomicI64>,
        calls: Arc<AtomicUsize>,
    ) -> AccessDispatcher {
        let queues = QueueConfig::new(&live.url, live.config.prefix()).unwrap();
        let proof_queue = Arc::new(
            bullmq::Queue::with_options(CAPABILITY_QUEUE_NAME, queues.queue_options())
                .await
                .unwrap(),
        );
        AccessDispatcher::start_processor(
            queues,
            Arc::new(Processor {
                config: live.config.clone(),
                preparation: Arc::new(Preparation(calls, clock.clone())),
                proof_queue,
                clock: Arc::new(Clock(clock)),
                proof_service: Arc::new(
                    crate::access_dispatch::proof::RpcProofService::new(
                        "http://127.0.0.1:1".into(),
                    )
                    .unwrap(),
                ),
            }),
        )
        .await
        .unwrap()
    }

    async fn await_state(
        owner: &mut AccessCoordinator,
        request: &str,
        wanted: State,
        clock: &AtomicI64,
    ) -> raziel_proto::raziel::v1::AccessRequest {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Ok(value) = owner.get(AGENT, request, clock.load(Ordering::SeqCst))
                    && value.state == wanted as i32
                {
                    return value;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("bounded dispatcher state wait")
    }

    #[tokio::test]
    #[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
    async fn live_bullmq_serial_dispatch_recovers_proposal_and_releases_offline_signer() {
        let live = Live::new();
        let mut owner = live.owner();
        let first = approve(&mut owner, REQUEST);
        approve(&mut owner, SECOND);
        let clock = Arc::new(AtomicI64::new(NOW + 2));
        let calls = Arc::new(AtomicUsize::new(0));
        let worker = start(&live, clock.clone(), calls.clone()).await;
        let proposal = await_state(&mut owner, REQUEST, State::AwaitingSignature, &clock).await;
        assert_eq!(
            owner.get(AGENT, SECOND, NOW + 2).unwrap().state,
            State::ApprovedWaiting as i32
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        worker.close().await.unwrap();
        let worker = start(&live, clock.clone(), calls.clone()).await;
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(owner.get(AGENT, REQUEST, NOW + 2).unwrap(), proposal);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        clock.store(NOW + 123, Ordering::SeqCst);
        await_state(&mut owner, REQUEST, State::WaitingForAgent, &clock).await;
        let second = await_state(&mut owner, SECOND, State::AwaitingSignature, &clock).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        owner
            .cancel(AGENT, SECOND, second.generation, NOW + 123)
            .unwrap();
        worker.close().await.unwrap();
        assert!(owner.dispatch_request(&first, NOW + 123).unwrap().is_none());
        let mut connection = live.connection();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(format!("{}:access-dispatch:*", live.config.prefix()))
            .query(&mut connection)
            .unwrap();
        for key in keys {
            let _: i64 = redis::cmd("DEL").arg(key).query(&mut connection).unwrap();
        }
        let _: i64 = redis::cmd("DEL")
            .arg(format!("{}:proof-jobs:meta", live.config.prefix()))
            .query(&mut connection)
            .unwrap();
    }
}
