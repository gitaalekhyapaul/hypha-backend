use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bullmq::worker::CancellationToken;
use bullmq::{JobProgress, Queue};
use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AUTHORIZATION_PUBLIC_VALUES_VERSION,
    AuthorizationPublicValuesV1, CapabilityRequestBindingV1, capability_request_commitment,
    encode_public_values,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CreateCapabilityRequest, authorization_context_entry,
};
use serde_json::{Value, json};

use super::processor::{execute_with_limits, validate_public_job};
use super::{
    CapabilityExecutionRequest, CapabilityExecutor, ClaimedJobProcessor, ExecutionControl,
    ExecutionFuture, ExecutorFailureClass, PROOF_EXECUTION_DEADLINE, ProgressPublisher,
    ProofSupervisor, PublicFailureClass, SupervisorConfig, SupervisorConfigError,
    SupervisorFailureClass, SupervisorProgressPhase, WORKER_DRAIN_DELAY, WORKER_LOCK_DURATION,
    WORKER_LOCK_RENEW_TIME, WORKER_STALLED_INTERVAL,
};
use crate::admission::{
    AdmittedCapabilityRequest, LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
    admit_create_capability_request,
};
use crate::one_shot::{OneShotMetrics, OneShotSuccess};
use crate::private_envelope::{
    DeleteStatus, PrivateCapabilityEnvelope, PrivateEnvelopeError, PrivateEnvelopeRef,
    PrivateEnvelopeStore,
};
use crate::publication::{CAPABILITY_JOB_NAME, CapabilityJobPayload};
use crate::queue::CAPABILITY_QUEUE_NAME;
use crate::service::{ClockError, TrustedClock};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PRIVATE_CONTEXT_VALUE: &str = "finance-private-context";

#[test]
fn supervisor_configuration_is_explicit_bounded_and_redacted() {
    let config = SupervisorConfig::from_values(
        "redis://raziel:private-password@example.invalid:6379/0",
        "raziel-capabilities",
        "/var/lib/raziel/private-envelopes",
    )
    .unwrap();

    assert_eq!(config.queue_prefix(), "raziel-capabilities");
    assert_eq!(
        config.private_envelope_root(),
        Path::new("/var/lib/raziel/private-envelopes")
    );

    let worker = config.queue_config().worker_options();
    assert_eq!(worker.concurrency, 1);
    assert_eq!(
        worker.lock_duration,
        WORKER_LOCK_DURATION.as_millis() as u64
    );
    assert_eq!(
        worker.lock_renew_time,
        Some(WORKER_LOCK_RENEW_TIME.as_millis() as u64)
    );
    assert_eq!(
        worker.stalled_interval,
        WORKER_STALLED_INTERVAL.as_millis() as u64
    );
    assert_eq!(worker.drain_delay, WORKER_DRAIN_DELAY.as_secs());

    let debug = format!("{config:?}");
    assert!(debug.contains("raziel-capabilities"));
    assert!(!debug.contains("private-password"));
    assert!(!debug.contains("example.invalid"));
    assert!(!debug.contains("private-envelopes"));
    assert!(debug.matches("<redacted>").count() >= 2);
}

#[test]
fn supervisor_configuration_rejects_ambiguous_owner_inputs() {
    assert_eq!(
        SupervisorConfig::from_values(
            "http://redis:6379/0",
            "raziel-capabilities",
            "/var/lib/raziel/private-envelopes",
        )
        .unwrap_err(),
        SupervisorConfigError::InvalidRedisUrl
    );
    assert_eq!(
        SupervisorConfig::from_values(
            "redis://redis:6379/0",
            "raziel capabilities",
            "/var/lib/raziel/private-envelopes",
        )
        .unwrap_err(),
        SupervisorConfigError::InvalidQueuePrefix
    );
    assert_eq!(
        SupervisorConfig::from_values(
            "redis://redis:6379/0",
            "raziel-capabilities",
            "relative/private-envelopes",
        )
        .unwrap_err(),
        SupervisorConfigError::InvalidPrivateEnvelopeRoot
    );
}

