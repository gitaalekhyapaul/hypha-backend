use raziel_policy_ir::ValidationCeilings;
use raziel_verifier::{
    DEFAULT_MAX_ARTIFACT_LIFETIME_SECONDS, DEFAULT_MAX_REGISTRY_RECORD_BYTES,
    DEFAULT_MAX_REGISTRY_STRING_BYTES, RegistryConfig, default_policy_validation_ceilings,
    default_registry_config,
};

use crate::error::AdminError;

pub const MAX_RECORD_BYTES: u64 = DEFAULT_MAX_REGISTRY_RECORD_BYTES;
pub const MAX_STRING_BYTES: u32 = DEFAULT_MAX_REGISTRY_STRING_BYTES;
pub const MAX_ARTIFACT_LIFETIME_SECONDS: u64 = DEFAULT_MAX_ARTIFACT_LIFETIME_SECONDS;

pub fn registry_config(trust_domain_id: &str) -> Result<RegistryConfig, AdminError> {
    default_registry_config(trust_domain_id).map_err(AdminError::from)
}

pub const fn validation_ceilings() -> ValidationCeilings {
    default_policy_validation_ceilings()
}
