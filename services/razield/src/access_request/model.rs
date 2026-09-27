use std::fmt;

use prost::Message;
use raziel_authority::validate_agent_api_key;
use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_proto::raziel::v1::{
    AccessRequest, AccessRequestState as State, CreateAccessRequestRequest,
    authorization_context_entry::Value,
};
use serde::{Deserialize, Serialize};

use super::approval::PublicBinding;
use super::dispatch_model::PublicProposal;
use super::issuance_model::PublicAttempt;

pub const MAX_INTENT_BYTES: usize = 16 * 1024;
pub const REVIEW_SECONDS: i64 = 24 * 60 * 60;
pub const RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;
pub(super) const RESERVATION_SECONDS: i64 = 10 * 60;
pub(super) const MAX_RECORD_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessError {
    InvalidRequest,
    NotFound,
    Conflict,
    Expired,
    CapacityExceeded,
    ServiceUnavailable,
    Configuration,
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::Expired => "EXPIRED",
            Self::CapacityExceeded => "CAPACITY_EXCEEDED",
            Self::ServiceUnavailable | Self::Configuration => "SERVICE_UNAVAILABLE",
        })
    }
}
impl std::error::Error for AccessError {}

impl From<AccessError> for tonic::Status {
    fn from(error: AccessError) -> Self {
        let code = match error {
            AccessError::InvalidRequest => tonic::Code::InvalidArgument,
            AccessError::NotFound => tonic::Code::NotFound,
            AccessError::Conflict => tonic::Code::Aborted,
            AccessError::Expired => tonic::Code::OutOfRange,
            AccessError::CapacityExceeded => tonic::Code::ResourceExhausted,
            AccessError::ServiceUnavailable | AccessError::Configuration => {
                tonic::Code::Unavailable
            }
        };
        Self::new(code, error.to_string())
    }
}

// Deliberately no Debug or Serialize implementation for private input.
#[derive(Clone, PartialEq)]
pub(super) struct NormalizedIntent(pub CreateAccessRequestRequest);

impl NormalizedIntent {
    pub fn parse(mut input: CreateAccessRequestRequest) -> Result<Self, AccessError> {
        if input.encoded_len() > MAX_INTENT_BYTES {
            return Err(AccessError::InvalidRequest);
        }
        let owner = input.owner.as_ref().ok_or(AccessError::InvalidRequest)?;
        validate_agent_id(&owner.agent_id)?;
        validate_agent_api_key(&owner.agent_public_key).map_err(|_| AccessError::InvalidRequest)?;
        let intent = input.intent.as_mut().ok_or(AccessError::InvalidRequest)?;
        intent.request_id = normalize_request_id(&intent.request_id)?;
        let max_string = TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize;
        let text = |value: &str| {
            !value.trim().is_empty()
                && value.len() <= max_string
                && !value.chars().any(char::is_control)
        };
        if intent.format_version != 1
            || !text(&intent.task_id)
            || !text(&intent.purpose)
            || intent.action != "query"
            || !valid_dataset_id(&intent.dataset_id)
            || intent.dataset_revision == 0
            || intent.dataset_revision > 9_007_199_254_740_991
            || input.dataset.as_ref().is_some_and(|dataset| {
                dataset.dataset_id != intent.dataset_id
                    || dataset.revision != intent.dataset_revision
                    || dataset.checksum.len() != 32
            })
            || intent.request_context.len() > 16
        {
            return Err(AccessError::InvalidRequest);
        }
        intent.request_context.sort_by(|a, b| a.name.cmp(&b.name));
        if intent
            .request_context
            .windows(2)
            .any(|p| p[0].name == p[1].name)
        {
            return Err(AccessError::InvalidRequest);
        }
        for entry in &intent.request_context {
            if !valid_identifier(&entry.name) || entry.name.len() > max_string {
                return Err(AccessError::InvalidRequest);
            }
            match &entry.value {
                Some(Value::BoolValue(_) | Value::IntValue(_)) => {}
                Some(Value::StringValue(value)) if value.len() <= max_string => {}
                _ => return Err(AccessError::InvalidRequest),
            }
        }
        Ok(Self(input))
    }

