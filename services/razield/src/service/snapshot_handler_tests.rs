use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use prost_types::Timestamp;
use raziel_proto::raziel::v1::snapshot_service_client::SnapshotServiceClient;
use raziel_proto::raziel::v1::snapshot_service_server::SnapshotService as _;
use raziel_proto::raziel::v1::{
    BuildSnapshotRequest, BuildSnapshotResponse, GetSnapshotRequest, GetSnapshotResponse,
    RelationshipSnapshotDescriptor, RevokeSnapshotRequest, RevokeSnapshotResponse, SnapshotStatus,
};
use tokio::sync::oneshot;
use tonic::transport::Server;
use tonic::{Code, Request};

use super::{
    MAX_SERVICE_MESSAGE_BYTES, SnapshotOperationError, SnapshotOperations, SnapshotServiceHandler,
    bounded_snapshot_service,
};

#[tokio::test]
async fn handler_returns_public_results_from_each_operation() {
    let operations = RecordingOperations::default();
    let handler = SnapshotServiceHandler::new(operations);

    let built = handler
        .build_snapshot(Request::new(BuildSnapshotRequest::default()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(built.snapshot.unwrap().snapshot_id, snapshot_id());
    let found = handler
        .get_snapshot(Request::new(GetSnapshotRequest {
            snapshot_id: snapshot_id(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(found.status, SnapshotStatus::Accepted as i32);
    let revoked = handler
        .revoke_snapshot(Request::new(RevokeSnapshotRequest {
            snapshot_id: snapshot_id(),
            reason: "owner request".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(revoked.status, SnapshotStatus::Revoked as i32);
    assert_eq!(handler.operations().calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn handler_maps_private_failures_to_stable_generic_statuses() {
    for (error, code, message) in [
        (
            SnapshotOperationError::ConfigurationReferenceMismatch,
            Code::InvalidArgument,
            "invalid snapshot request",
        ),
        (
            SnapshotOperationError::StaleEpoch,
            Code::FailedPrecondition,
            "snapshot request cannot be applied",
        ),
        (
            SnapshotOperationError::SpiceDb,
            Code::Unavailable,
            "snapshot service unavailable",
        ),
        (
            SnapshotOperationError::RegistryConfirmation,
            Code::Internal,
            "snapshot service failed",
        ),
    ] {
        let handler = SnapshotServiceHandler::new(RecordingOperations::failing(error));
        let status = handler
            .build_snapshot(Request::new(BuildSnapshotRequest::default()))
            .await
            .unwrap_err();
        assert_eq!(status.code(), code);
        assert_eq!(status.message(), message);
    }
}

#[tokio::test]
async fn tonic_wrapper_rejects_oversized_snapshot_messages_before_side_effects() {
    let operations = Arc::new(RecordingOperations::default());
    let address = unused_loopback_address();
    let service = bounded_snapshot_service(SnapshotServiceHandler::new(Arc::clone(&operations)));
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

    let error = client
        .build_snapshot(BuildSnapshotRequest {
            spicedb_endpoint_ref: "x".repeat(MAX_SERVICE_MESSAGE_BYTES + 1),
            ..BuildSnapshotRequest::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::OutOfRange);
    assert_eq!(operations.calls.load(Ordering::SeqCst), 0);

    shutdown_tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

struct RecordingOperations {
    calls: AtomicUsize,
    failure: Option<SnapshotOperationError>,
}

impl Default for RecordingOperations {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            failure: None,
        }
    }
}

impl RecordingOperations {
    fn failing(error: SnapshotOperationError) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            failure: Some(error),
        }
    }

    fn record(&self) -> Result<(), SnapshotOperationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.failure.map_or(Ok(()), Err)
    }
}

#[tonic::async_trait]
impl SnapshotOperations for RecordingOperations {
    async fn build(
        &self,
        _request: BuildSnapshotRequest,
    ) -> Result<BuildSnapshotResponse, SnapshotOperationError> {
        self.record()?;
        Ok(BuildSnapshotResponse {
            snapshot: Some(descriptor()),
        })
    }

    async fn get(
        &self,
        _request: GetSnapshotRequest,
    ) -> Result<GetSnapshotResponse, SnapshotOperationError> {
        self.record()?;
        Ok(GetSnapshotResponse {
            snapshot: Some(descriptor()),
            status: SnapshotStatus::Accepted as i32,
        })
    }

    async fn revoke(
        &self,
        _request: RevokeSnapshotRequest,
    ) -> Result<RevokeSnapshotResponse, SnapshotOperationError> {
        self.record()?;
        Ok(RevokeSnapshotResponse {
            snapshot_id: snapshot_id(),
            status: SnapshotStatus::Revoked as i32,
            revoked_at: Some(Timestamp {
                seconds: 1_800_000_000,
                nanos: 0,
            }),
        })
    }
}

fn descriptor() -> RelationshipSnapshotDescriptor {
    RelationshipSnapshotDescriptor {
        snapshot_id: snapshot_id(),
        ..RelationshipSnapshotDescriptor::default()
    }
}

fn snapshot_id() -> String {
    format!("snapshot-v1:{}", "22".repeat(32))
}

fn unused_loopback_address() -> SocketAddr {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    address
}

async fn connect_client(address: SocketAddr) -> SnapshotServiceClient<tonic::transport::Channel> {
    let endpoint = format!("http://{address}");
    for _ in 0..50 {
        match SnapshotServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return client,
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    panic!("loopback snapshot service did not start")
}
