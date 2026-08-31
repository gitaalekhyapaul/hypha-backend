use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bullmq::JobProgress;
use bullmq::types::JobState;
use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, capability_request_commitment,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CreateCapabilityRequest, authorization_context_entry,
};

use super::bullmq::{
    CapabilityQueue, QueueAddError, QueueAddOutcome, QueueReadError, RetainedQueueJob,
};
use super::coordinator::publish_transaction;
use super::payload::{CapabilityJobPayload, PayloadError};
use super::reconciliation::{StartupReconciliationError, reconcile_startup_with_queue};
use super::{
    CAPABILITY_JOB_NAME, CapabilityPublicationError, PublicCapabilityJobState,
    PublicationDisposition,
};
use crate::admission::{
    AdmittedCapabilityRequest, LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
    admit_create_capability_request,
};
use crate::private_envelope::{
    DeleteStatus, PrivateEnvelopeError, PrivateEnvelopeRef, PrivateEnvelopeStore,
};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PRIVATE_CONTEXT_VALUE: &str = "private-merchant-4829";
const DISPLAY_AGENT_ID: &str = "reconciliation-agent";

static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn payload_has_two_strict_fields_and_redacts_its_reference() {
    let reference =
        PrivateEnvelopeRef::from_queue_token(&format!("pe1_{}", "ab".repeat(32))).unwrap();
    let payload = CapabilityJobPayload::new(reference);
    let value = payload.to_json_value().unwrap();
    let object = value.as_object().unwrap();

    assert_eq!(object.len(), 2);
    assert_eq!(object["format_version"], 1);
    assert_eq!(object["private_envelope_ref"], reference.queue_token());
    assert_eq!(
        CapabilityJobPayload::from_json_value(&value).unwrap(),
        payload
    );

    let mut unknown = value.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("task_id".into(), "private-task".into());
    assert_eq!(
        CapabilityJobPayload::from_json_value(&unknown),
        Err(PayloadError::Malformed)
    );

    let mut wrong_version = value.clone();
    wrong_version["format_version"] = 2.into();
    assert_eq!(
        CapabilityJobPayload::from_json_value(&wrong_version),
        Err(PayloadError::UnsupportedVersion)
    );

    let mut path = value;
    path["private_envelope_ref"] = "../private.pb".into();
    assert_eq!(
        CapabilityJobPayload::from_json_value(&path),
        Err(PayloadError::InvalidReference)
    );

    let debug = format!("{payload:?}");
    assert!(!debug.contains(&reference.queue_token()));
    assert!(debug.contains("<redacted>"));
}

