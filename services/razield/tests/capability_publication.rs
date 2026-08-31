use std::error::Error;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use bullmq::Queue;
use bullmq::options::DeduplicationOptions;
use bullmq::types::{JobState, KeepJobs, RemoveOnFinish};
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
use razield::admission::{LOCAL_PLONK_ADMISSION_WINDOW_SECONDS, admit_create_capability_request};
use razield::private_envelope::{PrivateEnvelopeError, PrivateEnvelopeRef, PrivateEnvelopeStore};
use razield::publication::{
    CAPABILITY_JOB_NAME, CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION, CapabilityJobPublisher,
    CapabilityPublicationError, PublicationDisposition,
};
use razield::queue::{
    CAPABILITY_QUEUE_NAME, GLOBAL_CONCURRENCY, JOB_ATTEMPTS, MAX_PUBLIC_JOB_PAYLOAD_BYTES,
    PROOF_CAPACITY_DEDUPLICATION_ID, QueueConfig, TERMINAL_RETENTION_AGE_SECONDS,
    TERMINAL_RETENTION_COUNT, TERMINAL_RETENTION_EVICTION_LIMIT,
};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PRIVATE_CONTEXT_VALUE: &str = "private-merchant-4829";
const DISPLAY_AGENT_ID: &str = "reconciliation-agent";
const OBLITERATE_BATCH_SIZE: usize = 1_000;

static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the digest-pinned Redis publication service"]
async fn publishes_minimal_waiting_job_and_returns_exact_duplicate() -> TestResult {
    let config = test_config("live-publication", "RAZIEL_QUEUE_TEST_REDIS_URL")?;
    let queue = open_clean_queue(&config).await?;
    let root = TestRoot::new("live-publication");
    let store = PrivateEnvelopeStore::open(root.path())?;
    let publisher = CapabilityJobPublisher::connect(store.clone(), &config).await?;
    let signing_key = signing_key();
    let transport = signed_request(valid_request(&signing_key), &signing_key);
    let mut distinct_transport = transport.clone();
    distinct_transport.issuance_nonce = vec![0xc2; 32];
    distinct_transport.request_signature.clear();
    let distinct = admit_create_capability_request(
        signed_request(distinct_transport, &signing_key),
        TRUSTED_NOW,
    )?;
    let mut renamed_transport = transport.clone();
    renamed_transport.agent_id = "renamed-display-label".into();
    let admitted = admit_create_capability_request(transport, TRUSTED_NOW)?;
    let renamed = admit_create_capability_request(renamed_transport, TRUSTED_NOW)?;

    let created = publisher.publish(&admitted).await?;
    assert_eq!(created.job_id(), admitted.job_id());
    assert_eq!(created.disposition(), PublicationDisposition::Created);
    assert_eq!(
        queue.get_global_concurrency().await?,
        Some(GLOBAL_CONCURRENCY)
    );

    let job = queue
        .get_job(admitted.job_id())
        .await?
        .ok_or_else(|| io::Error::other("published job is missing"))?;
    assert_eq!(
        queue.get_job_state(admitted.job_id()).await?,
        JobState::Waiting
    );
    assert_eq!(job.id(), admitted.job_id());
    assert_eq!(job.name(), CAPABILITY_JOB_NAME);
    assert_eq!(job.attempts_started(), 0);
    assert_eq!(job.attempts_made(), 0);
    assert!(job.processed_on().is_none());
    assert_job_policy(&job);

    let object = job
        .data()
        .as_object()
        .ok_or_else(|| io::Error::other("queue payload is not a JSON object"))?;
    assert_eq!(object.len(), 2);
    assert_eq!(
        object
            .get("format_version")
            .and_then(|value| value.as_u64()),
        Some(u64::from(CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION))
    );
    let token = object
        .get("private_envelope_ref")
        .and_then(|value| value.as_str())
        .ok_or_else(|| io::Error::other("queue payload has no envelope reference"))?;
    let reference = PrivateEnvelopeRef::from_queue_token(token)?;
    store.read(&reference)?;

    let serialized_payload = serde_json::to_string(job.data())?;
    assert!(serialized_payload.len() <= MAX_PUBLIC_JOB_PAYLOAD_BYTES);
    for private_value in [
        PRIVATE_CONTEXT_VALUE,
        DISPLAY_AGENT_ID,
        admitted.request().task_id.as_str(),
        root.path().to_str().expect("UTF-8 test path"),
        "agent_public_key",
        "request_signature",
        "issuance_nonce",
        "request_context",
    ] {
        assert!(!serialized_payload.contains(private_value));
    }

    let retained = publisher.publish(&renamed).await?;
    assert_eq!(retained.job_id(), admitted.job_id());
    assert_eq!(retained.disposition(), PublicationDisposition::Retained);
    let counts = queue.get_job_counts().await?;
    assert_eq!(counts.waiting, 1);
    assert_eq!(counts.active, 0);
    assert_eq!(counts.completed, 0);
    assert_eq!(counts.failed, 0);
    assert_eq!(std::fs::read_dir(root.path())?.count(), 1);

    assert_eq!(
        publisher.publish(&distinct).await,
        Err(CapabilityPublicationError::QueueCapacityOccupied)
    );
    assert_eq!(
        store.read(&reference_for(&distinct)?),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
    assert_eq!(queue.get_job_counts().await?.waiting, 1);

    let debug = format!("{created:?} {retained:?} {publisher:?}");
    assert!(!debug.contains(admitted.job_id()));
    assert!(!debug.contains(token));
    assert!(!debug.contains(PRIVATE_CONTEXT_VALUE));
    assert!(!debug.contains(root.path().to_str().expect("UTF-8 test path")));

    publisher.close().await;
    clean_queue(&queue).await?;
    println!("capability_publication_live=pass");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a digest-pinned Redis user that rejects script execution"]
async fn confirmed_live_rejection_removes_the_new_envelope() -> TestResult {
    let admin_config = test_config("live-rejection", "RAZIEL_QUEUE_TEST_REDIS_URL")?;
    let restricted_config = test_config("live-rejection", "RAZIEL_QUEUE_REJECT_REDIS_URL")?;
    let queue = open_clean_queue(&admin_config).await?;
    let root = TestRoot::new("live-rejection");
    let store = PrivateEnvelopeStore::open(root.path())?;
    let publisher = CapabilityJobPublisher::connect(store.clone(), &restricted_config).await?;
    let signing_key = signing_key();
    let admitted = admit_create_capability_request(
        signed_request(valid_request(&signing_key), &signing_key),
        TRUSTED_NOW,
    )?;
    let reference = reference_for(&admitted)?;

    assert_eq!(
        publisher.publish(&admitted).await,
        Err(CapabilityPublicationError::QueuePublicationRejected)
    );
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
    assert!(queue.get_job(admitted.job_id()).await?.is_none());
    assert_eq!(queue.get_job_counts().await?.waiting, 0);

    publisher.close().await;
    clean_queue(&queue).await?;
    println!("capability_publication_failure_cleanup_live=pass");
    Ok(())
}

fn test_config(case_name: &str, redis_url_variable: &str) -> TestResult<QueueConfig> {
    let redis_url = std::env::var(redis_url_variable).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{redis_url_variable} is required"),
        )
    })?;
    let prefix = std::env::var("RAZIEL_QUEUE_TEST_PREFIX")
        .unwrap_or_else(|_| "raziel-publication".to_string());
    Ok(QueueConfig::new(
        redis_url,
        format!("{prefix}-{case_name}"),
    )?)
}