#[test]
fn environment_loader_requires_all_supervisor_settings() {
    let values = HashMap::from([
        ("RAZIEL_REDIS_URL", "redis://redis:6379/0"),
        ("RAZIEL_QUEUE_PREFIX", "raziel-capabilities"),
        (
            "RAZIEL_PRIVATE_ENVELOPE_ROOT",
            "/var/lib/raziel/private-envelopes",
        ),
    ]);
    let config =
        SupervisorConfig::from_lookup(|key| values.get(key).map(ToString::to_string)).unwrap();
    assert_eq!(config.queue_prefix(), "raziel-capabilities");

    let missing_root = SupervisorConfig::from_lookup(|key| {
        (key != "RAZIEL_PRIVATE_ENVELOPE_ROOT")
            .then(|| values.get(key).map(ToString::to_string))
            .flatten()
    });
    assert_eq!(
        missing_root.unwrap_err(),
        SupervisorConfigError::InvalidPrivateEnvelopeRoot
    );
}

#[tokio::test]
async fn invalid_public_job_stops_before_progress_storage_and_executor() {
    let fixture = WorkerFixture::new("invalid-public-job", ExecutorBehavior::Complete);
    let malformed = json!({
        "format_version": 1,
        "private_envelope_ref": fixture.reference.queue_token(),
        "unexpected": PRIVATE_CONTEXT_VALUE,
    });
    let error = validate_public_job(CAPABILITY_JOB_NAME, &fixture.reference.job_id(), &malformed)
        .unwrap_err();

    assert_eq!(error.class(), SupervisorFailureClass::InvalidPublicPayload);
    assert_eq!(error.public_class(), PublicFailureClass::InvalidRequest);
    assert_eq!(fixture.store.reads(), 0);
    assert_eq!(fixture.store.deletes(), 0);
    assert_eq!(fixture.executor.invocations(), 0);
}

#[tokio::test]
async fn mismatched_job_identity_cannot_delete_another_envelope() {
    let fixture = WorkerFixture::new("identity-mismatch", ExecutorBehavior::Complete);
    let error =
        validate_public_job(CAPABILITY_JOB_NAME, &"0".repeat(64), &fixture.payload()).unwrap_err();

    assert_eq!(error.class(), SupervisorFailureClass::JobIdentityMismatch);
    assert_eq!(fixture.store.reads(), 0);
    assert_eq!(fixture.store.deletes(), 0);
    assert_eq!(fixture.executor.invocations(), 0);
}

#[tokio::test]
async fn valid_claim_uses_closed_progress_then_executes_and_deletes() {
    let fixture = WorkerFixture::new("successful-handoff", ExecutorBehavior::Complete);
    let mut progress = RecordingProgress::default();

    fixture
        .processor
        .process(fixture.reference, CancellationToken::new(), &mut progress)
        .await
        .unwrap();
    assert_eq!(progress.phases(), &[SupervisorProgressPhase::Executing]);
    assert_eq!(fixture.store.reads(), 1);
    assert_eq!(fixture.store.deletes(), 1);
    assert_eq!(fixture.executor.invocations(), 1);
    assert_eq!(fixture.executor.last_issued_at(), Some(TRUSTED_NOW));
    assert_eq!(
        fixture.executor.last_commitment(),
        Some(*fixture._admitted.capability_request_commitment())
    );
    assert_eq!(
        fixture.executor.last_deadline(),
        Some(PROOF_EXECUTION_DEADLINE)
    );
    assert!(!fixture.root.envelope_path(&fixture.reference).exists());

    let executor_debug = fixture.executor.last_request_debug().unwrap();
    assert!(executor_debug.contains("<redacted>"));
    assert!(!executor_debug.contains(PRIVATE_CONTEXT_VALUE));
    assert!(!executor_debug.contains(&fixture.reference.queue_token()));
}

#[tokio::test]
async fn progress_failure_stops_before_private_read_and_removes_the_envelope() {
    let fixture = WorkerFixture::new("progress-failure", ExecutorBehavior::Complete);
    let mut progress = RecordingProgress::failing_on(SupervisorProgressPhase::Executing);

    let error = fixture
        .processor
        .process(fixture.reference, CancellationToken::new(), &mut progress)
        .await
        .unwrap_err();

    assert_eq!(error.class(), SupervisorFailureClass::ProgressUnavailable);
    assert_eq!(error.public_class(), PublicFailureClass::ServiceUnavailable);
    assert_eq!(fixture.store.reads(), 0);
    assert_eq!(fixture.store.deletes(), 1);
    assert_eq!(fixture.executor.invocations(), 0);
}

