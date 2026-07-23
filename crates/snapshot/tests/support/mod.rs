#![allow(dead_code)]

use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ObjectDefinition, PermissionDefinition, PermissionExpr,
    PolicyLimits, PolicyProgram, RelationDefinition, ValidatedPolicyProgram, ValidationCeilings,
};
use raziel_snapshot::{RelationshipRecord, SnapshotLimits, StoredCaveat, StoredContextEntry};

pub const TEST_SALT: [u8; 32] = [0x5a; 32];
pub const TEST_SNAPSHOT_LIMITS: SnapshotLimits = SnapshotLimits {
    max_relationships: 256,
    max_relationship_bytes: 4096,
    max_total_canonical_bytes: 1_048_576,
};
pub const TEST_POLICY_LIMITS: PolicyLimits = PolicyLimits {
    max_expression_nodes: 32,
    max_caveat_nodes: 32,
    max_path_depth: 4,
    max_relationship_witnesses: 16,
    max_merkle_depth: 8,
    max_string_bytes: 256,
    max_context_entries: 16,
    max_total_private_input_bytes: 4096,
};

pub fn policy() -> ValidatedPolicyProgram {
    policy_with_limits(TEST_POLICY_LIMITS)
}

pub fn policy_with_limits(policy_limits: PolicyLimits) -> ValidatedPolicyProgram {
    let program = PolicyProgram {
        ir_version: 1,
        compiler_name: "synthetic-test-compiler".into(),
        compiler_version: "1".into(),
        source_schema_digest: [0x42; 32],
        definitions: vec![
            ObjectDefinition {
                name: "agent".into(),
                relations: vec![],
            },
            ObjectDefinition {
                name: "dataset".into(),
                relations: vec![
                    RelationDefinition {
                        name: "caveated".into(),
                        allowed_subject_types: vec![AllowedSubjectType {
                            object_type: "agent".into(),
                            subject_relation: None,
                            caveat_name: Some("access_rule".into()),
                        }],
                    },
                    RelationDefinition {
                        name: "direct".into(),
                        allowed_subject_types: vec![AllowedSubjectType {
                            object_type: "agent".into(),
                            subject_relation: None,
                            caveat_name: None,
                        }],
                    },
                    RelationDefinition {
                        name: "grouped".into(),
                        allowed_subject_types: vec![AllowedSubjectType {
                            object_type: "team".into(),
                            subject_relation: Some("member".into()),
                            caveat_name: None,
                        }],
                    },
                ],
            },
            ObjectDefinition {
                name: "team".into(),
                relations: vec![RelationDefinition {
                    name: "member".into(),
                    allowed_subject_types: vec![AllowedSubjectType {
                        object_type: "agent".into(),
                        subject_relation: None,
                        caveat_name: None,
                    }],
                }],
            },
        ],
        permissions: vec![PermissionDefinition {
            object_type: "dataset".into(),
            name: "query".into(),
            expression: PermissionExpr::RelationRef {
                name: "direct".into(),
            },
        }],
        caveats: vec![CaveatDefinition {
            name: "access_rule".into(),
            parameters: vec![
                CaveatParameter {
                    source: CaveatInputSource::RequestContext,
                    name: "requested".into(),
                    value_type: CaveatValueType::Int,
                },
                CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "approved".into(),
                    value_type: CaveatValueType::Bool,
                },
                CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "limit".into(),
                    value_type: CaveatValueType::Int,
                },
                CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "region".into(),
                    value_type: CaveatValueType::String,
                },
                CaveatParameter {
                    source: CaveatInputSource::CapabilityClaim,
                    name: "task".into(),
                    value_type: CaveatValueType::String,
                },
            ],
            expression: CaveatExpr::ConstBool(true),
        }],
        limits: policy_limits,
    };
    ValidatedPolicyProgram::new(
        program,
        &ValidationCeilings {
            policy_limits: TEST_POLICY_LIMITS,
            max_definitions: 8,
            max_relations_per_definition: 8,
            max_allowed_subject_types_per_relation: 8,
            max_permissions: 8,
            max_caveats: 8,
            max_parameters_per_caveat: 8,
            max_syntax_depth: 8,
        },
    )
    .unwrap()
}

pub fn relationships() -> Vec<RelationshipRecord> {
    vec![
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "synthetic-private-test-a".into(),
            relation: "caveated".into(),
            subject_type: "agent".into(),
            subject_id: "synthetic-agent-a".into(),
            subject_relation: None,
            stored_caveat: Some(StoredCaveat {
                name: "access_rule".into(),
                context: vec![
                    StoredContextEntry {
                        name: "approved".into(),
                        value: CaveatValue::Bool(true),
                    },
                    StoredContextEntry {
                        name: "limit".into(),
                        value: CaveatValue::Int(500),
                    },
                    StoredContextEntry {
                        name: "region".into(),
                        value: CaveatValue::String("synthetic-region".into()),
                    },
                ],
            }),
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "synthetic-private-test-a".into(),
            relation: "direct".into(),
            subject_type: "agent".into(),
            subject_id: "synthetic-agent-b".into(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "synthetic-private-test-a".into(),
            relation: "grouped".into(),
            subject_type: "team".into(),
            subject_id: "synthetic-team-a".into(),
            subject_relation: Some("member".into()),
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "synthetic-private-test-b".into(),
            relation: "direct".into(),
            subject_type: "agent".into(),
            subject_id: "synthetic-agent-a".into(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "synthetic-private-test-c".into(),
            relation: "direct".into(),
            subject_type: "agent".into(),
            subject_id: "synthetic-agent-c".into(),
            subject_relation: None,
            stored_caveat: None,
        },
    ]
}