    pub fn identity(&self) -> String {
        format!(
            "{}:{}",
            self.0.owner.as_ref().unwrap().agent_id,
            self.0.intent.as_ref().unwrap().request_id
        )
    }

    pub fn matches_retry(&self, retry: &Self) -> bool {
        self.0.owner == retry.0.owner
            && self.0.intent == retry.0.intent
            && retry
                .0
                .dataset
                .as_ref()
                .is_none_or(|dataset| Some(dataset) == self.0.dataset.as_ref())
    }
}

pub(super) fn validate_agent_id(value: &str) -> Result<(), AccessError> {
    if valid_hex(value, 32) {
        Ok(())
    } else {
        Err(AccessError::InvalidRequest)
    }
}

pub(super) fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) fn normalize_request_id(value: &str) -> Result<String, AccessError> {
    let value = value.to_ascii_lowercase();
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || bytes[14] != b'4'
        || !matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
        || !bytes.iter().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(b)
            }
        })
    {
        return Err(AccessError::InvalidRequest);
    }
    Ok(value)
}

pub(super) fn identity(agent: &str, request: &str) -> Result<String, AccessError> {
    validate_agent_id(agent)?;
    Ok(format!("{agent}:{}", normalize_request_id(request)?))
}

pub(super) fn valid_identity(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(agent, request)| identity(agent, request).is_ok_and(|id| id == value))
}

fn valid_identifier(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub(super) fn valid_dataset_id(value: &str) -> bool {
    value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Reserved,
    Published,
    Deleting,
}

/// An explicit allowlist, separate from the private normalized transport.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub format_version: u32,
    pub agent_id: String,
    pub request_id: String,
    pub task_id: String,
    pub purpose: String,
    pub action: String,
    pub dataset_id: String,
    pub dataset_revision: u64,
    pub state: i32,
    pub generation: u64,
    pub created: i64,
    pub updated: i64,
    pub deadline: i64,
    pub due: i64,
    pub phase: Phase,
    pub private_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_binding: Option<PublicBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<PublicProposal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumed_from_generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<PublicAttempt>,
}

