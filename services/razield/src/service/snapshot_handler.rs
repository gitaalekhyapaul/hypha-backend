use std::sync::Arc;

use raziel_proto::raziel::v1::snapshot_service_server::{SnapshotService, SnapshotServiceServer};
use raziel_proto::raziel::v1::{
    BuildSnapshotRequest, BuildSnapshotResponse, GetSnapshotRequest, GetSnapshotResponse,
    RevokeSnapshotRequest, RevokeSnapshotResponse,
};
use tonic::{Request, Response, Status};

use super::{MAX_SERVICE_MESSAGE_BYTES, SnapshotOperationError};

#[tonic::async_trait]
pub trait SnapshotOperations: Send + Sync + 'static {
    async fn build(
        &self,
        request: BuildSnapshotRequest,
    ) -> Result<BuildSnapshotResponse, SnapshotOperationError>;

    async fn get(
        &self,
        request: GetSnapshotRequest,
    ) -> Result<GetSnapshotResponse, SnapshotOperationError>;

    async fn revoke(
        &self,
        request: RevokeSnapshotRequest,
    ) -> Result<RevokeSnapshotResponse, SnapshotOperationError>;
}

#[tonic::async_trait]
impl<O> SnapshotOperations for Arc<O>
where
    O: SnapshotOperations,
{
    async fn build(
        &self,
        request: BuildSnapshotRequest,
    ) -> Result<BuildSnapshotResponse, SnapshotOperationError> {
        self.as_ref().build(request).await
    }

    async fn get(
        &self,
        request: GetSnapshotRequest,
    ) -> Result<GetSnapshotResponse, SnapshotOperationError> {
        self.as_ref().get(request).await
    }

    async fn revoke(
        &self,
        request: RevokeSnapshotRequest,
    ) -> Result<RevokeSnapshotResponse, SnapshotOperationError> {
        self.as_ref().revoke(request).await
    }
}

pub struct SnapshotServiceHandler<O> {
    operations: O,
}

impl<O> SnapshotServiceHandler<O> {
    pub const fn new(operations: O) -> Self {
        Self { operations }
    }

    #[cfg(test)]
    pub(crate) const fn operations(&self) -> &O {
        &self.operations
    }
}

pub fn bounded_snapshot_service<O>(
    handler: SnapshotServiceHandler<O>,
) -> SnapshotServiceServer<SnapshotServiceHandler<O>>
where
    O: SnapshotOperations,
{
    SnapshotServiceServer::new(handler).max_decoding_message_size(MAX_SERVICE_MESSAGE_BYTES)
}

#[tonic::async_trait]
impl<O> SnapshotService for SnapshotServiceHandler<O>
where
    O: SnapshotOperations,
{
    async fn build_snapshot(
        &self,
        request: Request<BuildSnapshotRequest>,
    ) -> Result<Response<BuildSnapshotResponse>, Status> {
        self.operations
            .build(request.into_inner())
            .await
            .map(Response::new)
            .map_err(map_operation_error)
    }

    async fn get_snapshot(
        &self,
        request: Request<GetSnapshotRequest>,
    ) -> Result<Response<GetSnapshotResponse>, Status> {
        self.operations
            .get(request.into_inner())
            .await
            .map(Response::new)
            .map_err(map_operation_error)
    }

    async fn revoke_snapshot(
        &self,
        request: Request<RevokeSnapshotRequest>,
    ) -> Result<Response<RevokeSnapshotResponse>, Status> {
        self.operations
            .revoke(request.into_inner())
            .await
            .map(Response::new)
            .map_err(map_operation_error)
    }
}

fn map_operation_error(error: SnapshotOperationError) -> Status {
    match error {
        SnapshotOperationError::InvalidRequest
        | SnapshotOperationError::ConfigurationReferenceMismatch
        | SnapshotOperationError::UnknownSnapshot => {
            Status::invalid_argument("invalid snapshot request")
        }
        SnapshotOperationError::StaleEpoch
        | SnapshotOperationError::WriterStaleGeneration
        | SnapshotOperationError::WriterConflict
        | SnapshotOperationError::AlreadyRevoked => {
            Status::failed_precondition("snapshot request cannot be applied")
        }
        SnapshotOperationError::SpiceDb
        | SnapshotOperationError::WriterUnavailable
        | SnapshotOperationError::PrivateStore
        | SnapshotOperationError::SecretRead => Status::unavailable("snapshot service unavailable"),
        SnapshotOperationError::Clock
        | SnapshotOperationError::RegistryRead
        | SnapshotOperationError::PolicyResolution
        | SnapshotOperationError::PolicyGuestLimit
        | SnapshotOperationError::AuthorityKey
        | SnapshotOperationError::InvalidValidityWindow
        | SnapshotOperationError::Entropy
        | SnapshotOperationError::SnapshotBuild
        | SnapshotOperationError::Signing
        | SnapshotOperationError::WriterRejected
        | SnapshotOperationError::RegistryConfirmation
        | SnapshotOperationError::BlockingTask => Status::internal("snapshot service failed"),
    }
}
