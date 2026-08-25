use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use raziel_proto::raziel::v1::verification_service_server::VerificationService;
use raziel_proto::raziel::v1::{
    PublicErrorCode, VerificationOutcome, VerifyCapabilityRequest, VerifyCapabilityResponse,
};
use raziel_prover::TrustedProgramKey;
use raziel_verifier::{LocalRegistry, RegistryConfig, RegistryLoader};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::admission::{
    AdmittedVerificationRequest, LocalVerificationTrust, VerificationAdmissionError,
    admit_verify_capability_request,
};
use crate::worker::{
    IsolatedVerificationRequest, VerificationExecutionError, VerificationExecutor,
};

pub struct ReloadingAdmission {
    loader: RegistryLoader,
    registry_path: PathBuf,
    trust: LocalVerificationTrust,
}

impl ReloadingAdmission {
    #[must_use]
    pub fn new(
        registry_path: PathBuf,
        registry_config: RegistryConfig,
        trusted_program_key: TrustedProgramKey,
    ) -> Self {
        Self {
            loader: RegistryLoader::new(registry_config.clone()),
            registry_path,
            trust: LocalVerificationTrust::new(
                LocalRegistry::new(registry_config),
                trusted_program_key,
            ),
        }
    }

    pub fn admit(
        &mut self,
        request: VerifyCapabilityRequest,
        now_unix_seconds: i64,
    ) -> Result<AdmittedVerificationRequest, VerificationAdmissionError> {
        let generation = self
            .loader
            .load_current(&self.registry_path, now_unix_seconds)?;
        self.trust.install_generation(generation)?;
        admit_verify_capability_request(request, now_unix_seconds, &mut self.trust)
    }

    pub fn load_at_startup(
        &mut self,
        now_unix_seconds: i64,
    ) -> Result<(), VerificationAdmissionError> {
        let generation = self
            .loader
            .load_current(&self.registry_path, now_unix_seconds)?;
        self.trust.install_generation(generation)?;
        Ok(())
    }

    #[must_use]
    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }
}

pub struct VerificationServiceHandler<E> {
    admission: Mutex<ReloadingAdmission>,
    executor: Arc<E>,
}

impl<E> VerificationServiceHandler<E> {
    #[must_use]
    pub fn new(admission: ReloadingAdmission, executor: E) -> Self {
        Self {
            admission: Mutex::new(admission),
            executor: Arc::new(executor),
        }
    }
}

#[tonic::async_trait]
impl<E: VerificationExecutor> VerificationService for VerificationServiceHandler<E> {
    async fn verify_capability(
        &self,
        request: Request<VerifyCapabilityRequest>,
    ) -> Result<Response<VerifyCapabilityResponse>, Status> {
        let now = trusted_now_unix_seconds()?;
        let admitted = match self.admission.lock().await.admit(request.into_inner(), now) {
            Ok(admitted) => admitted,
            Err(error) => return Ok(Response::new(rejected(error.public_error_code()))),
        };
        let worker_request = match IsolatedVerificationRequest::from_admission(
            admitted.worker_request(),
            admitted.proof_digest(),
        ) {
            Ok(request) => request,
            Err(_) => {
                return Ok(Response::new(rejected(PublicErrorCode::ServiceUnavailable)));
            }
        };
        let worker_result = self.executor.verify(worker_request).await;
        match worker_result {
            Ok(success)
                if success.verification_key_id()
                    == admitted
                        .worker_request()
                        .trusted_program_key()
                        .verification_key_id() =>
            {
                Ok(Response::new(promote(admitted)))
            }
            Ok(_) | Err(VerificationExecutionError::MalformedResult) => {
                Ok(Response::new(rejected(PublicErrorCode::ServiceUnavailable)))
            }
            Err(VerificationExecutionError::Rejected) => {
                Ok(Response::new(rejected(PublicErrorCode::CapabilityInvalid)))
            }
            Err(
                VerificationExecutionError::Configuration
                | VerificationExecutionError::Unavailable
                | VerificationExecutionError::TimedOut,
            ) => Ok(Response::new(rejected(PublicErrorCode::ServiceUnavailable))),
        }
    }
}

fn promote(admitted: AdmittedVerificationRequest) -> VerifyCapabilityResponse {
    VerifyCapabilityResponse {
        outcome: VerificationOutcome::Verified as i32,
        verified_claims: Some(admitted.candidate_claims().clone()),
        failure_code: PublicErrorCode::Unspecified as i32,
        policy_record_id: admitted.policy_record_id().to_owned(),
        snapshot_id: admitted.snapshot_id().to_owned(),
        proof_digest: admitted.proof_digest().to_vec(),
    }
}

fn rejected(code: PublicErrorCode) -> VerifyCapabilityResponse {
    VerifyCapabilityResponse {
        outcome: VerificationOutcome::Rejected as i32,
        verified_claims: None,
        failure_code: code as i32,
        policy_record_id: String::new(),
        snapshot_id: String::new(),
        proof_digest: Vec::new(),
    }
}

fn trusted_now_unix_seconds() -> Result<i64, Status> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Status::unavailable("trusted clock unavailable"))?
        .as_secs();
    i64::try_from(seconds).map_err(|_| Status::unavailable("trusted clock unavailable"))
}

#[cfg(test)]
mod tests {
    use raziel_proto::raziel::v1::VerifiedClaims;

    use super::*;
    use crate::admission::VerificationWorkerRequest;

    const KEY_ID: &str =
        "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
    const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn candidate_claims_only_appear_in_promoted_success() {
        let rejected = rejected(PublicErrorCode::CapabilityInvalid);
        assert!(rejected.verified_claims.is_none());
        assert!(rejected.policy_record_id.is_empty());
        assert!(rejected.proof_digest.is_empty());

        let admitted = AdmittedVerificationRequest::new(
            VerificationWorkerRequest::new(
                vec![0xaa],
                vec![0xbb],
                TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap(),
            ),
            VerifiedClaims {
                capability_id: "capability".to_owned(),
                ..VerifiedClaims::default()
            },
            "policy-record".to_owned(),
            "snapshot".to_owned(),
            [0x44; 32],
        );
        let promoted = promote(admitted);
        assert_eq!(promoted.outcome, VerificationOutcome::Verified as i32);
        assert_eq!(
            promoted.verified_claims.unwrap().capability_id,
            "capability"
        );
        assert_eq!(promoted.policy_record_id, "policy-record");
        assert_eq!(promoted.snapshot_id, "snapshot");
        assert_eq!(promoted.proof_digest, [0x44; 32]);
    }
}
