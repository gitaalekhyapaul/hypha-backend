use std::io::Cursor;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use bullmq::worker::CancellationToken;
use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AUTHORIZATION_PUBLIC_VALUES_VERSION,
    AuthorizationPublicValuesV1, CapabilityRequestBindingV1, capability_request_commitment,
    encode_public_values,
};
use raziel_proto::raziel::v1::CreateCapabilityRequest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use super::wire::{OneShotRequestWireV1, OneShotResultWireV1};
use super::{
    MAX_ONE_SHOT_REQUEST_BYTES, MAX_ONE_SHOT_RESULT_BYTES, ONE_SHOT_PROTOCOL_VERSION,
    OneShotFailure, OneShotFailureClass, OneShotMetrics, OneShotProtocolError, OneShotRequest,
    OneShotResult, OneShotSuccess, UnixSocketExecutor, decode_request, decode_result,
    encode_request, encode_result, read_frame_to_eof, write_frame,
};
use crate::admission::{LOCAL_PLONK_ADMISSION_WINDOW_SECONDS, admit_create_capability_request};
use crate::private_envelope::PrivateEnvelopeStore;
use crate::worker::{
    CapabilityExecutionRequest, CapabilityExecutor, ExecutionControl, ExecutorFailureClass,
    execute_with_limits,
};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PRIVATE_TASK: &str = "private-task-001";
const CORRELATION_ID: &str = "proof-job-001";

static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn request_round_trip_keeps_private_fields_out_of_debug_output() {
    let envelope = private_envelope();
    let request = OneShotRequest::new(envelope, TRUSTED_NOW, CORRELATION_ID.into()).unwrap();

    let bytes = encode_request(&request).unwrap();
    let decoded = decode_request(&bytes).unwrap();

    assert_eq!(
        decoded.envelope().request().task_id,
        request.envelope().request().task_id
    );
    assert_eq!(decoded.worker_start_unix_seconds(), TRUSTED_NOW);
    assert_eq!(decoded.correlation_id(), CORRELATION_ID);
    assert!(bytes.len() < MAX_ONE_SHOT_REQUEST_BYTES);
    assert!(!format!("{request:?}{decoded:?}").contains(PRIVATE_TASK));
}

#[test]
fn success_and_failure_results_are_strict_bounded_and_redacted() {
    let success = success();
    let encoded = encode_result(&OneShotResult::Success(Box::new(success.clone()))).unwrap();
    assert_eq!(
        decode_result(&encoded),
        Ok(OneShotResult::Success(Box::new(success.clone())))
    );
    assert!(encoded.len() < MAX_ONE_SHOT_RESULT_BYTES);
    assert!(!format!("{success:?}").contains("proof-secret-marker"));

    let failure =
        OneShotFailure::new(OneShotFailureClass::ProofFailed, CORRELATION_ID.into()).unwrap();
    let encoded = encode_result(&OneShotResult::Failure(failure.clone())).unwrap();
    assert_eq!(decode_result(&encoded), Ok(OneShotResult::Failure(failure)));
}

#[test]
fn codecs_reject_noncanonical_versions_values_and_oversized_messages() {
    let request =
        OneShotRequest::new(private_envelope(), TRUSTED_NOW, CORRELATION_ID.into()).unwrap();
    let baseline = encode_request(&request).unwrap();
    let mut wire = OneShotRequestWireV1::decode(baseline.as_slice()).unwrap();

    wire.format_version = ONE_SHOT_PROTOCOL_VERSION + 1;
    assert!(matches!(
        decode_request(&wire.encode_to_vec()),
        Err(OneShotProtocolError::UnsupportedVersion)
    ));
    wire.format_version = ONE_SHOT_PROTOCOL_VERSION;
    wire.deadline_seconds += 1;
    assert!(matches!(
        decode_request(&wire.encode_to_vec()),
        Err(OneShotProtocolError::InvalidRequest)
    ));

    let mut unknown = baseline;
    unknown.extend_from_slice(&[0x30, 0x01]);
    assert!(matches!(
        decode_request(&unknown),
        Err(OneShotProtocolError::MalformedRequest)
    ));
    assert!(matches!(
        decode_request(&vec![0_u8; MAX_ONE_SHOT_REQUEST_BYTES + 1]),
        Err(OneShotProtocolError::RequestTooLarge)
    ));

    let invalid_result = OneShotResultWireV1 {
        format_version: ONE_SHOT_PROTOCOL_VERSION,
        outcome: None,
    }
    .encode_to_vec();
    assert_eq!(
        decode_result(&invalid_result),
        Err(OneShotProtocolError::InvalidResult)
    );
}

#[test]
fn frame_requires_one_complete_payload_and_eof() {
    let payload = b"bounded-message";
    let mut framed = Vec::new();
    write_frame(&mut framed, payload, 64).unwrap();
    assert_eq!(
        read_frame_to_eof(&mut Cursor::new(&framed), 64).unwrap(),
        payload
    );

    let mut trailing = framed.clone();
    trailing.push(0);
    assert_eq!(
        read_frame_to_eof(&mut Cursor::new(trailing), 64),
        Err(OneShotProtocolError::TrailingFrameData)
    );
    assert_eq!(
        read_frame_to_eof(&mut Cursor::new(&framed), payload.len() - 1),
        Err(OneShotProtocolError::FrameRead)
    );
}