#[tokio::test]
async fn executor_failure_stays_typed_privately_and_generic_publicly() {
    let fixture = WorkerFixture::new(
        "executor-failure",
        ExecutorBehavior::Fail(ExecutorFailureClass::MalformedResult),
    );
    let mut progress = RecordingProgress::default();

    let error = fixture
        .processor
        .process(fixture.reference, CancellationToken::new(), &mut progress)
        .await
        .unwrap_err();

    assert_eq!(
        error.class(),
        SupervisorFailureClass::Executor(ExecutorFailureClass::MalformedResult)
    );
    assert_eq!(error.public_class(), PublicFailureClass::ServiceUnavailable);
    assert_eq!(error.public_class().redis_reason(), "SERVICE_UNAVAILABLE");
    assert_eq!(fixture.store.deletes(), 1);

    let output = format!("{error:?} {error}");
    assert!(!output.contains(PRIVATE_CONTEXT_VALUE));
    assert!(!output.contains(&fixture.reference.queue_token()));
}

#[tokio::test]
async fn worker_start_rechecks_the_full_local_proof_window() {
    let fixture = WorkerFixture::new_at(
        "stale-at-worker-start",
        ExecutorBehavior::Complete,
        TRUSTED_NOW + 1,
    );
    let mut progress = RecordingProgress::default();

    let error = fixture
        .processor
        .process(fixture.reference, CancellationToken::new(), &mut progress)
        .await
        .unwrap_err();

    assert_eq!(
        error.class(),
        SupervisorFailureClass::InsufficientValidityWindow
    );
    assert_eq!(fixture.store.reads(), 1);
    assert_eq!(fixture.store.deletes(), 1);
    assert_eq!(fixture.executor.invocations(), 0);
}

#[tokio::test]
async fn execution_deadline_cancels_and_waits_for_executor_exit() {
    let fixture = WorkerFixture::new("deadline", ExecutorBehavior::AwaitCancellation);
    let envelope = fixture.store.inner.read(&fixture.reference).unwrap();
    let request = CapabilityExecutionRequest::new(envelope, TRUSTED_NOW);
    let cancellation = CancellationToken::new();

    let error = execute_with_limits(
        fixture.executor.as_ref(),
        request,
        cancellation.clone(),
        Duration::from_millis(1),
        Duration::from_secs(1),
    )
    .await
    .unwrap_err();

    assert_eq!(error, ExecutorFailureClass::TimedOut);
    assert!(cancellation.is_cancelled());
    assert_eq!(fixture.executor.invocations(), 1);
}

#[tokio::test]
async fn active_cancellation_reaches_the_executor_before_cleanup() {
    let fixture = WorkerFixture::new("active-cancellation", ExecutorBehavior::AwaitCancellation);
    let token = CancellationToken::new();
    let cancel = token.clone();
    let processor = Arc::clone(&fixture.processor);
    let reference = fixture.reference;

    let task = tokio::spawn(async move {
        let mut progress = RecordingProgress::default();
        let result = processor.process(reference, token, &mut progress).await;
        (result, progress)
    });

    fixture.executor.wait_until_started().await;
    cancel.cancel();
    let (result, progress) = task.await.unwrap();
    let error = result.unwrap_err();

    assert_eq!(error.class(), SupervisorFailureClass::Cancelled);
    assert_eq!(error.public_class(), PublicFailureClass::Canceled);
    assert_eq!(error.public_class().redis_reason(), "CANCELED");
    assert_eq!(progress.phases(), &[SupervisorProgressPhase::Executing]);
    assert_eq!(fixture.store.deletes(), 1);
    assert_eq!(fixture.executor.invocations(), 1);
}

struct WorkerFixture {
    _admitted: AdmittedCapabilityRequest,
    root: TestRoot,
    reference: PrivateEnvelopeRef,
    store: Arc<RecordingStore>,
    executor: Arc<FakeExecutor>,
    processor: Arc<ClaimedJobProcessor>,
}