#[tokio::test]
async fn startup_reconciliation_keeps_live_and_recent_envelopes_only() {
    let root = TestRoot::new("startup-reconcile");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let live = store
        .stage(&admitted_request(0xc0, DISPLAY_AGENT_ID))
        .unwrap();
    let stale_missing = store
        .stage(&admitted_request(0xc1, DISPLAY_AGENT_ID))
        .unwrap();
    let recent_missing = store
        .stage(&admitted_request(0xc2, DISPLAY_AGENT_ID))
        .unwrap();
    let terminal = store
        .stage(&admitted_request(0xc3, DISPLAY_AGENT_ID))
        .unwrap();
    let now = SystemTime::now();
    let old = now - Duration::from_secs(20 * 60);
    for reference in [live, stale_missing, terminal] {
        set_modified(&envelope_path(root.path(), reference), old);
    }

    let queue = ReconciliationQueue::default();
    queue.insert(live, Ok(Some(retained_job(live, JobState::Waiting))));
    queue.insert(
        terminal,
        Ok(Some(retained_job(terminal, JobState::Completed))),
    );

    let summary =
        reconcile_startup_with_queue(store.clone(), &queue, now, Duration::from_secs(10 * 60))
            .await
            .unwrap();

    assert_eq!(summary.references_scanned, 4);
    assert_eq!(summary.live_references, 1);
    assert_eq!(summary.cleanup.envelopes_deleted, 2);
    assert_eq!(summary.cleanup.temporary_files_deleted, 0);
    store.read(&live).unwrap();
    store.read(&recent_missing).unwrap();
    assert_eq!(
        store.read(&stale_missing),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
    assert_eq!(
        store.read(&terminal),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
}

#[tokio::test]
async fn startup_reconciliation_preserves_files_on_queue_failure_or_mismatch() {
    let unavailable_root = TestRoot::new("startup-unavailable");
    let unavailable_store = PrivateEnvelopeStore::open(unavailable_root.path()).unwrap();
    let unavailable = unavailable_store
        .stage(&admitted_request(0xc4, DISPLAY_AGENT_ID))
        .unwrap();
    set_modified(
        &envelope_path(unavailable_root.path(), unavailable),
        SystemTime::now() - Duration::from_secs(20 * 60),
    );
    let unavailable_queue = ReconciliationQueue::default();
    unavailable_queue.insert(unavailable, Err(QueueReadError::Unavailable));
    assert_eq!(
        reconcile_startup_with_queue(
            unavailable_store.clone(),
            &unavailable_queue,
            SystemTime::now(),
            Duration::from_secs(10 * 60),
        )
        .await,
        Err(StartupReconciliationError::QueueUnavailable)
    );
    unavailable_store.read(&unavailable).unwrap();

    let mismatch_root = TestRoot::new("startup-mismatch");
    let mismatch_store = PrivateEnvelopeStore::open(mismatch_root.path()).unwrap();
    let mismatch = mismatch_store
        .stage(&admitted_request(0xc5, DISPLAY_AGENT_ID))
        .unwrap();
    let wrong_reference =
        PrivateEnvelopeRef::from_queue_token(&format!("pe1_{}", "cd".repeat(32))).unwrap();
    let mismatch_queue = ReconciliationQueue::default();
    mismatch_queue.insert(
        mismatch,
        Ok(Some(retained_job(wrong_reference, JobState::Waiting))),
    );
    assert_eq!(
        reconcile_startup_with_queue(
            mismatch_store.clone(),
            &mismatch_queue,
            SystemTime::now(),
            Duration::from_secs(10 * 60),
        )
        .await,
        Err(StartupReconciliationError::QueueRecordMismatch)
    );
    mismatch_store.read(&mismatch).unwrap();
}

#[tokio::test]
async fn stages_before_add_and_returns_the_authoritative_duplicate() {
    let root = TestRoot::new("ordering");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb1, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());

    let created = publish_transaction(&store, &queue, &admitted)
        .await
        .unwrap();
    assert_eq!(created.job_id(), admitted.job_id());
    assert_eq!(created.disposition(), PublicationDisposition::Created);
    assert_eq!(created.state(), PublicCapabilityJobState::Queued);
    assert_eq!(created.submitted_at_unix_millis(), 1_800_000_000_000);
    assert_eq!(created.updated_at_unix_millis(), 1_800_000_000_000);
    assert!(queue.add_saw_durable_envelope());
    assert_eq!(queue.add_calls(), 1);

    let renamed = admitted_request(0xb1, "renamed-display-label");
    assert_eq!(renamed.job_id(), admitted.job_id());
    let retained = publish_transaction(&store, &queue, &renamed).await.unwrap();
    assert_eq!(retained.job_id(), admitted.job_id());
    assert_eq!(retained.disposition(), PublicationDisposition::Retained);
    assert_eq!(queue.add_calls(), 1);
    store.read(&reference_for(&admitted)).unwrap();

    let output = format!("{created:?} {retained:?}");
    assert!(!output.contains(admitted.job_id()));
    assert!(!output.contains("pe1_"));
    assert!(!output.contains(PRIVATE_CONTEXT_VALUE));
    assert!(!output.contains(DISPLAY_AGENT_ID));
}

#[tokio::test]
async fn terminal_record_fails_closed_until_a_bound_result_reader_exists() {
    let root = TestRoot::new("terminal-result");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xba, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.set_record(retained_job(reference_for(&admitted), JobState::Completed));

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::QueueTerminalResultUnavailable)
    );
    assert_eq!(queue.add_calls(), 0);
    store.read(&reference_for(&admitted)).unwrap();
}

