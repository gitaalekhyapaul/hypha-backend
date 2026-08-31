use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, capability_request_commitment,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::capability_service_client::CapabilityServiceClient;
use raziel_proto::raziel::v1::capability_service_server::CapabilityService as _;
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CancelCapabilityRequest, CreateCapabilityRequest,
    GetCapabilityRequest, ProofJobState, PublicErrorCode, authorization_context_entry,
};
use tokio::sync::oneshot;
use tonic::transport::Server;
use tonic::{Code, Request};

use super::{
    CapabilityJobAccess, CapabilityServiceHandler, ClockError, MAX_SERVICE_MESSAGE_BYTES,
    TrustedClock, bounded_capability_service,
};
use crate::admission::{AdmittedCapabilityRequest, LOCAL_PLONK_ADMISSION_WINDOW_SECONDS};
use crate::publication::{
    CapabilityLifecycleError, CapabilityPublicationError, PublicCapabilityJobState,
    PublicationDisposition, PublishedCapabilityJob, RetainedCapabilityJob,
};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PUBLISHED_AT_MILLIS: u64 = 1_800_000_000_125;

#[tokio::test]
async fn valid_create_returns_the_authoritative_queued_job() {
    let publisher = RecordingPublisher::default();
    let service = CapabilityServiceHandler::new(publisher, FixedClock(Ok(TRUSTED_NOW)));
    let request = signed_request("display-agent");

    let response = service
        .create_capability(Request::new(request.clone()))
        .await
        .unwrap()
        .into_inner();
    let job = response.job.unwrap();

    assert_eq!(job.job_id.len(), 64);
    assert_eq!(job.state, ProofJobState::Queued as i32);
    assert_eq!(
        job.submitted_at,
        Some(Timestamp {
            seconds: TRUSTED_NOW,
            nanos: 125_000_000,
        })
    );
    assert_eq!(job.updated_at, job.submitted_at);
    assert_eq!(job.public_error_code, PublicErrorCode::Unspecified as i32);
    assert!(job.capability.is_none());
    assert!(job.internal_error_ref.is_none());
    assert!(job.metrics.is_none());
    assert_eq!(service.publisher().calls(), 1);
    assert_eq!(request.agent_id, "display-agent");
}

#[tokio::test]
async fn display_identity_is_non_authoritative_but_bound_mutations_stop_before_publication() {
    let service =
        CapabilityServiceHandler::new(RecordingPublisher::default(), FixedClock(Ok(TRUSTED_NOW)));
    let original = signed_request("first-display-name");
    let first = service
        .create_capability(Request::new(original.clone()))
        .await
        .unwrap()
        .into_inner()
        .job
        .unwrap();

    let mut renamed = original.clone();
    renamed.agent_id = "second-display-name".into();
    let second = service
        .create_capability(Request::new(renamed))
        .await
        .unwrap()
        .into_inner()
        .job
        .unwrap();
    assert_eq!(first.job_id, second.job_id);
    assert_eq!(service.publisher().calls(), 2);

    let mut mutated = original;
    mutated.task_id.push_str("-changed");
    let error = service
        .create_capability(Request::new(mutated))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "invalid capability request");
    assert_eq!(service.publisher().calls(), 2);
}

#[tokio::test]
async fn clock_and_publication_failures_map_to_stable_generic_statuses() {
    let clock_failure = CapabilityServiceHandler::new(
        RecordingPublisher::default(),
        FixedClock(Err(ClockError::OutOfRange)),
    );
    let error = clock_failure
        .create_capability(Request::new(signed_request("clock-failure")))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unavailable);
    assert_eq!(error.message(), "capability service unavailable");
    assert_eq!(clock_failure.publisher().calls(), 0);

    let queue_failure = CapabilityServiceHandler::new(
        RecordingPublisher::failing(CapabilityPublicationError::QueueUnavailable),
        FixedClock(Ok(TRUSTED_NOW)),
    );
    let error = queue_failure
        .create_capability(Request::new(signed_request("queue-failure")))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unavailable);
    assert_eq!(error.message(), "capability service unavailable");
    assert_eq!(queue_failure.publisher().calls(), 1);

    let mismatch = CapabilityServiceHandler::new(
        RecordingPublisher::failing(CapabilityPublicationError::QueueRecordMismatch),
        FixedClock(Ok(TRUSTED_NOW)),
    );
    let error = mismatch
        .create_capability(Request::new(signed_request("queue-mismatch")))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Internal);
    assert_eq!(error.message(), "capability service failed");

    let busy = CapabilityServiceHandler::new(
        RecordingPublisher::failing(CapabilityPublicationError::QueueCapacityOccupied),
        FixedClock(Ok(TRUSTED_NOW)),
    );
    let error = busy
        .create_capability(Request::new(signed_request("queue-busy")))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::ResourceExhausted);
    assert_eq!(error.message(), "proof capacity is occupied");
}