impl WorkerFixture {
    fn new(name: &str, behavior: ExecutorBehavior) -> Self {
        Self::new_at(name, behavior, TRUSTED_NOW)
    }

    fn new_at(name: &str, behavior: ExecutorBehavior, trusted_now: i64) -> Self {
        let root = TestRoot::new(name);
        let inner = PrivateEnvelopeStore::open(root.path()).unwrap();
        let admitted = admitted_request(name.as_bytes()[0]);
        let reference = inner.stage(&admitted).unwrap();
        let store = Arc::new(RecordingStore::new(inner));
        let executor = Arc::new(FakeExecutor::new(behavior));
        let processor = Arc::new(ClaimedJobProcessor::new(
            Arc::clone(&store) as Arc<dyn super::EnvelopeAccess>,
            Arc::clone(&executor) as Arc<dyn CapabilityExecutor>,
            Arc::new(FixedClock(trusted_now)),
        ));
        Self {
            _admitted: admitted,
            root,
            reference,
            store,
            executor,
            processor,
        }
    }

    fn payload(&self) -> Value {
        json!({
            "format_version": 1,
            "private_envelope_ref": self.reference.queue_token(),
        })
    }
}

struct RecordingStore {
    inner: PrivateEnvelopeStore,
    reads: AtomicUsize,
    deletes: AtomicUsize,
}

impl RecordingStore {
    fn new(inner: PrivateEnvelopeStore) -> Self {
        Self {
            inner,
            reads: AtomicUsize::new(0),
            deletes: AtomicUsize::new(0),
        }
    }

    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    fn deletes(&self) -> usize {
        self.deletes.load(Ordering::SeqCst)
    }
}

impl super::EnvelopeAccess for RecordingStore {
    fn read(
        &self,
        reference: &PrivateEnvelopeRef,
    ) -> Result<PrivateCapabilityEnvelope, PrivateEnvelopeError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read(reference)
    }

    fn delete(&self, reference: &PrivateEnvelopeRef) -> Result<DeleteStatus, PrivateEnvelopeError> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        self.inner.delete(reference)
    }
}

#[derive(Clone, Copy)]
enum ExecutorBehavior {
    Complete,
    Fail(ExecutorFailureClass),
    AwaitCancellation,
}

struct FakeExecutor {
    behavior: ExecutorBehavior,
    invocations: AtomicUsize,
    started: tokio::sync::Notify,
    last_request_debug: Mutex<Option<String>>,
    last_commitment: Mutex<Option<[u8; 32]>>,
    last_issued_at: AtomicI64,
    last_deadline: Mutex<Option<Duration>>,
}

impl FakeExecutor {
    fn new(behavior: ExecutorBehavior) -> Self {
        Self {
            behavior,
            invocations: AtomicUsize::new(0),
            started: tokio::sync::Notify::new(),
            last_request_debug: Mutex::new(None),
            last_commitment: Mutex::new(None),
            last_issued_at: AtomicI64::new(i64::MIN),
            last_deadline: Mutex::new(None),
        }
    }

    fn invocations(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }

    fn last_request_debug(&self) -> Option<String> {
        self.last_request_debug.lock().unwrap().clone()
    }

    fn last_commitment(&self) -> Option<[u8; 32]> {
        *self.last_commitment.lock().unwrap()
    }

    fn last_issued_at(&self) -> Option<i64> {
        match self.last_issued_at.load(Ordering::SeqCst) {
            i64::MIN => None,
            value => Some(value),
        }
    }

    fn last_deadline(&self) -> Option<Duration> {
        *self.last_deadline.lock().unwrap()
    }

    async fn wait_until_started(&self) {
        let notified = self.started.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.invocations() > 0 {
            return;
        }
        notified.await;
    }
}