#[test]
fn result_validation_rejects_bad_public_values_keys_and_resource_claims() {
    let metrics = metrics(19);
    assert_eq!(
        OneShotSuccess::new(
            b"proof-secret-marker".to_vec(),
            vec![0xff],
            verification_key_id(),
            metrics
        ),
        Err(OneShotProtocolError::InvalidResult)
    );
    assert_eq!(
        OneShotSuccess::new(
            b"proof-secret-marker".to_vec(),
            public_values(),
            "sp1-vk-v1:ABC".into(),
            metrics
        ),
        Err(OneShotProtocolError::InvalidResult)
    );
    let mut invalid_metrics = metrics;
    invalid_metrics.peak_swap_bytes = 1;
    assert_eq!(
        OneShotSuccess::new(
            b"proof-secret-marker".to_vec(),
            public_values(),
            verification_key_id(),
            invalid_metrics
        ),
        Err(OneShotProtocolError::InvalidResult)
    );
}

#[tokio::test]
async fn unix_executor_exchanges_one_strict_request_and_result() {
    let root = TestRoot::new();
    let (listener, socket_path, owner_uid) = test_listener(&root);
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request =
            decode_request(&read_frame_async(&mut stream, MAX_ONE_SHOT_REQUEST_BYTES).await)
                .unwrap();
        assert_eq!(request.worker_start_unix_seconds(), TRUSTED_NOW);
        let result = encode_result(&OneShotResult::Success(Box::new(success()))).unwrap();
        write_frame_async(&mut stream, &result).await;
        stream.shutdown().await.unwrap();
    });

    let executor = UnixSocketExecutor::new(socket_path, owner_uid).unwrap();
    let result = executor
        .execute(
            CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
            ExecutionControl::new(Duration::from_secs(5), CancellationToken::new()),
        )
        .await
        .unwrap();
    assert_eq!(result.metrics(), metrics(19));
    server.await.unwrap();
}

#[tokio::test]
async fn unix_executor_waits_for_launcher_cancellation_acknowledgement() {
    let root = TestRoot::new();
    let (listener, socket_path, owner_uid) = test_listener(&root);
    let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
    let (exit_confirmed_tx, exit_confirmed_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request =
            decode_request(&read_frame_async(&mut stream, MAX_ONE_SHOT_REQUEST_BYTES).await)
                .unwrap();
        request_seen_tx.send(()).unwrap();
        assert_eq!(stream.read_u8().await.unwrap(), 1);
        assert_eq!(
            stream.read_u8().await.unwrap_err().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
        exit_confirmed_tx.send(()).unwrap();
        let failure = OneShotFailure::new(
            OneShotFailureClass::Cancelled,
            request.correlation_id().to_owned(),
        )
        .unwrap();
        let result = encode_result(&OneShotResult::Failure(failure)).unwrap();
        write_frame_async(&mut stream, &result).await;
        stream.shutdown().await.unwrap();
    });

    let cancellation = CancellationToken::new();
    let execution = tokio::spawn(
        UnixSocketExecutor::new(socket_path, owner_uid)
            .unwrap()
            .execute(
                CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
                ExecutionControl::new(Duration::from_secs(5), cancellation.clone()),
            ),
    );
    request_seen_rx.await.unwrap();
    cancellation.cancel();
    assert_eq!(
        execution.await.unwrap(),
        Err(ExecutorFailureClass::Cancelled)
    );
    exit_confirmed_rx.await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn unix_executor_rejects_an_unsafe_launcher_socket() {
    let root = TestRoot::new();
    let (_listener, socket_path, owner_uid) = test_listener(&root);
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660)).unwrap();
    let result = UnixSocketExecutor::new(socket_path.clone(), owner_uid)
        .unwrap()
        .execute(
            CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
            ExecutionControl::new(Duration::from_secs(5), CancellationToken::new()),
        )
        .await;
    assert_eq!(result, Err(ExecutorFailureClass::Unavailable));

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let result = UnixSocketExecutor::new(socket_path, owner_uid)
        .unwrap()
        .execute(
            CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
            ExecutionControl::new(Duration::from_secs(5), CancellationToken::new()),
        )
        .await;
    assert_eq!(result, Err(ExecutorFailureClass::Unavailable));
}

#[tokio::test]
async fn unix_executor_requires_a_canonical_termination_acknowledgement() {
    let root = TestRoot::new();
    let (listener, socket_path, owner_uid) = test_listener(&root);
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = read_frame_async(&mut stream, MAX_ONE_SHOT_REQUEST_BYTES).await;
        assert_eq!(stream.read_u8().await.unwrap(), 1);
    });
    let executor = UnixSocketExecutor::new(socket_path, owner_uid).unwrap();
    let result = execute_with_limits(
        &executor,
        CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
        CancellationToken::new(),
        Duration::from_millis(20),
        Duration::from_millis(50),
    )
    .await;
    assert_eq!(result, Err(ExecutorFailureClass::TerminationUnconfirmed));
    server.await.unwrap();
}

