use std::error::Error;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bullmq::Queue;
use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, capability_request_commitment,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::capability_service_client::CapabilityServiceClient;
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CreateCapabilityRequest, ProofJobState, authorization_context_entry,
};
use razield::admission::{LOCAL_PLONK_ADMISSION_WINDOW_SECONDS, admit_create_capability_request};
use razield::private_envelope::{PrivateEnvelopeError, PrivateEnvelopeRef, PrivateEnvelopeStore};
use razield::publication::CapabilityJobPublisher;
use razield::queue::{CAPABILITY_QUEUE_NAME, QueueConfig};
use razield::service::{ServiceConfig, run_until_shutdown};
use tokio::sync::oneshot;
use tonic::Code;

const PRIVATE_CONTEXT_VALUE: &str = "private-merchant-4829";
const OBLITERATE_BATCH_SIZE: usize = 1_000;

static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the digest-pinned Redis capability service"]
async fn live_create_uses_startup_reconciliation_and_returns_queued() -> TestResult {
    let redis_url = required_env("RAZIEL_QUEUE_TEST_REDIS_URL")?;
    let prefix = unique_prefix();
    let queue_config = QueueConfig::new(redis_url.clone(), prefix.clone())?;
    let queue = open_clean_queue(&queue_config).await?;
    let root = TestRoot::new("capability-service");
    let store = PrivateEnvelopeStore::open(root.path())?;
    let trusted_now = unix_now()?;
    let request = signed_request(valid_request(trusted_now, 0xd0, "display-agent"));
    let admitted = admit_create_capability_request(request.clone(), trusted_now)?;
    let publisher = CapabilityJobPublisher::connect(store.clone(), &queue_config).await?;
    publisher.publish(&admitted).await?;

    let orphan_request = signed_request(valid_request(trusted_now, 0xd1, "orphan-agent"));
    let orphan = admit_create_capability_request(orphan_request, trusted_now)?;
    let orphan_reference = store.stage(&orphan)?;
    set_modified(
        &envelope_path(root.path(), orphan_reference),
        SystemTime::now() - Duration::from_secs(20 * 60),
    )?;
    let live_reference = reference_for_job(admitted.job_id())?;
    publisher.close().await;
    drop(store);

    let address = unused_loopback_address()?;
    let service_config = ServiceConfig::from_values(
        &address.to_string(),
        &redis_url,
        &prefix,
        root.path()
            .to_str()
            .ok_or_else(|| io::Error::other("test root is not UTF-8"))?,
    )?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        run_until_shutdown(service_config, async {
            let _ = shutdown_rx.await;
        })
        .await
    });
    let mut client = connect_client(address).await?;

    let reconciled_store = PrivateEnvelopeStore::open(root.path())?;
    reconciled_store.read(&live_reference)?;
    assert_eq!(
        reconciled_store.read(&orphan_reference),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );

    let mut renamed = request.clone();
    renamed.agent_id = "renamed-display-agent".into();
    let response = client.create_capability(renamed).await?.into_inner();
    let job = response
        .job
        .ok_or_else(|| io::Error::other("CreateCapability returned no job"))?;
    assert_eq!(job.job_id, admitted.job_id());
    assert_eq!(job.state, ProofJobState::Queued as i32);
    assert!(job.submitted_at.is_some());
    assert!(job.updated_at.is_some());
    assert!(job.capability.is_none());
    assert!(job.internal_error_ref.is_none());
    assert!(job.metrics.is_none());
    assert!(!format!("{job:?}").contains(PRIVATE_CONTEXT_VALUE));

    let mut mutated = request;
    mutated.resource_scope.push_str("-changed");
    let error = client.create_capability(mutated).await.unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(queue.get_job_counts().await?.waiting, 1);

    shutdown_tx
        .send(())
        .map_err(|_| io::Error::other("service shutdown receiver closed"))?;
    server.await??;
    clean_queue(&queue).await?;
    println!("capability_service_live=pass");
    Ok(())
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{name} is required")).into()
    })
}

fn unique_prefix() -> String {
    format!(
        "raziel-service-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos()
    )
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

fn valid_request(trusted_now: i64, nonce_byte: u8, agent_id: &str) -> CreateCapabilityRequest {
    let signing_key = signing_key();
    CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: agent_id.into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: "task-2026-08-22".into(),
        purpose: "reconcile merchant settlement records".into(),
        action: "reconcile".into(),
        resource_type: "payment_batch".into(),
        resource_scope: "batch:synthetic-001".into(),
        requested_expires_at: Some(Timestamp {
            seconds: trusted_now + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
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

fn signed_request(mut request: CreateCapabilityRequest) -> CreateCapabilityRequest {
    let commitment = capability_request_commitment(&expected_binding(&request)).unwrap();
    request.request_signature = signing_key().sign(&commitment).to_bytes().to_vec();
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

fn unix_now() -> TestResult<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(seconds).map_err(Into::into)
}

fn reference_for_job(job_id: &str) -> Result<PrivateEnvelopeRef, PrivateEnvelopeError> {
    PrivateEnvelopeRef::from_queue_token(&format!("pe1_{job_id}"))
}

fn envelope_path(root: &Path, reference: PrivateEnvelopeRef) -> PathBuf {
    root.join(format!("{}.pb", reference.queue_token()))
}

fn set_modified(path: &Path, time: SystemTime) -> io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.set_times(std::fs::FileTimes::new().set_modified(time))
}

fn unused_loopback_address() -> io::Result<SocketAddr> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let address = listener.local_addr()?;
    drop(listener);
    Ok(address)
}

async fn connect_client(
    address: SocketAddr,
) -> TestResult<CapabilityServiceClient<tonic::transport::Channel>> {
    let endpoint = format!("http://{address}");
    for _ in 0..100 {
        match CapabilityServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    Err(io::Error::other("loopback capability service did not start").into())
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
            "razield-live-service-{label}-{}-{sequence}-{timestamp}",
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
