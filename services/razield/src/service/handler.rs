use prost_types::Timestamp;
use raziel_proto::raziel::v1::capability_service_server::{
    CapabilityService, CapabilityServiceServer,
};
use raziel_proto::raziel::v1::{
    CancelCapabilityRequest, CancelCapabilityResponse, CreateCapabilityRequest,
    CreateCapabilityResponse, GetCapabilityRequest, GetCapabilityResponse, ProofJob, ProofJobState,
    PublicErrorCode,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

use super::{MAX_SERVICE_MESSAGE_BYTES, TrustedClock};
use crate::admission::{
    AdmittedCapabilityRequest, MAX_PROTOBUF_TIMESTAMP_SECONDS, admit_create_capability_request,
};
use crate::publication::{
    CapabilityJobPublisher, CapabilityLifecycleError, CapabilityPublicationError,
    PublicCapabilityJobState, PublishedCapabilityJob, RetainedCapabilityJob,
};
use crate::worker::PublicFailureClass;

/// Narrow lifecycle seam used by the RPC handler and deterministic tests.
#[tonic::async_trait]
pub trait CapabilityJobAccess: Send + Sync + 'static {
    async fn publish(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PublishedCapabilityJob, CapabilityPublicationError>;

    async fn get(&self, job_id: &str) -> Result<RetainedCapabilityJob, CapabilityLifecycleError>;

    async fn cancel(&self, job_id: &str)
    -> Result<RetainedCapabilityJob, CapabilityLifecycleError>;
}

#[tonic::async_trait]
impl CapabilityJobAccess for CapabilityJobPublisher {
    async fn publish(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
        CapabilityJobPublisher::publish(self, admitted).await
    }

    async fn get(&self, job_id: &str) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        CapabilityJobPublisher::get(self, job_id).await
    }

    async fn cancel(
        &self,
        job_id: &str,
    ) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        CapabilityJobPublisher::cancel(self, job_id).await
    }
}

#[tonic::async_trait]
impl<P> CapabilityJobAccess for Arc<P>
where
    P: CapabilityJobAccess,
{
    async fn publish(
        &self,
        admitted: &AdmittedCapabilityRequest,
    ) -> Result<PublishedCapabilityJob, CapabilityPublicationError> {
        self.as_ref().publish(admitted).await
    }

    async fn get(&self, job_id: &str) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        self.as_ref().get(job_id).await
    }

    async fn cancel(
        &self,
        job_id: &str,
    ) -> Result<RetainedCapabilityJob, CapabilityLifecycleError> {
        self.as_ref().cancel(job_id).await
    }
}

/// Tonic capability boundary with injected owner clock and durable publisher.
pub struct CapabilityServiceHandler<P, C> {
    publisher: P,
    clock: C,
}

impl<P, C> CapabilityServiceHandler<P, C> {
    pub const fn new(publisher: P, clock: C) -> Self {
        Self { publisher, clock }
    }

    #[cfg(test)]
    pub(crate) const fn publisher(&self) -> &P {
        &self.publisher
    }
}

/// Wraps the handler with the accepted pre-handler request ceiling.
pub fn bounded_capability_service<P, C>(
    handler: CapabilityServiceHandler<P, C>,
) -> CapabilityServiceServer<CapabilityServiceHandler<P, C>>
where
    P: CapabilityJobAccess,
    C: TrustedClock,
{
    CapabilityServiceServer::new(handler).max_decoding_message_size(MAX_SERVICE_MESSAGE_BYTES)
}

#[tonic::async_trait]
impl<P, C> CapabilityService for CapabilityServiceHandler<P, C>
where
    P: CapabilityJobAccess,
    C: TrustedClock,
{
    async fn create_capability(
        &self,
        request: Request<CreateCapabilityRequest>,
    ) -> Result<Response<CreateCapabilityResponse>, Status> {
        let trusted_now = self
            .clock
            .now_unix_seconds()
            .map_err(|_| unavailable_status())?;
        let admitted = admit_create_capability_request(request.into_inner(), trusted_now)
            .map_err(|_| Status::invalid_argument("invalid capability request"))?;
        let published = self
            .publisher
            .publish(&admitted)
            .await
            .map_err(map_publication_error)?;
        let job = proof_job_from_publication(published)?;
        Ok(Response::new(CreateCapabilityResponse { job: Some(job) }))
    }

    async fn get_capability(
        &self,
        request: Request<GetCapabilityRequest>,
    ) -> Result<Response<GetCapabilityResponse>, Status> {
        let retained = self
            .publisher
            .get(&request.into_inner().job_id)
            .await
            .map_err(map_lifecycle_error)?;
        Ok(Response::new(GetCapabilityResponse {
            job: Some(proof_job_from_retained(retained)?),
        }))
    }

    async fn cancel_capability(
        &self,
        request: Request<CancelCapabilityRequest>,
    ) -> Result<Response<CancelCapabilityResponse>, Status> {
        let retained = self
            .publisher
            .cancel(&request.into_inner().job_id)
            .await
            .map_err(map_lifecycle_error)?;
        Ok(Response::new(CancelCapabilityResponse {
            job: Some(proof_job_from_retained(retained)?),
        }))
    }
}