impl Record {
    pub fn reserved(input: &NormalizedIntent, now: i64) -> Result<Self, AccessError> {
        if !(0..=253_402_300_799 - REVIEW_SECONDS - RETENTION_SECONDS).contains(&now) {
            return Err(AccessError::ServiceUnavailable);
        }
        let owner = input.0.owner.as_ref().unwrap();
        let intent = input.0.intent.as_ref().unwrap();
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| AccessError::ServiceUnavailable)?;
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            format_version: 1,
            agent_id: owner.agent_id.clone(),
            request_id: intent.request_id.clone(),
            task_id: intent.task_id.clone(),
            purpose: intent.purpose.clone(),
            action: intent.action.clone(),
            dataset_id: intent.dataset_id.clone(),
            dataset_revision: intent.dataset_revision,
            state: State::PendingReview as i32,
            generation: 1,
            created: now,
            updated: now,
            deadline: now + REVIEW_SECONDS,
            due: now + RESERVATION_SECONDS,
            phase: Phase::Reserved,
            private_ref: format!("{}.{suffix}", input.identity()),
            policy_binding: None,
            dispatch_generation: None,
            proposal: None,
            resumed_from_generation: None,
            attempt: None,
        })
    }

    pub fn identity(&self) -> String {
        format!("{}:{}", self.agent_id, self.request_id)
    }

    pub fn matches_intent(&self, input: &NormalizedIntent) -> bool {
        let intent = input.0.intent.as_ref().unwrap();
        self.identity() == input.identity()
            && self.task_id == intent.task_id
            && self.purpose == intent.purpose
            && self.action == intent.action
            && self.dataset_id == intent.dataset_id
            && self.dataset_revision == intent.dataset_revision
    }

    pub fn state(&self) -> Result<State, AccessError> {
        State::try_from(self.state)
            .ok()
            .filter(|s| *s != State::Unspecified)
            .ok_or(AccessError::ServiceUnavailable)
    }

    pub fn terminal(&self) -> Result<bool, AccessError> {
        Ok(matches!(
            self.state()?,
            State::Ready | State::Rejected | State::Canceled | State::Expired | State::Failed
        ))
    }

    pub fn predispatch(&self) -> Result<bool, AccessError> {
        Ok(matches!(
            self.state()?,
            State::PendingReview | State::ApprovedWaiting | State::WaitingForAgent
        ))
    }

    pub fn public(&self) -> AccessRequest {
        AccessRequest {
            format_version: 1,
            agent_id: self.agent_id.clone(),
            request_id: self.request_id.clone(),
            task_id: self.task_id.clone(),
            purpose: self.purpose.clone(),
            action: self.action.clone(),
            dataset_id: self.dataset_id.clone(),
            dataset_revision: self.dataset_revision,
            state: self.state,
            generation: self.generation,
            created_at_unix_seconds: self.created,
            updated_at_unix_seconds: self.updated,
            deadline_unix_seconds: self.deadline,
            policy_binding: self.policy_binding.as_ref().map(PublicBinding::public),
            proposal: self.proposal.as_ref().map(|proposal| proposal.public(self)),
            proof_job_id: self.attempt.as_ref().map(|attempt| attempt.job_id.clone()),
            proof_job_state: self
                .attempt
                .as_ref()
                .map_or(0, |attempt| attempt.proof_state),
            ..Default::default()
        }
    }

    pub fn finish(&mut self, state: State, now: i64) -> Result<(), AccessError> {
        if self.terminal()? || now < self.updated {
            return Err(AccessError::Conflict);
        }
        self.state = state as i32;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(AccessError::ServiceUnavailable)?;
        self.updated = now;
        self.due = now
            .checked_add(RETENTION_SECONDS)
            .ok_or(AccessError::ServiceUnavailable)?;
        Ok(())
    }

    pub fn encode(&self) -> Result<String, AccessError> {
        let bytes = serde_json::to_string(self).map_err(|_| AccessError::ServiceUnavailable)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(bytes)
    }

    pub fn decode(id: &str, bytes: &str) -> Result<Self, AccessError> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(AccessError::ServiceUnavailable);
        }
        let record: Self =
            serde_json::from_str(bytes).map_err(|_| AccessError::ServiceUnavailable)?;
        if record.format_version != 1
            || !valid_identity(id)
            || record.identity() != id
            || !record
                .private_ref
                .strip_prefix(&format!("{id}."))
                .is_some_and(|s| valid_hex(s, 32))
            || record.generation == 0
            || record.created < 0
            || record.updated < record.created
            || record.deadline.checked_sub(record.created) != Some(REVIEW_SECONDS)
            || record.due < record.created
        {
            return Err(AccessError::ServiceUnavailable);
        }
        record.state()?;
        if record
            .resumed_from_generation
            .is_some_and(|value| value == 0 || value >= record.generation)
        {
            return Err(AccessError::ServiceUnavailable);
        }
        if let Some(binding) = &record.policy_binding {
            binding.validate()?;
            if record.state()? == State::PendingReview || record.phase == Phase::Reserved {
                return Err(AccessError::ServiceUnavailable);
            }
        }
        if matches!(
            record.state()?,
            State::ApprovedWaiting
                | State::WaitingForAgent
                | State::Preparing
                | State::AwaitingSignature
                | State::Cancelling
        ) && record.policy_binding.is_none()
        {
            return Err(AccessError::ServiceUnavailable);
        }
        if record
            .dispatch_generation
            .is_some_and(|generation| generation == 0 || generation >= record.generation)
            || (matches!(
                record.state()?,
                State::Preparing | State::AwaitingSignature | State::Cancelling
            ) && record.dispatch_generation.is_none())
        {
            return Err(AccessError::ServiceUnavailable);
        }
        if let Some(proposal) = &record.proposal {
            proposal.validate(&record)?;
        } else if record.state()? == State::AwaitingSignature {
            return Err(AccessError::ServiceUnavailable);
        }
        if let Some(attempt) = &record.attempt {
            attempt.validate(&record)?;
        } else if matches!(
            record.state()?,
            State::Submitting | State::Queued | State::Executing | State::Proving | State::Ready
        ) {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(record)
    }
}