impl CapabilityExecutor for FakeExecutor {
    fn execute(
        &self,
        request: CapabilityExecutionRequest,
        control: ExecutionControl,
    ) -> ExecutionFuture {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        *self.last_request_debug.lock().unwrap() = Some(format!("{request:?}"));
        *self.last_commitment.lock().unwrap() =
            Some(*request.envelope().capability_request_commitment());
        self.last_issued_at
            .store(request.issued_at_unix_seconds(), Ordering::SeqCst);
        *self.last_deadline.lock().unwrap() = Some(control.deadline());
        self.started.notify_waiters();
        let behavior = self.behavior;
        Box::pin(async move {
            match behavior {
                ExecutorBehavior::Complete => Ok(fake_success()),
                ExecutorBehavior::Fail(error) => Err(error),
                ExecutorBehavior::AwaitCancellation => {
                    control.cancelled().await;
                    Err(ExecutorFailureClass::Cancelled)
                }
            }
        })
    }
}

fn fake_success() -> OneShotSuccess {
    let artifact = b"synthetic-proof".to_vec();
    let public_values = encode_public_values(&AuthorizationPublicValuesV1 {
        format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
        policy_hash: [0x11; 32],
        relationship_snapshot_root: [0x22; 32],
        capability_request_commitment: [0x33; 32],
        capability_scope_hash: [0x44; 32],
        issued_at_unix_seconds: TRUSTED_NOW,
        expires_at_unix_seconds: TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
        issuance_nullifier: [0x55; 32],
    })
    .unwrap();
    OneShotSuccess::new(
        artifact.clone(),
        public_values,
        format!("sp1-vk-v1:{}", "ab".repeat(32)),
        OneShotMetrics {
            execute_millis: 1,
            prove_millis: 1,
            artifact_bytes: artifact.len() as u64,
            peak_memory_bytes: 1,
            peak_swap_bytes: 0,
        },
    )
    .unwrap()
}

#[derive(Clone, Copy)]
struct FixedClock(i64);

impl TrustedClock for FixedClock {
    fn now_unix_seconds(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

#[derive(Default)]
struct RecordingProgress {
    phases: Vec<SupervisorProgressPhase>,
    fail_on: Option<SupervisorProgressPhase>,
}

impl RecordingProgress {
    fn failing_on(phase: SupervisorProgressPhase) -> Self {
        Self {
            phases: Vec::new(),
            fail_on: Some(phase),
        }
    }

    fn phases(&self) -> &[SupervisorProgressPhase] {
        &self.phases
    }
}

impl ProgressPublisher for RecordingProgress {
    fn publish<'a>(
        &'a mut self,
        phase: SupervisorProgressPhase,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
        Box::pin(async move {
            self.phases.push(phase);
            if self.fail_on == Some(phase) {
                return Err(());
            }
            Ok(())
        })
    }
}

fn admitted_request(nonce_byte: u8) -> AdmittedCapabilityRequest {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let transport = signed_request(valid_request(&signing_key, nonce_byte), &signing_key);
    admit_create_capability_request(transport, TRUSTED_NOW).unwrap()
}

fn valid_request(signing_key: &SigningKey, nonce_byte: u8) -> CreateCapabilityRequest {
    CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: "agent-display-only".into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: "task-2026-08-22".into(),
        purpose: "reconcile merchant settlement records".into(),
        action: "reconcile".into(),
        resource_type: "payment_batch".into(),
        resource_scope: "batch:synthetic-001".into(),
        requested_expires_at: Some(Timestamp {
            seconds: TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
            nanos: 0,
        }),
        issuance_nonce: vec![nonce_byte; 32],
        snapshot_id: "snapshot-synthetic-001".into(),
        request_signature: Vec::new(),
        request_context: vec![AuthorizationContextEntry {
            name: "department".into(),
            value: Some(authorization_context_entry::Value::StringValue(
                PRIVATE_CONTEXT_VALUE.into(),
            )),
        }],
    }
}

fn signed_request(
    mut request: CreateCapabilityRequest,
    signing_key: &SigningKey,
) -> CreateCapabilityRequest {
    let commitment = capability_request_commitment(&expected_binding(&request)).unwrap();
    request.request_signature = signing_key.sign(&commitment).to_bytes().to_vec();
    request
}

