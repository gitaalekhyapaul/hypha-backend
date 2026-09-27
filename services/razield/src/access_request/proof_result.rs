use prost::Message;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_proto::raziel::v1::{
    AccessRequestState as State, ProofJob, ProofJobState, ProofSystem, PublicErrorCode,
};

use super::dispatch_model::hex;
use super::model::{AccessError, MAX_RECORD_BYTES, Record};

pub(super) const MAX_RESULT_BYTES: usize =
    crate::service::MAX_SERVICE_MESSAGE_BYTES - MAX_RECORD_BYTES;

/// Delivery validation only. The existing proof supervisor validates SP1, and the
/// independent gateway/verifier still decides whether a protected query is allowed.
pub(super) fn validate(record: &Record, job: &ProofJob) -> Result<State, AccessError> {
    let attempt = record
        .attempt
        .as_ref()
        .ok_or(AccessError::ServiceUnavailable)?;
    let proposal = record
        .proposal
        .as_ref()
        .ok_or(AccessError::ServiceUnavailable)?;
    let binding = record
        .policy_binding
        .as_ref()
        .ok_or(AccessError::ServiceUnavailable)?;
    let submitted = timestamp(job.submitted_at.as_ref())?;
    let updated = timestamp(job.updated_at.as_ref())?;
    if job.encoded_len() > MAX_RESULT_BYTES
        || job.job_id != attempt.job_id
        || job.internal_error_ref.is_some()
        || submitted.0 < attempt.accepted_at
        || updated < submitted
    {
        return Err(AccessError::ServiceUnavailable);
    }
    let state = ProofJobState::try_from(job.state).map_err(|_| AccessError::ServiceUnavailable)?;
    let error = PublicErrorCode::try_from(job.public_error_code)
        .map_err(|_| AccessError::ServiceUnavailable)?;
    if state != ProofJobState::Failed && error != PublicErrorCode::Unspecified {
        return Err(AccessError::ServiceUnavailable);
    }
    if state != ProofJobState::Succeeded && (job.capability.is_some() || job.metrics.is_some()) {
        return Err(AccessError::ServiceUnavailable);
    }
    Ok(match state {
        ProofJobState::Unspecified => return Err(AccessError::ServiceUnavailable),
        ProofJobState::Queued => State::Queued,
        ProofJobState::Executing => State::Executing,
        ProofJobState::Proving => State::Proving,
        ProofJobState::Canceled => State::Canceled,
        ProofJobState::Failed => {
            if !matches!(
                error,
                PublicErrorCode::InvalidRequest | PublicErrorCode::ServiceUnavailable
            ) {
                return Err(AccessError::ServiceUnavailable);
            }
            State::Failed
        }
        ProofJobState::Succeeded => {
            let capability = job
                .capability
                .as_ref()
                .ok_or(AccessError::ServiceUnavailable)?;
            let metrics = job
                .metrics
                .as_ref()
                .ok_or(AccessError::ServiceUnavailable)?;
            let snapshot = capability
                .snapshot_attestation
                .as_ref()
                .ok_or(AccessError::ServiceUnavailable)?;
            let key: [u8; 32] = proposal
                .agent_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AccessError::ServiceUnavailable)?;
            let issued = timestamp(capability.issued_at.as_ref())?;
            let expiry = timestamp(capability.expires_at.as_ref())?;
            if capability.format_version != 1
                || capability.capability_id != job.job_id
                || hex(&capability.capability_request_commitment) != job.job_id
                || capability.agent_key_thumbprint != derive_ed25519_key_thumbprint(&key)
                || capability.task_id != record.task_id
                || capability.purpose != record.purpose
                || capability.action != record.action
                || capability.resource_type != binding.resource_type
                || capability.resource_scope != binding.resource_scope
                || capability.policy_hash != binding.policy_hash
                || capability.snapshot_id != proposal.snapshot_id
                || capability.relationship_snapshot_root != proposal.snapshot_root
                || snapshot.snapshot_id != proposal.snapshot_id
                || snapshot.relationship_snapshot_root != proposal.snapshot_root
                || snapshot.policy_hash != binding.policy_hash
                || snapshot.policy_record_id != binding.policy_record_id
                || capability.proof_system != ProofSystem::Sp1 as i32
                || capability.zk_proof.is_empty()
                || capability.verification_key_id.is_empty()
                || capability.issuance_nullifier.len() != 32
                || issued.1 != 0
                || expiry.1 != 0
                || issued.0 < attempt.accepted_at
                || expiry <= issued
                || expiry.0 > proposal.requested_expiry
                || timestamp(snapshot.expires_at.as_ref())? < expiry
                || metrics.proof_size_bytes != capability.zk_proof.len() as u64
            {
                return Err(AccessError::ServiceUnavailable);
            }
            for duration in [&metrics.execution_duration, &metrics.proving_duration] {
                let duration = duration.as_ref().ok_or(AccessError::ServiceUnavailable)?;
                if duration.seconds < 0 || !(0..1_000_000_000).contains(&duration.nanos) {
                    return Err(AccessError::ServiceUnavailable);
                }
            }
            State::Ready
        }
    })
}

fn timestamp(value: Option<&prost_types::Timestamp>) -> Result<(i64, i32), AccessError> {
    let value = value.ok_or(AccessError::ServiceUnavailable)?;
    if !(0..=crate::admission::MAX_PROTOBUF_TIMESTAMP_SECONDS).contains(&value.seconds)
        || !(0..1_000_000_000).contains(&value.nanos)
    {
        return Err(AccessError::ServiceUnavailable);
    }
    Ok((value.seconds, value.nanos))
}
