use raziel_policy_ir::{PolicyLimits, ValidationCeilings};
use raziel_snapshot::SnapshotLimits;

pub const MAX_CANONICAL_POLICY_BYTES: usize = 1_048_576;

pub const TRUSTED_POLICY_CEILINGS: ValidationCeilings = ValidationCeilings {
    policy_limits: PolicyLimits {
        max_expression_nodes: 64,
        max_caveat_nodes: 64,
        max_path_depth: 4,
        max_relationship_witnesses: 32,
        max_merkle_depth: 32,
        max_string_bytes: 256,
        max_context_entries: 16,
        max_total_private_input_bytes: 4096,
    },
    max_definitions: 16,
    max_relations_per_definition: 16,
    max_allowed_subject_types_per_relation: 16,
    max_permissions: 32,
    max_caveats: 16,
    max_parameters_per_caveat: 16,
    max_syntax_depth: 16,
};

pub const TRUSTED_SNAPSHOT_LIMITS: SnapshotLimits = SnapshotLimits {
    max_relationships: 256,
    max_relationship_bytes: 4096,
    max_total_canonical_bytes: 1_048_576,
};
