use std::time::Duration;

use raziel_proto::raziel::v1::{
    CancelCapabilityRequest, CreateCapabilityRequest, GetCapabilityRequest, ProofJob,
    capability_service_client::CapabilityServiceClient,
};
use tonic::transport::{Channel, Endpoint};

use super::processor::Processor;
use crate::access_request::{AccessError, DispatchReference, PendingProof};
use crate::admission::admit_create_capability_request;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProofServiceError {
    NotFound,
    Rejected,
    Capacity,
    Unavailable,
}

#[tonic::async_trait]
pub(crate) trait ProofService: Send + Sync {
    async fn get(&self, job_id: &str) -> Result<ProofJob, ProofServiceError>;
    async fn create(&self, request: CreateCapabilityRequest)
    -> Result<ProofJob, ProofServiceError>;
    async fn cancel(&self, job_id: &str) -> Result<ProofJob, ProofServiceError>;
}

pub(crate) struct RpcProofService(CapabilityServiceClient<Channel>);

impl RpcProofService {
    pub fn new(address: String) -> Result<Self, AccessError> {
        if !(address.starts_with("http://") || address.starts_with("https://")) {
            return Err(AccessError::Configuration);
        }
        let endpoint = Endpoint::from_shared(address)
            .map_err(|_| AccessError::Configuration)?
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(7));
        Ok(Self(
            CapabilityServiceClient::new(endpoint.connect_lazy())
                .max_decoding_message_size(crate::service::MAX_SERVICE_MESSAGE_BYTES)
                .max_encoding_message_size(crate::access_request::MAX_INTENT_BYTES),
        ))
    }
}

#[tonic::async_trait]
impl ProofService for RpcProofService {
    async fn get(&self, job_id: &str) -> Result<ProofJob, ProofServiceError> {
        self.0
            .clone()
            .get_capability(GetCapabilityRequest {
                job_id: job_id.to_owned(),
            })
            .await
            .map_err(map_status)?
            .into_inner()
            .job
            .ok_or(ProofServiceError::Unavailable)
    }
    async fn create(
        &self,
        request: CreateCapabilityRequest,
    ) -> Result<ProofJob, ProofServiceError> {
        self.0
            .clone()
            .create_capability(request)
            .await
            .map_err(map_status)?
            .into_inner()
            .job
            .ok_or(ProofServiceError::Unavailable)
    }
    async fn cancel(&self, job_id: &str) -> Result<ProofJob, ProofServiceError> {
        self.0
            .clone()
            .cancel_capability(CancelCapabilityRequest {
                job_id: job_id.to_owned(),
            })
            .await
            .map_err(map_status)?
            .into_inner()
            .job
            .ok_or(ProofServiceError::Unavailable)
    }
}

impl Processor {
    pub(crate) async fn proof_step(
        &self,
        reference: &DispatchReference,
    ) -> Result<bool, AccessError> {
        let selected = reference.clone();
        let attempt = self
            .owner(move |owner, now| owner.pending_proof(&selected, now))
            .await?;
        if attempt.cancelling && !attempt.started {
            let selected = reference.clone();
            self.owner(move |owner, now| owner.finish_unsubmitted(&selected, now))
                .await?;
            return Ok(true);
        }
        // Every recovery path reads first. Unknown state never becomes a new nonce.
        let result = match self.proof_service.get(&attempt.job_id).await {
            Ok(job) => job,
            Err(ProofServiceError::NotFound) if !attempt.cancelling && !attempt.observed => {
                return self.submit_proof(reference, attempt).await;
            }
            Err(_) => return Err(AccessError::ServiceUnavailable),
        };
        let selected = reference.clone();
        self.owner(move |owner, now| owner.record_proof_job(&selected, result, now))
            .await?;
        // Re-read durable cancellation after Get, including a cancel concurrent with that RPC.
        let selected = reference.clone();
        let current = self
            .owner(move |owner, now| owner.dispatch_request(&selected, now))
            .await?;
        if current.is_some_and(|record| {
            record.state == raziel_proto::raziel::v1::AccessRequestState::Cancelling as i32
        }) {
            let result = self
                .proof_service
                .cancel(&attempt.job_id)
                .await
                .map_err(|_| AccessError::ServiceUnavailable)?;
            let selected = reference.clone();
            self.owner(move |owner, now| owner.record_proof_job(&selected, result, now))
                .await?;
        }
        Ok(false)
    }

    async fn submit_proof(
        &self,
        reference: &DispatchReference,
        attempt: PendingProof,
    ) -> Result<bool, AccessError> {
        let now = self
            .clock
            .now_unix_seconds()
            .map_err(|_| AccessError::ServiceUnavailable)?;
        if admit_create_capability_request(attempt.request.clone(), now).is_err() {
            if attempt.started {
                return Err(AccessError::ServiceUnavailable);
            }
            let selected = reference.clone();
            self.owner(move |owner, now| owner.finish_unsubmitted(&selected, now))
                .await?;
            return Ok(true);
        }
        let selected = reference.clone();
        let start = self
            .owner(move |owner, now| owner.begin_submission(&selected, now))
            .await?;
        match self.proof_service.create(attempt.request).await {
            Ok(result) => {
                let selected = reference.clone();
                self.owner(move |owner, now| owner.record_proof_job(&selected, result, now))
                    .await?;
            }
            Err(error @ (ProofServiceError::Rejected | ProofServiceError::Capacity))
                if start.first =>
            {
                let selected = reference.clone();
                self.owner(move |owner, now| {
                    owner.reject_first_submission(&selected, start.sequence, now)
                })
                .await?;
                if error == ProofServiceError::Rejected {
                    let selected = reference.clone();
                    self.owner(move |owner, now| owner.finish_unsubmitted(&selected, now))
                        .await?;
                    return Ok(true);
                }
            }
            Err(_) => return Err(AccessError::ServiceUnavailable),
        }
        Ok(false)
    }
}

fn map_status(status: tonic::Status) -> ProofServiceError {
    match status.code() {
        tonic::Code::NotFound => ProofServiceError::NotFound,
        tonic::Code::InvalidArgument => ProofServiceError::Rejected,
        tonic::Code::ResourceExhausted => ProofServiceError::Capacity,
        _ => ProofServiceError::Unavailable,
    }
}
