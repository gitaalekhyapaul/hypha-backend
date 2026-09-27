//! Owner-controlled routing records. These are not authorization grants or proof inputs.

mod store;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::policy_package::ApprovedPolicyPackage;

pub use store::DatasetBindingStore;

const MAX_RECORD_BYTES: u64 = 4096;
const MAX_DATASET_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingError {
    Invalid,
    Conflict,
    NotFound,
    Unavailable,
}

/// Values must come from the owner's finalized dataset service, not an agent payload.
pub struct FinalizedDatasetRevision {
    pub dataset_id: String,
    pub revision: u64,
    pub checksum: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetBinding {
    pub format_version: u32,
    pub policy_hash: String,
    pub resource_type: String,
    pub resource_scope: String,
    pub dataset_id: String,
    pub dataset_revision: String,
    pub dataset_checksum: String,
    pub action: String,
}

impl DatasetBinding {
    pub fn for_approved_policy(
        policy: &ApprovedPolicyPackage,
        resource_scope: String,
        dataset: FinalizedDatasetRevision,
    ) -> Result<Self, BindingError> {
        let binding = Self {
            format_version: 1,
            policy_hash: crate::io::lowercase_hex(policy.policy_hash()),
            resource_type: policy.identity().resource_type(),
            resource_scope,
            dataset_id: dataset.dataset_id,
            dataset_revision: dataset.revision.to_string(),
            dataset_checksum: crate::io::lowercase_hex(&dataset.checksum),
            action: "query".into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    fn validate(&self) -> Result<(), BindingError> {
        validate_key(&self.policy_hash, &self.resource_type, &self.resource_scope)?;
        let revision = self
            .dataset_revision
            .parse::<u64>()
            .map_err(|_| BindingError::Invalid)?;
        if self.format_version != 1
            || self.action != "query"
            || !identifier(&self.dataset_id)
            || !digest(&self.dataset_checksum)
            || revision == 0
            || revision > MAX_DATASET_REVISION
            || self.dataset_revision != revision.to_string()
        {
            return Err(BindingError::Invalid);
        }
        Ok(())
    }
}

fn validate_key(hash: &str, resource_type: &str, scope: &str) -> Result<(), BindingError> {
    if !digest(hash)
        || !identifier(scope)
        || resource_type.is_empty()
        || resource_type.len() > 63
        || !resource_type.starts_with(|c: char| c.is_ascii_alphabetic())
        || !resource_type
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(BindingError::Invalid);
    }
    Ok(())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