#[tokio::test]
async fn out_of_range_queue_timestamp_fails_closed() {
    let publisher = RecordingPublisher::with_timestamp(253_402_300_800_000);
    let service = CapabilityServiceHandler::new(publisher, FixedClock(Ok(TRUSTED_NOW)));

    let error = service
        .create_capability(Request::new(signed_request("timestamp-failure")))
        .await
        .unwrap_err();

    assert_eq!(error.code(), Code::Internal);
    assert_eq!(error.message(), "capability service failed");
    assert_eq!(service.publisher().calls(), 1);
}

#[tokio::test]
async fn get_and_cancel_validate_identifiers_and_map_missing_jobs() {
    let service =
        CapabilityServiceHandler::new(RecordingPublisher::default(), FixedClock(Ok(TRUSTED_NOW)));

    let get = service
        .get_capability(Request::new(GetCapabilityRequest {
            job_id: "00".repeat(32),
        }))
        .await
        .unwrap_err();
    let cancel = service
        .cancel_capability(Request::new(CancelCapabilityRequest {
            job_id: "00".repeat(32),
        }))
        .await
        .unwrap_err();
    assert_eq!(get.code(), Code::NotFound);
    assert_eq!(get.message(), "capability job not found");
    assert_eq!(cancel.code(), Code::NotFound);
    assert_eq!(cancel.message(), "capability job not found");

    let malformed = service
        .get_capability(Request::new(GetCapabilityRequest {
            job_id: "AA".repeat(32),
        }))
        .await
        .unwrap_err();
    assert_eq!(malformed.code(), Code::InvalidArgument);
    assert_eq!(malformed.message(), "invalid capability job");
    assert_eq!(service.publisher().calls(), 0);
}

#[tokio::test]
async fn tonic_wrapper_rejects_oversized_messages_before_handler_side_effects() {
    let publisher = Arc::new(RecordingPublisher::default());
    let address = unused_loopback_address();
    let service = bounded_capability_service(CapabilityServiceHandler::new(
        Arc::clone(&publisher),
        FixedClock(Ok(TRUSTED_NOW)),
    ));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_shutdown(address, async {
                let _ = shutdown_rx.await;
            })
            .await
    });
    let mut client = connect_client(address).await;

    let oversized = CreateCapabilityRequest {
        agent_id: "x".repeat(MAX_SERVICE_MESSAGE_BYTES + 1),
        ..CreateCapabilityRequest::default()
    };
    let error = client.create_capability(oversized).await.unwrap_err();
    assert_eq!(error.code(), Code::OutOfRange);
    assert_eq!(publisher.calls(), 0);

    let malformed = client
        .create_capability(CreateCapabilityRequest::default())
        .await
        .unwrap_err();
    assert_eq!(malformed.code(), Code::InvalidArgument);
    assert_eq!(publisher.calls(), 0);

    shutdown_tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[derive(Clone, Copy)]
struct FixedClock(Result<i64, ClockError>);

impl TrustedClock for FixedClock {
    fn now_unix_seconds(&self) -> Result<i64, ClockError> {
        self.0
    }
}

struct RecordingPublisher {
    calls: AtomicUsize,
    failure: Option<CapabilityPublicationError>,
    timestamp_ms: u64,
}

impl Default for RecordingPublisher {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            failure: None,
            timestamp_ms: PUBLISHED_AT_MILLIS,
        }
    }
}

impl RecordingPublisher {
    fn failing(failure: CapabilityPublicationError) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            failure: Some(failure),
            timestamp_ms: PUBLISHED_AT_MILLIS,
        }
    }

    fn with_timestamp(timestamp_ms: u64) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            failure: None,
            timestamp_ms,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[tonic::async_trait]
impl CapabilityJobAccess for RecordingPublisher {
    async fn publish(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        Ok(PublishedCapabilityJob::from_authoritative_fields(
            admitted.job_id().into(),
            PublicationDisposition::Created,
            PublicCapabilityJobState::Queued,
            self.timestamp_ms,
            self.timestamp_ms,
        ))
    }

    async fn get(&self, job_id: &str) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        validate_mock_job_id(job_id)?;
        Err(CapabilityLifecycleError::NotFound)
    }

    async fn cancel(
        &self,
        job_id: &str,
    ) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        validate_mock_job_id(job_id)?;
        Err(CapabilityLifecycleError::NotFound)
    }
}

fn validate_mock_job_id(job_id: &str) -> Result<(), CapabilityLifecycleError> {
    if job_id.len() == 64
        && job_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CapabilityLifecycleError::InvalidJobId)
    }
}

fn signed_request(agent_id: &str) -> CreateCapabilityRequest {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let mut request = CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: agent_id.into(),
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
        issuance_nonce: vec![0xa5; 32],
        snapshot_id: "snapshot-synthetic-001".into(),
        request_signature: Vec::new(),
        request_context: vec![AuthorizationContextEntry {
            name: "department".into(),
            value: Some(authorization_context_entry::Value::StringValue(
                "private-merchant-4829".into(),
            )),
        }],
    };
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

fn unused_loopback_address() -> SocketAddr {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    address
}

async fn connect_client(address: SocketAddr) -> CapabilityServiceClient<tonic::transport::Channel> {
    let endpoint = format!("http://{address}");
    for _ in 0..50 {
        match CapabilityServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return client,
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    panic!("loopback capability service did not start")
}