#[tokio::test]
async fn unix_executor_rejects_a_non_cancellation_terminal_acknowledgement() {
    let root = TestRoot::new();
    let (listener, socket_path, owner_uid) = test_listener(&root);
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request =
            decode_request(&read_frame_async(&mut stream, MAX_ONE_SHOT_REQUEST_BYTES).await)
                .unwrap();
        assert_eq!(stream.read_u8().await.unwrap(), 1);
        let failure = OneShotFailure::new(
            OneShotFailureClass::Internal,
            request.correlation_id().to_owned(),
        )
        .unwrap();
        let result = encode_result(&OneShotResult::Failure(failure)).unwrap();
        write_frame_async(&mut stream, &result).await;
        stream.shutdown().await.unwrap();
    });

    let executor = UnixSocketExecutor::new(socket_path, owner_uid).unwrap();
    let result = execute_with_limits(
        &executor,
        CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
        CancellationToken::new(),
        Duration::from_millis(20),
        Duration::from_millis(50),
    )
    .await;
    assert_eq!(result, Err(ExecutorFailureClass::TerminationUnconfirmed));
    server.await.unwrap();
}

fn success() -> OneShotSuccess {
    OneShotSuccess::new(
        b"proof-secret-marker".to_vec(),
        public_values(),
        verification_key_id(),
        metrics(19),
    )
    .unwrap()
}

const fn metrics(artifact_bytes: u64) -> OneShotMetrics {
    OneShotMetrics {
        execute_millis: 400,
        prove_millis: 1_700_000,
        artifact_bytes,
        peak_memory_bytes: 46_301_016_064,
        peak_swap_bytes: 0,
    }
}

fn public_values() -> Vec<u8> {
    encode_public_values(&AuthorizationPublicValuesV1 {
        format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
        policy_hash: [0x11; 32],
        relationship_snapshot_root: [0x22; 32],
        capability_request_commitment: [0x33; 32],
        capability_scope_hash: [0x44; 32],
        issued_at_unix_seconds: TRUSTED_NOW,
        expires_at_unix_seconds: TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
        issuance_nullifier: [0x55; 32],
    })
    .unwrap()
}

fn verification_key_id() -> String {
    format!("sp1-vk-v1:{}", "ab".repeat(32))
}

pub(crate) fn private_envelope() -> crate::private_envelope::PrivateCapabilityEnvelope {
    let root = TestRoot::new();
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let mut transport = CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: "synthetic-agent".into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: PRIVATE_TASK.into(),
        purpose: "payment-reconciliation".into(),
        action: "access".into(),
        resource_type: "dataset".into(),
        resource_scope: "private-payments".into(),
        requested_expires_at: Some(Timestamp {
            seconds: TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
            nanos: 0,
        }),
        issuance_nonce: vec![0x88; 32],
        snapshot_id: "snapshot-private-test".into(),
        request_signature: Vec::new(),
        request_context: vec![],
    };
    let public_key: [u8; 32] = transport.agent_public_key.as_slice().try_into().unwrap();
    let binding = CapabilityRequestBindingV1 {
        format_version: transport.format_version,
        agent_key_thumbprint: derive_ed25519_key_thumbprint(&public_key),
        task_id: transport.task_id.clone(),
        purpose: transport.purpose.clone(),
        action: transport.action.clone(),
        resource_type: transport.resource_type.clone(),
        resource_scope: transport.resource_scope.clone(),
        requested_expires_at_unix_seconds: transport.requested_expires_at.unwrap().seconds,
        issuance_nonce: transport.issuance_nonce.as_slice().try_into().unwrap(),
        snapshot_id: transport.snapshot_id.clone(),
        request_context: vec![],
    };
    transport.request_signature = signing_key
        .sign(&capability_request_commitment(&binding).unwrap())
        .to_bytes()
        .to_vec();
    let admitted = admit_create_capability_request(transport, TRUSTED_NOW).unwrap();
    let reference = store.stage(&admitted).unwrap();
    store.read(&reference).unwrap()
}

fn test_listener(root: &TestRoot) -> (UnixListener, PathBuf, u32) {
    std::fs::create_dir(root.path()).unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket_path = root.path().join("launcher.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let owner_uid = std::fs::symlink_metadata(&socket_path).unwrap().uid();
    (listener, socket_path, owner_uid)
}

async fn read_frame_async(stream: &mut UnixStream, maximum: usize) -> Vec<u8> {
    let length = stream.read_u32().await.unwrap() as usize;
    assert!(length > 0 && length <= maximum);
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).await.unwrap();
    payload
}

async fn write_frame_async(stream: &mut UnixStream, payload: &[u8]) {
    stream.write_u32(payload.len() as u32).await.unwrap();
    stream.write_all(payload).await.unwrap();
    stream.flush().await.unwrap();
}

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new() -> Self {
        let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self {
            path: PathBuf::from("/tmp").join(format!(
                "razield-one-shot-{}-{timestamp}-{sequence}",
                std::process::id()
            )),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
