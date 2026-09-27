use raziel_authority::validate_agent_api_key;
use raziel_proto::raziel::v1::{AccessRequestState as State, IssuanceProposal};
use serde::{Deserialize, Serialize};

use super::model::{AccessError, Record, identity, valid_hex};

pub(super) const SIGNATURE_SECONDS: i64 = 120;
pub(super) const ISSUANCE_SECONDS: i64 = 4800;

/// The complete BullMQ payload. Private intent and grant data never enter the queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchReference {
    pub agent_id: String,
    pub request_id: String,
    pub generation: u64,
}

impl DispatchReference {
    pub fn validate(&self) -> Result<(), AccessError> {
        let id = identity(&self.agent_id, &self.request_id)?;
        if id != format!("{}:{}", self.agent_id, self.request_id) || self.generation == 0 {
            return Err(AccessError::InvalidRequest);
        }
        Ok(())
    }

    pub fn job_id(&self) -> String {
        format!("{}-{}-{}", self.agent_id, self.request_id, self.generation)
    }

    pub(super) fn identity(&self) -> String {
        format!("{}:{}", self.agent_id, self.request_id)
    }

    pub(super) fn matches(&self, record: &Record) -> bool {
        record.identity() == self.identity() && record.dispatch_generation == Some(self.generation)
    }
}

/// Only approved public proposal fields belong in the Redis request record.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicProposal {
    pub proposal_id: String,
    pub generation: u64,
    pub created: i64,
    pub signature_deadline: i64,
    pub agent_public_key: Vec<u8>,
    pub snapshot_id: String,
    pub snapshot_root: Vec<u8>,
    pub requested_expiry: i64,
}

impl PublicProposal {
    pub fn validate(&self, record: &Record) -> Result<(), AccessError> {
        if !valid_hex(&self.proposal_id, 32)
            || self.generation == 0
            || self.generation > record.generation
            || self.created < record.created
            || self.created > record.updated
            || self.created.checked_add(SIGNATURE_SECONDS) != Some(self.signature_deadline)
            || self.created.checked_add(ISSUANCE_SECONDS) != Some(self.requested_expiry)
            || self.requested_expiry > record.deadline
            || self.requested_expiry > 253_402_300_799
            || record.policy_binding.is_none()
            || self.snapshot_root.len() != 32
            || self.snapshot_id != format!("snapshot-v1:{}", hex(&self.snapshot_root))
            || validate_agent_api_key(&self.agent_public_key).is_err()
            || record.dispatch_generation.is_none()
            || !matches!(
                record.state()?,
                State::AwaitingSignature
                    | State::Submitting
                    | State::Queued
                    | State::Executing
                    | State::Proving
                    | State::Cancelling
                    | State::Ready
                    | State::Failed
                    | State::Canceled
            )
            || (record.state()? == State::AwaitingSignature && self.generation != record.generation)
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }

    pub fn public(&self, record: &Record) -> IssuanceProposal {
        IssuanceProposal {
            format_version: 1,
            proposal_id: self.proposal_id.clone(),
            generation: self.generation,
            signature_deadline_unix_seconds: self.signature_deadline,
            agent_public_key: self.agent_public_key.clone(),
            task_id: record.task_id.clone(),
            purpose: record.purpose.clone(),
            action: record.action.clone(),
            policy_binding: record
                .policy_binding
                .as_ref()
                .map(super::approval::PublicBinding::public),
            snapshot_id: self.snapshot_id.clone(),
            snapshot_root: self.snapshot_root.clone(),
            requested_expires_at_unix_seconds: self.requested_expiry,
        }
    }
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