#[tokio::test]
async fn confirmed_publication_rejection_removes_a_new_envelope() {
    let root = TestRoot::new("confirmed-rejection");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb2, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.set_add_error(Some(QueueAddError::Rejected));

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::QueuePublicationRejected)
    );
    assert_eq!(
        store.read(&reference_for(&admitted)),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
}

#[tokio::test]
async fn occupied_capacity_removes_the_losing_private_envelope() {
    let root = TestRoot::new("capacity-occupied");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xbd, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.set_add_outcome(QueueAddOutcome::CapacityOccupied);

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::QueueCapacityOccupied)
    );
    assert_eq!(queue.add_calls(), 1);
    assert_eq!(
        store.read(&reference_for(&admitted)),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
}

#[tokio::test]
async fn failed_cleanup_is_reported_instead_of_the_queue_error() {
    let root = TestRoot::new("cleanup-failure");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb3, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.set_add_error(Some(QueueAddError::Rejected));
    queue.break_root_on_add(root.path().to_path_buf());

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::EnvelopeCleanupFailed)
    );

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        store.delete(&reference_for(&admitted)).unwrap(),
        DeleteStatus::Deleted
    );
}

#[tokio::test]
async fn uncertain_publication_keeps_the_envelope_for_reconciliation() {
    let root = TestRoot::new("uncertain");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb4, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.push_read(Ok(None));
    queue.push_read(Err(QueueReadError::Unavailable));
    queue.set_add_error(Some(QueueAddError::Uncertain));

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::QueuePublicationUncertain)
    );
    store.read(&reference_for(&admitted)).unwrap();
}

#[tokio::test]
async fn prepublication_queue_failure_removes_a_new_envelope() {
    let root = TestRoot::new("unavailable-before-add");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb9, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.push_read(Err(QueueReadError::Unavailable));

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::QueueUnavailable)
    );
    assert_eq!(queue.add_calls(), 0);
    assert_eq!(
        store.read(&reference_for(&admitted)),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
}

#[tokio::test]
async fn uncertain_add_returns_a_record_that_redis_retained() {
    let root = TestRoot::new("uncertain-retained");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb5, DISPLAY_AGENT_ID);
    let queue = FakeQueue::new(store.clone());
    queue.set_add_error(Some(QueueAddError::Uncertain));
    queue.set_persist_on_uncertain(true);

    let result = publish_transaction(&store, &queue, &admitted)
        .await
        .unwrap();
    assert_eq!(result.disposition(), PublicationDisposition::Retained);
    assert_eq!(queue.add_calls(), 1);
}

#[tokio::test]
async fn missing_records_clean_up_but_mismatches_preserve_private_input() {
    let missing_root = TestRoot::new("missing-after-add");
    let missing_store = PrivateEnvelopeStore::open(missing_root.path()).unwrap();
    let missing_request = admitted_request(0xb6, DISPLAY_AGENT_ID);
    let missing_queue = FakeQueue::new(missing_store.clone());
    missing_queue.set_persist_on_success(false);
    assert_eq!(
        publish_transaction(&missing_store, &missing_queue, &missing_request).await,
        Err(CapabilityPublicationError::QueueRecordMissing)
    );
    assert_eq!(
        missing_store.read(&reference_for(&missing_request)),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );

    let mismatch_root = TestRoot::new("mismatched-record");
    let mismatch_store = PrivateEnvelopeStore::open(mismatch_root.path()).unwrap();
    let mismatch_request = admitted_request(0xb7, DISPLAY_AGENT_ID);
    let mismatch_queue = FakeQueue::new(mismatch_store.clone());
    let wrong_reference =
        PrivateEnvelopeRef::from_queue_token(&format!("pe1_{}", "cd".repeat(32))).unwrap();
    let mut mismatched_record = retained_job(reference_for(&mismatch_request), JobState::Waiting);
    mismatched_record.payload = Ok(CapabilityJobPayload::new(wrong_reference));
    mismatch_queue.set_record(mismatched_record);
    assert_eq!(
        publish_transaction(&mismatch_store, &mismatch_queue, &mismatch_request).await,
        Err(CapabilityPublicationError::QueueRecordMismatch)
    );
    assert_eq!(mismatch_queue.add_calls(), 0);
    mismatch_store
        .read(&reference_for(&mismatch_request))
        .unwrap();
}

