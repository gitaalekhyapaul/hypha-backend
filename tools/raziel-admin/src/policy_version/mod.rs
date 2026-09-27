//! Owner-private, durable allocation of policy identities before compilation.
//! Allocations are never reclaimed after a failed or abandoned draft.

mod private;
mod store;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{AdminError, args::Arguments, policy_package::PolicyVersionIdentity};

pub use store::PolicyVersionStore;

/// Bounds allocation metadata, independently of installed policy-package limits.
pub const MAX_ALLOCATION_RECORDS: usize = 4096;
pub const MAX_ALLOCATION_BYTES: u64 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllocatedPolicyVersion {
    candidate_id: String,
    request_digest: String,
    requested_family_id: Option<String>,
    identity: PolicyVersionIdentity,
}

impl AllocatedPolicyVersion {
    pub fn candidate_id(&self) -> &str {
        &self.candidate_id
    }

    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }

    pub fn requested_family_id(&self) -> Option<&str> {
        self.requested_family_id.as_deref()
    }

    pub fn identity(&self) -> &PolicyVersionIdentity {
        &self.identity
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredAllocation {
    format_version: u32,
    candidate_id: String,
    request_digest: String,
    #[serde(deserialize_with = "required_optional_string")]
    requested_family_id: Option<String>,
    family_id: String,
    version_id: String,
    version: u32,
}

impl StoredAllocation {
    fn validate(self) -> Result<AllocatedPolicyVersion, AdminError> {
        validate_request(
            &self.candidate_id,
            &self.request_digest,
            self.requested_family_id.as_deref(),
        )?;
        if self.format_version != 1
            || self
                .requested_family_id
                .as_ref()
                .is_some_and(|id| id != &self.family_id)
            || (self.requested_family_id.is_none() && self.version != 1)
        {
            return Err(AdminError::InvalidCandidate);
        }
        let identity = PolicyVersionIdentity::new(self.family_id, self.version_id, self.version)
            .map_err(|_| AdminError::InvalidCandidate)?;
        Ok(AllocatedPolicyVersion {
            candidate_id: self.candidate_id,
            request_digest: self.request_digest,
            requested_family_id: self.requested_family_id,
            identity,
        })
    }
}

fn required_optional_string<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::deserialize(deserializer)
}

fn valid_candidate_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_request(candidate: &str, digest: &str, family: Option<&str>) -> Result<(), AdminError> {
    if !valid_candidate_id(candidate)
        || !lower_hex(digest, 64)
        || family.is_some_and(|value| !lower_hex(value, 32))
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(())
}

fn random_id() -> Result<String, AdminError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| AdminError::FileWrite)?;
    Ok(crate::io::lowercase_hex(&bytes))
}

pub(crate) fn allocate_command(arguments: &Arguments) -> Result<(), AdminError> {
    arguments.require_known(&[
        "--root",
        "--candidate-id",
        "--request-digest",
        "--family-id",
    ])?;
    let root = arguments.required_path("--root")?;
    let candidate = arguments.required("--candidate-id")?;
    let digest = arguments.required("--request-digest")?;
    let family = arguments.optional("--family-id")?;
    validate_request(candidate, digest, family)?;
    PolicyVersionStore::new(root)?.allocate(candidate, digest, family)?;
    Ok(())
}