async fn open_clean_queue(config: &QueueConfig) -> TestResult<Queue> {
    let queue = Queue::with_options(CAPABILITY_QUEUE_NAME, config.queue_options()).await?;
    queue.remove_global_concurrency().await?;
    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    Ok(queue)
}

async fn clean_queue(queue: &Queue) -> TestResult {
    queue.remove_global_concurrency().await?;
    queue.obliterate(true, OBLITERATE_BATCH_SIZE).await?;
    queue.close().await;
    Ok(())
}

fn assert_job_policy(job: &bullmq::Job) {
    let options = job.opts();
    assert_eq!(options.attempts, Some(JOB_ATTEMPTS));
    let expected = bullmq::JobOptions::new()
        .attempts(JOB_ATTEMPTS)
        .deduplication(DeduplicationOptions {
            id: PROOF_CAPACITY_DEDUPLICATION_ID.into(),
            ..Default::default()
        })
        .remove_on_complete(RemoveOnFinish::Options(KeepJobs {
            age: Some(TERMINAL_RETENTION_AGE_SECONDS),
            count: Some(TERMINAL_RETENTION_COUNT),
            limit: Some(TERMINAL_RETENTION_EVICTION_LIMIT),
        }))
        .remove_on_fail(RemoveOnFinish::Options(KeepJobs {
            age: Some(TERMINAL_RETENTION_AGE_SECONDS),
            count: Some(TERMINAL_RETENTION_COUNT),
            limit: Some(TERMINAL_RETENTION_EVICTION_LIMIT),
        }));
    assert_eq!(
        serde_json::to_value(options).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

fn valid_request(signing_key: &SigningKey) -> CreateCapabilityRequest {
    CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: DISPLAY_AGENT_ID.into(),
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
        issuance_nonce: vec![0xc1; 32],
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

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn reference_for(
    admitted: &razield::admission::AdmittedCapabilityRequest,
) -> Result<PrivateEnvelopeRef, PrivateEnvelopeError> {
    PrivateEnvelopeRef::from_queue_token(&format!("pe1_{}", admitted.job_id()))
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
            "razield-live-publication-{label}-{}-{sequence}-{timestamp}",
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