#[tokio::test]
async fn an_existing_envelope_without_a_job_never_publishes_or_deletes() {
    let root = TestRoot::new("existing-envelope");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb8, DISPLAY_AGENT_ID);
    let reference = store.stage(&admitted).unwrap();
    let queue = FakeQueue::new(store.clone());

    assert_eq!(
        publish_transaction(&store, &queue, &admitted).await,
        Err(CapabilityPublicationError::QueuePublicationUncertain)
    );
    assert_eq!(queue.add_calls(), 0);
    store.read(&reference).unwrap();
}

#[derive(Default)]
struct ReconciliationQueue {
    records: StdMutex<HashMap<String, Result<Option<RetainedQueueJob>, QueueReadError>>>,
}

impl ReconciliationQueue {
    fn insert(
        &self,
        reference: PrivateEnvelopeRef,
        record: Result<Option<RetainedQueueJob>, QueueReadError>,
    ) {
        self.records
            .lock()
            .unwrap()
            .insert(job_id_for(reference), record);
    }
}

impl CapabilityQueue for ReconciliationQueue {
    async fn get_job(&self, job_id: &str) -> Result<Option<RetainedQueueJob>, QueueReadError> {
        self.records
            .lock()
            .unwrap()
            .get(job_id)
            .cloned()
            .unwrap_or(Ok(None))
    }

    async fn add_job(
        &self,
        _job_id: &str,
        _payload: &CapabilityJobPayload,
    ) -> Result<QueueAddOutcome, QueueAddError> {
        panic!("startup reconciliation must not publish jobs")
    }
}

struct FakeQueue {
    store: PrivateEnvelopeStore,
    state: StdMutex<FakeQueueState>,
}

struct FakeQueueState {
    record: Option<RetainedQueueJob>,
    reads: VecDeque<Result<Option<RetainedQueueJob>, QueueReadError>>,
    add_error: Option<QueueAddError>,
    add_outcome: QueueAddOutcome,
    persist_on_success: bool,
    persist_on_uncertain: bool,
    add_saw_durable_envelope: bool,
    add_calls: usize,
    break_root_on_add: Option<PathBuf>,
}

impl FakeQueue {
    fn new(store: PrivateEnvelopeStore) -> Self {
        Self {
            store,
            state: StdMutex::new(FakeQueueState {
                record: None,
                reads: VecDeque::new(),
                add_error: None,
                add_outcome: QueueAddOutcome::Added,
                persist_on_success: true,
                persist_on_uncertain: false,
                add_saw_durable_envelope: false,
                add_calls: 0,
                break_root_on_add: None,
            }),
        }
    }

    fn push_read(&self, read: Result<Option<RetainedQueueJob>, QueueReadError>) {
        self.state.lock().unwrap().reads.push_back(read);
    }

    fn set_add_error(&self, error: Option<QueueAddError>) {
        self.state.lock().unwrap().add_error = error;
    }

    fn set_add_outcome(&self, outcome: QueueAddOutcome) {
        self.state.lock().unwrap().add_outcome = outcome;
    }

