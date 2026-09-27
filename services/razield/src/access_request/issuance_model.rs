use prost::Message;
use raziel_proto::raziel::v1::{
    AccessIssuanceSubmission, AccessRequestState as State, CreateCapabilityRequest,
};
use serde::{Deserialize, Serialize};

use super::model::{AccessError, NormalizedIntent, Record, valid_hex};
use crate::admission::admit_create_capability_request;

pub(super) const MAX_ATTEMPT_BYTES: usize = 512;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicAttempt {
    pub job_id: String,
    pub accepted_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_started_at: Option<i64>,
    #[serde(default)]
    pub proof_state: i32,
    #[serde(default)]
    pub submission_count: u64,
}

impl PublicAttempt {
    pub fn validate(&self, record: &Record) -> Result<(), AccessError> {
        let proposal = record
            .proposal
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        if !valid_hex(&self.job_id, 64)
            || self.accepted_at < proposal.created
            || self.accepted_at >= proposal.signature_deadline
            || self.accepted_at > record.updated
            || self
                .submission_started_at
                .is_some_and(|time| time < self.accepted_at || time > record.updated)
            || raziel_proto::raziel::v1::ProofJobState::try_from(self.proof_state).is_err()
            || (self.submission_started_at.is_some() && self.submission_count == 0)
            || !matches!(
                record.state()?,
                State::Submitting
                    | State::Queued
                    | State::Executing
                    | State::Proving
                    | State::Cancelling
                    | State::Ready
                    | State::Failed
                    | State::Canceled
            )
        {
            return Err(AccessError::ServiceUnavailable);
        }
        use raziel_proto::raziel::v1::ProofJobState as Proof;
        let expected = match record.state()? {
            State::Submitting => Some(Proof::Unspecified),
            State::Queued => Some(Proof::Queued),
            State::Executing => Some(Proof::Executing),
            State::Proving => Some(Proof::Proving),
            State::Ready => Some(Proof::Succeeded),
            State::Failed => Some(Proof::Failed),
            State::Canceled => Some(Proof::Canceled),
            State::Cancelling if self.proof_state <= Proof::Proving as i32 => None,
            _ => return Err(AccessError::ServiceUnavailable),
        };
        if expected.is_some_and(|state| self.proof_state != state as i32)
            || (matches!(
                record.state()?,
                State::Queued | State::Executing | State::Proving | State::Ready
            ) && self.submission_started_at.is_none())
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }
}

// All other request fields are retained immutably in the original intent/proposal.
// Reconstructing them avoids a second private context copy and preserves exact retries.
#[derive(Clone, PartialEq, Message)]
pub(super) struct StoredAttempt {
    #[prost(uint32, tag = "1")]
    pub version: u32,
    #[prost(string, tag = "2")]
    pub proposal_id: String,
    #[prost(uint64, tag = "3")]
    pub generation: u64,
    #[prost(int64, tag = "4")]
    pub accepted_at: i64,
    #[prost(bytes = "vec", tag = "5")]
    pub nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub signature: Vec<u8>,
    #[prost(string, tag = "7")]
    pub job_id: String,
}

impl StoredAttempt {
    pub fn accept(
        submission: &AccessIssuanceSubmission,
        record: &Record,
        original: &NormalizedIntent,
        now: i64,
    ) -> Result<Self, AccessError> {
        let request = submission
            .request
            .as_ref()
            .ok_or(AccessError::InvalidRequest)?;
        let mut attempt = Self {
            version: submission.format_version,
            proposal_id: submission.proposal_id.clone(),
            generation: submission.generation,
            accepted_at: now,
            nonce: request.issuance_nonce.clone(),
            signature: request.request_signature.clone(),
            job_id: String::new(),
        };
        if attempt.submission(record, original)? != *submission {
            return Err(AccessError::Conflict);
        }
        attempt.job_id = admit_create_capability_request(request.clone(), now)
            .map_err(|_| AccessError::InvalidRequest)?
            .job_id()
            .to_owned();
        attempt.validate(record, original)?;
        Ok(attempt)
    }

    pub fn public(&self) -> PublicAttempt {
        PublicAttempt {
            job_id: self.job_id.clone(),
            accepted_at: self.accepted_at,
            submission_started_at: None,
            proof_state: 0,
            submission_count: 0,
        }
    }

    pub fn matches_public(&self, public: &PublicAttempt) -> bool {
        self.job_id == public.job_id && self.accepted_at == public.accepted_at
    }

    pub fn submission(
        &self,
        record: &Record,
        original: &NormalizedIntent,
    ) -> Result<AccessIssuanceSubmission, AccessError> {
        let proposal = record
            .proposal
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        let binding = record
            .policy_binding
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        let owner = original
            .0
            .owner
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        let intent = original
            .0
            .intent
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        Ok(AccessIssuanceSubmission {
            format_version: 1,
            proposal_id: proposal.proposal_id.clone(),
            generation: proposal.generation,
            request: Some(CreateCapabilityRequest {
                format_version: 1,
                agent_id: record.agent_id.clone(),
                agent_public_key: owner.agent_public_key.clone(),
                task_id: record.task_id.clone(),
                purpose: record.purpose.clone(),
                action: record.action.clone(),
                resource_type: binding.resource_type.clone(),
                resource_scope: binding.resource_scope.clone(),
                requested_expires_at: Some(prost_types::Timestamp {
                    seconds: proposal.requested_expiry,
                    nanos: 0,
                }),
                issuance_nonce: self.nonce.clone(),
                snapshot_id: proposal.snapshot_id.clone(),
                request_signature: self.signature.clone(),
                request_context: intent.request_context.clone(),
            }),
        })
    }

    pub fn validate(
        &self,
        record: &Record,
        original: &NormalizedIntent,
    ) -> Result<(), AccessError> {
        let proposal = record
            .proposal
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        if self.version != 1
            || self.proposal_id != proposal.proposal_id
            || self.generation != proposal.generation
            || self.accepted_at < proposal.created
            || self.accepted_at >= proposal.signature_deadline
            || self.encoded_len() > MAX_ATTEMPT_BYTES
            || !valid_hex(&self.job_id, 64)
        {
            return Err(AccessError::ServiceUnavailable);
        }
        let request = self
            .submission(record, original)?
            .request
            .ok_or(AccessError::ServiceUnavailable)?;
        // Replay validation uses the persisted acceptance instant, not a fresh validity window.
        let admitted = admit_create_capability_request(request, self.accepted_at)
            .map_err(|_| AccessError::ServiceUnavailable)?;
        if admitted.job_id() != self.job_id {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }
}
