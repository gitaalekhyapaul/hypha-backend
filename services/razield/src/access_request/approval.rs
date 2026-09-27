use prost::Message;
use raziel_admin::request_approval::{ApprovalConfig, ApprovalError, validate_command};
use raziel_proto::raziel::v1::{
    AccessPolicyBinding, ApproveAccessRequestRequest, CreateAccessRequestRequest,
};
use serde::{Deserialize, Serialize};

use super::model::{
    AccessError, MAX_INTENT_BYTES, NormalizedIntent, Record, normalize_request_id,
    valid_dataset_id, valid_hex,
};

pub(super) const MAX_APPROVAL_BYTES: usize = MAX_INTENT_BYTES + 1024;

/// Only the installed-package implementation is constructed by the runtime.
pub(super) trait ApprovalPreparer: Send {
    fn prepare(
        &self,
        command: &ApproveAccessRequestRequest,
        original: &CreateAccessRequestRequest,
        now: i64,
    ) -> Result<AccessPolicyBinding, AccessError>;
}

impl ApprovalPreparer for ApprovalConfig {
    fn prepare(
        &self,
        command: &ApproveAccessRequestRequest,
        original: &CreateAccessRequestRequest,
        now: i64,
    ) -> Result<AccessPolicyBinding, AccessError> {
        ApprovalConfig::prepare(self, command, original, now).map_err(|error| match error {
            ApprovalError::Invalid => AccessError::InvalidRequest,
            ApprovalError::Conflict => AccessError::Conflict,
            ApprovalError::Unavailable => AccessError::ServiceUnavailable,
        })
    }
}

pub(super) fn normalize_command(
    mut command: ApproveAccessRequestRequest,
) -> Result<ApproveAccessRequestRequest, AccessError> {
    if command.encoded_len() > MAX_INTENT_BYTES {
        return Err(AccessError::InvalidRequest);
    }
    command.request_id = normalize_request_id(&command.request_id)?;
    validate_command(&command).map_err(|_| AccessError::InvalidRequest)?;
    if command.expected_generation == u64::MAX {
        return Err(AccessError::InvalidRequest);
    }
    Ok(command)
}

/// Redis stores this allowlist, never the owner command or grant parameters.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicBinding {
    pub policy_record_id: String,
    pub policy_hash: Vec<u8>,
    pub resource_type: String,
    pub resource_scope: String,
}

impl PublicBinding {
    pub fn from_proto(binding: &AccessPolicyBinding) -> Result<Self, AccessError> {
        let value = Self {
            policy_record_id: binding.policy_record_id.clone(),
            policy_hash: binding.policy_hash.clone(),
            resource_type: binding.resource_type.clone(),
            resource_scope: binding.resource_scope.clone(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), AccessError> {
        if !valid_dataset_id(&self.policy_record_id)
            || self.policy_hash.len() != 32
            || !self
                .resource_type
                .strip_prefix("dataset_")
                .is_some_and(|id| valid_hex(id, 32))
            || !valid_dataset_id(&self.resource_scope)
        {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(())
    }

    pub fn public(&self) -> AccessPolicyBinding {
        AccessPolicyBinding {
            policy_record_id: self.policy_record_id.clone(),
            policy_hash: self.policy_hash.clone(),
            resource_type: self.resource_type.clone(),
            resource_scope: self.resource_scope.clone(),
        }
    }
}

// No Debug or Serialize implementation: this contains private grant inputs.
#[derive(Clone, PartialEq)]
pub(super) struct StoredApproval {
    pub command: ApproveAccessRequestRequest,
    pub binding: AccessPolicyBinding,
    pub approved_at: i64,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateApprovalWire {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(message, optional, tag = "2")]
    command: Option<ApproveAccessRequestRequest>,
    #[prost(message, optional, tag = "3")]
    binding: Option<AccessPolicyBinding>,
    #[prost(int64, tag = "4")]
    approved_at: i64,
}

impl StoredApproval {
    pub fn matches_retry(&self, command: &ApproveAccessRequestRequest) -> bool {
        let mut retry = command.clone();
        if retry.dataset.is_none() {
            retry.dataset = self.command.dataset.clone();
        }
        retry == self.command
    }

    pub fn validate(&self, original: &NormalizedIntent) -> Result<(), AccessError> {
        let command =
            normalize_command(self.command.clone()).map_err(|_| AccessError::ServiceUnavailable)?;
        let intent = original
            .0
            .intent
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        let owner = original
            .0
            .owner
            .as_ref()
            .ok_or(AccessError::ServiceUnavailable)?;
        if command != self.command
            || command.agent_id != owner.agent_id
            || command.request_id != intent.request_id
            || command.dataset.is_none()
            || command.dataset != original.0.dataset
            || self.binding.policy_record_id != command.policy_record_id
            || self.binding.resource_type != format!("dataset_{}", command.policy_version_id)
            || self.binding.resource_scope != intent.dataset_id
            || !(0..=253_402_300_799).contains(&self.approved_at)
        {
            return Err(AccessError::ServiceUnavailable);
        }
        PublicBinding::from_proto(&self.binding)?;
        Ok(())
    }

    pub fn matches_published(&self, record: &Record) -> bool {
        record
            .policy_binding
            .as_ref()
            .is_some_and(|binding| binding.public() == self.binding)
            && self.command.expected_generation < record.generation
            && self.approved_at >= record.created
            && self.approved_at <= record.updated
            && self.approved_at < record.deadline
    }

    pub fn encode(&self) -> Result<Vec<u8>, AccessError> {
        let bytes = PrivateApprovalWire {
            version: 1,
            command: Some(self.command.clone()),
            binding: Some(self.binding.clone()),
            approved_at: self.approved_at,
        }
        .encode_to_vec();
        if bytes.len() > MAX_APPROVAL_BYTES {
            return Err(AccessError::ServiceUnavailable);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], original: &NormalizedIntent) -> Result<Self, AccessError> {
        if bytes.len() > MAX_APPROVAL_BYTES {
            return Err(AccessError::ServiceUnavailable);
        }
        let wire =
            PrivateApprovalWire::decode(bytes).map_err(|_| AccessError::ServiceUnavailable)?;
        if wire.version != 1 || wire.encode_to_vec() != bytes {
            return Err(AccessError::ServiceUnavailable);
        }
        let value = Self {
            command: wire.command.ok_or(AccessError::ServiceUnavailable)?,
            binding: wire.binding.ok_or(AccessError::ServiceUnavailable)?,
            approved_at: wire.approved_at,
        };
        value.validate(original)?;
        Ok(value)
    }
}