fn expected_binding(request: &CreateCapabilityRequest) -> CapabilityRequestBindingV1 {
    let public_key: [u8; 32] = request.agent_public_key.as_slice().try_into().unwrap();
    CapabilityRequestBindingV1 {
        format_version: request.format_version,
        agent_key_thumbprint: derive_ed25519_key_thumbprint(&public_key),
        task_id: request.task_id.clone(),
        purpose: request.purpose.clone(),
        action: request.action.clone(),
        resource_type: request.resource_type.clone(),
        resource_scope: request.resource_scope.clone(),
        requested_expires_at_unix_seconds: request.requested_expires_at.unwrap().seconds,
        issuance_nonce: request.issuance_nonce.as_slice().try_into().unwrap(),
        snapshot_id: request.snapshot_id.clone(),
        request_context: request
            .request_context
            .iter()
            .map(|entry| ContextEntry {
                name: entry.name.clone(),
                value: match entry.value.as_ref().unwrap() {
                    authorization_context_entry::Value::BoolValue(value) => {
                        CaveatValue::Bool(*value)
                    }
                    authorization_context_entry::Value::IntValue(value) => CaveatValue::Int(*value),
                    authorization_context_entry::Value::StringValue(value) => {
                        CaveatValue::String(value.clone())
                    }
                },
            })
            .collect(),
    }
}

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(name: &str) -> Self {
        let sequence = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "razield-worker-{name}-{}-{sequence}",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn envelope_path(&self, reference: &PrivateEnvelopeRef) -> PathBuf {
        self.path.join(format!("{}.pb", reference.queue_token()))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn worker_timing_constants_are_sane() {
    assert!(WORKER_LOCK_RENEW_TIME < WORKER_LOCK_DURATION);
    assert!(WORKER_STALLED_INTERVAL >= WORKER_LOCK_DURATION);
    assert!(WORKER_DRAIN_DELAY >= Duration::from_secs(1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the digest-pinned Redis conformance service"]
async fn live_worker_claims_one_job_and_fails_closed_without_a_capability()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_url = std::env::var("RAZIEL_QUEUE_TEST_REDIS_URL")?;
    let root = TestRoot::new("live-claim");
    let prefix = format!(
        "raziel-worker-live-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let config = SupervisorConfig::from_values(
        &redis_url,
        &prefix,
        root.path().to_str().ok_or("test root is not UTF-8")?,
    )?;
    let queue =
        Queue::with_options(CAPABILITY_QUEUE_NAME, config.queue_config().queue_options()).await?;
    queue.obliterate(true, 1_000).await?;

    let store = PrivateEnvelopeStore::open(root.path())?;
    let admitted = admitted_request(0xd4);
    let reference = store.stage(&admitted)?;
    let payload = CapabilityJobPayload::new(reference)
        .to_json_value()
        .unwrap();
    let job_id = reference.job_id();
    queue
        .add(CAPABILITY_JOB_NAME, payload.clone())
        .options(config.queue_config().job_options())
        .job_id(&job_id)
        .await?;

    let executor = Arc::new(FakeExecutor::new(ExecutorBehavior::Complete));
    let supervisor = ProofSupervisor::start_with_clock(
        config,
        Arc::clone(&executor) as Arc<dyn CapabilityExecutor>,
        Arc::new(FixedClock(TRUSTED_NOW)),
        super::ProofSupervisorActivation::library_plumbing(),
    )
    .await?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let counts = queue.get_job_counts().await?;
        if counts.failed == 1 && counts.active == 0 && counts.waiting == 0 {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("timed out waiting for the live worker claim".into());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let retained = queue
        .get_job(&job_id)
        .await?
        .ok_or("failed-closed live worker job is missing")?;
    assert_eq!(executor.invocations(), 1);
    assert_eq!(executor.last_issued_at(), Some(TRUSTED_NOW));
    assert_eq!(retained.failed_reason(), "SERVICE_UNAVAILABLE");
    assert!(retained.returnvalue().is_empty());
    match retained.progress() {
        JobProgress::Object(value) => {
            assert_eq!(value, &SupervisorProgressPhase::Executing.to_public_value())
        }
        JobProgress::Number(_) => return Err("worker progress used an open numeric value".into()),
    }
    assert!(!root.envelope_path(&reference).exists());
    assert!(!serde_json::to_string(retained.data())?.contains(PRIVATE_CONTEXT_VALUE));

    supervisor.close().await?;
    queue.remove_global_concurrency().await?;
    queue.obliterate(true, 1_000).await?;
    queue.close().await;
    println!("bullmq_supervisor_plumbing=pass");
    Ok(())
}