    fn set_persist_on_success(&self, persist: bool) {
        self.state.lock().unwrap().persist_on_success = persist;
    }

    fn set_persist_on_uncertain(&self, persist: bool) {
        self.state.lock().unwrap().persist_on_uncertain = persist;
    }

    fn set_record(&self, record: RetainedQueueJob) {
        self.state.lock().unwrap().record = Some(record);
    }

    fn break_root_on_add(&self, root: PathBuf) {
        self.state.lock().unwrap().break_root_on_add = Some(root);
    }

    fn add_saw_durable_envelope(&self) -> bool {
        self.state.lock().unwrap().add_saw_durable_envelope
    }

    fn add_calls(&self) -> usize {
        self.state.lock().unwrap().add_calls
    }
}

impl CapabilityQueue for FakeQueue {
    async fn get_job(&self, _job_id: &str) -> Result<Option<RetainedQueueJob>, QueueReadError> {
        let mut state = self.state.lock().unwrap();
        state
            .reads
            .pop_front()
            .unwrap_or_else(|| Ok(state.record.clone()))
    }

    async fn add_job(
        &self,
        job_id: &str,
        payload: &CapabilityJobPayload,
    ) -> Result<QueueAddOutcome, QueueAddError> {
        let durable = self.store.read(&payload.private_envelope_ref()).is_ok();
        let mut state = self.state.lock().unwrap();
        state.add_saw_durable_envelope = durable;
        state.add_calls += 1;

        let mut record = retained_job(payload.private_envelope_ref(), JobState::Waiting);
        record.id = job_id.into();
        match (state.add_error, state.add_outcome) {
            (None, QueueAddOutcome::Added) if state.persist_on_success => {
                state.record = Some(record);
            }
            (Some(QueueAddError::Uncertain), _) if state.persist_on_uncertain => {
                state.record = Some(record);
            }
            _ => {}
        }

        if let Some(root) = state.break_root_on_add.take() {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        state.add_error.map_or(Ok(state.add_outcome), Err)
    }
}

fn admitted_request(nonce_byte: u8, agent_id: &str) -> AdmittedCapabilityRequest {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let request = valid_request(&signing_key, nonce_byte, agent_id);
    let request = signed_request(request, &signing_key);
    admit_create_capability_request(request, TRUSTED_NOW).unwrap()
}

fn valid_request(
    signing_key: &SigningKey,
    nonce_byte: u8,
    agent_id: &str,
) -> CreateCapabilityRequest {
    CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: agent_id.into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: "task-2026-08-21".into(),
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

fn reference_for(admitted: &AdmittedCapabilityRequest) -> PrivateEnvelopeRef {
    PrivateEnvelopeRef::from_queue_token(&format!("pe1_{}", admitted.job_id())).unwrap()
}

fn retained_job(reference: PrivateEnvelopeRef, state: JobState) -> RetainedQueueJob {
    RetainedQueueJob {
        id: job_id_for(reference),
        name: CAPABILITY_JOB_NAME.into(),
        payload: Ok(CapabilityJobPayload::new(reference)),
        policy_matches: true,
        state,
        timestamp_ms: 1_800_000_000_000,
        processed_on_ms: None,
        finished_on_ms: None,
        progress: JobProgress::Number(0.0),
        failed_reason: String::new(),
        return_value: String::new(),
    }
}

fn job_id_for(reference: PrivateEnvelopeRef) -> String {
    reference
        .queue_token()
        .strip_prefix("pe1_")
        .unwrap()
        .to_owned()
}

fn envelope_path(root: &Path, reference: PrivateEnvelopeRef) -> PathBuf {
    root.join(format!("{}.pb", reference.queue_token()))
}

fn set_modified(path: &Path, time: SystemTime) {
    let file = OpenOptions::new().write(true).open(path).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(time))
        .unwrap();
}

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> Self {
        let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "razield-publication-{label}-{}-{sequence}-{timestamp}",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.path).is_ok() {
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o700));
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