fn proof_job_from_publication(published: PublishedCapabilityJob) -> Result<ProofJob, Status> {
    let state = match published.state() {
        PublicCapabilityJobState::Queued => ProofJobState::Queued,
        PublicCapabilityJobState::Executing => ProofJobState::Executing,
        PublicCapabilityJobState::Proving => ProofJobState::Proving,
        PublicCapabilityJobState::Succeeded
        | PublicCapabilityJobState::Failed
        | PublicCapabilityJobState::Canceled => {
            return Err(Status::internal("capability service failed"));
        }
    };
    Ok(ProofJob {
        job_id: published.job_id().to_owned(),
        state: state as i32,
        submitted_at: Some(timestamp_from_unix_millis(
            published.submitted_at_unix_millis(),
        )?),
        updated_at: Some(timestamp_from_unix_millis(
            published.updated_at_unix_millis(),
        )?),
        capability: None,
        public_error_code: PublicErrorCode::Unspecified as i32,
        internal_error_ref: None,
        metrics: None,
    })
}

fn proof_job_from_retained(retained: RetainedCapabilityJob) -> Result<ProofJob, Status> {
    let state = match retained.state() {
        PublicCapabilityJobState::Queued => ProofJobState::Queued,
        PublicCapabilityJobState::Executing => ProofJobState::Executing,
        PublicCapabilityJobState::Proving => ProofJobState::Proving,
        PublicCapabilityJobState::Succeeded => ProofJobState::Succeeded,
        PublicCapabilityJobState::Failed => ProofJobState::Failed,
        PublicCapabilityJobState::Canceled => ProofJobState::Canceled,
    };
    let public_error_code = match retained.failure() {
        None => PublicErrorCode::Unspecified,
        Some(PublicFailureClass::InvalidRequest) => PublicErrorCode::InvalidRequest,
        Some(PublicFailureClass::ServiceUnavailable) => PublicErrorCode::ServiceUnavailable,
        Some(PublicFailureClass::Canceled) => {
            return Err(Status::internal("capability service failed"));
        }
    };
    Ok(ProofJob {
        job_id: retained.job_id().to_owned(),
        state: state as i32,
        submitted_at: Some(timestamp_from_unix_millis(
            retained.submitted_at_unix_millis(),
        )?),
        updated_at: Some(timestamp_from_unix_millis(
            retained.updated_at_unix_millis(),
        )?),
        capability: retained.capability().cloned(),
        public_error_code: public_error_code as i32,
        internal_error_ref: None,
        metrics: retained.metrics().cloned(),
    })
}

fn timestamp_from_unix_millis(unix_millis: u64) -> Result<Timestamp, Status> {
    let seconds = i64::try_from(unix_millis / 1_000)
        .map_err(|_| Status::internal("capability service failed"))?;
    if seconds > MAX_PROTOBUF_TIMESTAMP_SECONDS {
        return Err(Status::internal("capability service failed"));
    }
    let nanos = i32::try_from((unix_millis % 1_000) * 1_000_000)
        .map_err(|_| Status::internal("capability service failed"))?;
    Ok(Timestamp { seconds, nanos })
}

fn map_publication_error(error: CapabilityPublicationError) -> Status {
    match error {
        CapabilityPublicationError::PrivateEnvelope(_)
        | CapabilityPublicationError::QueueUnavailable
        | CapabilityPublicationError::QueuePublicationUncertain
        | CapabilityPublicationError::EnvelopeCleanupFailed => unavailable_status(),
        CapabilityPublicationError::QueuePublicationRejected
        | CapabilityPublicationError::QueueRecordMissing
        | CapabilityPublicationError::QueueRecordMismatch
        | CapabilityPublicationError::QueueTerminalResultUnavailable => {
            Status::internal("capability service failed")
        }
    }
}

fn map_lifecycle_error(error: CapabilityLifecycleError) -> Status {
    match error {
        CapabilityLifecycleError::InvalidJobId => {
            Status::invalid_argument("invalid capability job")
        }
        CapabilityLifecycleError::NotFound => Status::not_found("capability job not found"),
        CapabilityLifecycleError::QueueUnavailable
        | CapabilityLifecycleError::CancellationPublicationUncertain => unavailable_status(),
        CapabilityLifecycleError::QueueRecordMismatch
        | CapabilityLifecycleError::CancellationPublicationRejected => {
            Status::internal("capability service failed")
        }
    }
}

fn unavailable_status() -> Status {
    Status::unavailable("capability service unavailable")
}
