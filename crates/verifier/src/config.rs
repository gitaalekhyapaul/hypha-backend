use raziel_policy_ir::{PolicyLimits, ValidationCeilings};

use crate::{RegistryConfig, RegistryError, RegistryLimits};

pub const DEFAULT_MAX_REGISTRY_RECORD_BYTES: u64 = 1 << 20;
pub const DEFAULT_MAX_REGISTRY_STRING_BYTES: u32 = 512;
pub const DEFAULT_MAX_ARTIFACT_LIFETIME_SECONDS: u64 = 31 * 24 * 60 * 60;

/// Builds the shared registry limits used by administration, proving, and verification.
pub fn default_registry_config(trust_domain_id: &str) -> Result<RegistryConfig, RegistryError> {
    RegistryConfig::new(
        trust_domain_id.into(),
        RegistryLimits {
            max_record_bytes: DEFAULT_MAX_REGISTRY_RECORD_BYTES,
            max_string_bytes: DEFAULT_MAX_REGISTRY_STRING_BYTES,
            max_artifact_lifetime_seconds: DEFAULT_MAX_ARTIFACT_LIFETIME_SECONDS,
            max_future_clock_skew_seconds: 30,
            max_manifest_files_per_kind: 10_000,
            max_manifest_total_bytes: 64 << 20,
            policy_validation: default_policy_validation_ceilings(),
        },
    )
}

pub const fn default_policy_validation_ceilings() -> ValidationCeilings {
    ValidationCeilings {
        policy_limits: PolicyLimits {
            max_expression_nodes: 4_096,
            max_caveat_nodes: 4_096,
            max_path_depth: 64,
            max_relationship_witnesses: 4_096,
            max_merkle_depth: 64,
            max_string_bytes: DEFAULT_MAX_REGISTRY_STRING_BYTES,
            max_context_entries: 256,
            max_total_private_input_bytes: 16 << 20,
        },
        max_definitions: 512,
        max_relations_per_definition: 256,
        max_allowed_subject_types_per_relation: 256,
        max_permissions: 1_024,
        max_caveats: 512,
        max_parameters_per_caveat: 128,
        max_syntax_depth: 128,
    }
}
