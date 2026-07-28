use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AuthorizationGuestInputV1, CapabilityRequestBindingV1,
    TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS, agent_subject,
};
use raziel_canonical::{StoredCaveat, StoredContextEntry, policy_hash};
use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ContextEntry, ObjectDefinition, PermissionDefinition,
    PermissionExpr, PolicyLimits, PolicyProgram, RelationDefinition, ValidatedPolicyProgram,
};
use raziel_snapshot::{
    RELATIONSHIP_WITNESS_SET_VERSION, RelationshipRecord, RelationshipWitnessSetV1, SnapshotTree,
};

use crate::support::{KEY_THUMBPRINT, SNAPSHOT_SALT};

pub fn authorization_inputs() -> Vec<(&'static str, AuthorizationGuestInputV1)> {
    let policy = ValidatedPolicyProgram::new(policy_program(), &TRUSTED_POLICY_CEILINGS).unwrap();
    let tree = SnapshotTree::build(
        &policy,
        relationships(),
        &SNAPSHOT_SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let policy_hash = policy_hash(&policy).unwrap();
    let root = tree.root();

    let build = |action: &str, leaf_indices: &[u32], request_context: Vec<ContextEntry>| {
        AuthorizationGuestInputV1 {
            format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
            expected_policy_hash: policy_hash,
            relationship_snapshot_root: root,
            relationship_witness_set: RelationshipWitnessSetV1 {
                format_version: RELATIONSHIP_WITNESS_SET_VERSION,
                snapshot_secret_salt: SNAPSHOT_SALT,
                leaf_count: 8,
                entries: leaf_indices
                    .iter()
                    .map(|index| tree.opening(*index).unwrap())
                    .collect(),
            },
            policy: policy.program().clone(),
            request: CapabilityRequestBindingV1 {
                format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
                agent_key_thumbprint: KEY_THUMBPRINT,
                task_id: "task_1".into(),
                purpose: "reconciliation".into(),
                action: action.into(),
                resource_type: "dataset".into(),
                resource_scope: "payments".into(),
                requested_expires_at_unix_seconds: 2_000,
                issuance_nonce: [0x73; 32],
                snapshot_id: "snapshot_1".into(),
                request_context,
            },
            issued_at_unix_seconds: 1_000,
            expires_at_unix_seconds: 1_800,
        }
    };

    vec![
        ("direct", build("direct_access", &[1], vec![])),
        (
            "caveated",
            build(
                "caveat_access",
                &[0],
                vec![ContextEntry {
                    name: "requested_amount".into(),
                    value: CaveatValue::Int(80),
                }],
            ),
        ),
        ("union", build("union_access", &[1], vec![])),
        (
            "intersection",
            build("intersection_access", &[3, 5], vec![]),
        ),
        ("subject-set", build("subject_set_access", &[4, 7], vec![])),
        ("arrow", build("arrow_access", &[2, 6], vec![])),
    ]
}

fn relationships() -> Vec<RelationshipRecord> {
    let subject = agent_subject(&KEY_THUMBPRINT);
    vec![
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments".into(),
            relation: "caveated".into(),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: Some(StoredCaveat {
                name: "reconcile_scope".into(),
                context: vec![StoredContextEntry {
                    name: "approved_amount".into(),
                    value: CaveatValue::Int(100),
                }],
            }),
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments".into(),
            relation: "direct".into(),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments".into(),
            relation: "organization".into(),
            subject_type: "organization".into(),
            subject_id: "finance".into(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments".into(),
            relation: "owner".into(),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments".into(),
            relation: "team".into(),
            subject_type: "team".into(),
            subject_id: "reconciliation".into(),
            subject_relation: Some("member".into()),
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments".into(),
            relation: "viewer".into(),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "organization".into(),
            resource_id: "finance".into(),
            relation: "auditor".into(),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: "team".into(),
            resource_id: "reconciliation".into(),
            relation: "member".into(),
            subject_type: subject.object_type,
            subject_id: subject.object_id,
            subject_relation: None,
            stored_caveat: None,
        },
    ]
}

fn policy_program() -> PolicyProgram {
    PolicyProgram {
        ir_version: 1,
        compiler_name: "prover_execute_test".into(),
        compiler_version: "1".into(),
        source_schema_digest: [0x84; 32],
        definitions: vec![
            ObjectDefinition {
                name: "agent".into(),
                relations: vec![],
            },
            ObjectDefinition {
                name: "dataset".into(),
                relations: vec![
                    relation("caveated", "agent", None, Some("reconcile_scope")),
                    relation("direct", "agent", None, None),
                    relation("organization", "organization", None, None),
                    relation("owner", "agent", None, None),
                    relation("team", "team", Some("member"), None),
                    relation("viewer", "agent", None, None),
                ],
            },
            ObjectDefinition {
                name: "organization".into(),
                relations: vec![relation("auditor", "agent", None, None)],
            },
            ObjectDefinition {
                name: "team".into(),
                relations: vec![relation("member", "agent", None, None)],
            },
        ],
        permissions: vec![
            PermissionDefinition {
                object_type: "dataset".into(),
                name: "arrow_access".into(),
                expression: PermissionExpr::Arrow {
                    source_relation: "organization".into(),
                    target_permission: "audit".into(),
                },
            },
            PermissionDefinition {
                object_type: "dataset".into(),
                name: "caveat_access".into(),
                expression: PermissionExpr::CaveatedRelation {
                    relation: "caveated".into(),
                    caveat_name: "reconcile_scope".into(),
                },
            },
            PermissionDefinition {
                object_type: "dataset".into(),
                name: "direct_access".into(),
                expression: PermissionExpr::RelationRef {
                    name: "direct".into(),
                },
            },
            PermissionDefinition {
                object_type: "dataset".into(),
                name: "intersection_access".into(),
                expression: PermissionExpr::AllOf(vec![
                    PermissionExpr::RelationRef {
                        name: "owner".into(),
                    },
                    PermissionExpr::RelationRef {
                        name: "viewer".into(),
                    },
                ]),
            },
            PermissionDefinition {
                object_type: "dataset".into(),
                name: "subject_set_access".into(),
                expression: PermissionExpr::SubjectRelationRef {
                    relation: "team".into(),
                    subject_relation: "member".into(),
                },
            },
            PermissionDefinition {
                object_type: "dataset".into(),
                name: "union_access".into(),
                expression: PermissionExpr::AnyOf(vec![
                    PermissionExpr::RelationRef {
                        name: "direct".into(),
                    },
                    PermissionExpr::CaveatedRelation {
                        relation: "caveated".into(),
                        caveat_name: "reconcile_scope".into(),
                    },
                ]),
            },
            PermissionDefinition {
                object_type: "organization".into(),
                name: "audit".into(),
                expression: PermissionExpr::RelationRef {
                    name: "auditor".into(),
                },
            },
        ],
        caveats: vec![CaveatDefinition {
            name: "reconcile_scope".into(),
            parameters: vec![
                CaveatParameter {
                    source: CaveatInputSource::RequestContext,
                    name: "requested_amount".into(),
                    value_type: CaveatValueType::Int,
                },
                CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "approved_amount".into(),
                    value_type: CaveatValueType::Int,
                },
                CaveatParameter {
                    source: CaveatInputSource::CapabilityClaim,
                    name: "task_id".into(),
                    value_type: CaveatValueType::String,
                },
            ],
            expression: CaveatExpr::And(vec![
                CaveatExpr::Equal(
                    Box::new(CaveatExpr::ConstString("task_1".into())),
                    Box::new(CaveatExpr::InputRef {
                        source: CaveatInputSource::CapabilityClaim,
                        name: "task_id".into(),
                        value_type: CaveatValueType::String,
                    }),
                ),
                CaveatExpr::LessThanOrEqual(
                    Box::new(CaveatExpr::InputRef {
                        source: CaveatInputSource::RequestContext,
                        name: "requested_amount".into(),
                        value_type: CaveatValueType::Int,
                    }),
                    Box::new(CaveatExpr::InputRef {
                        source: CaveatInputSource::RelationshipContext,
                        name: "approved_amount".into(),
                        value_type: CaveatValueType::Int,
                    }),
                ),
            ]),
        }],
        limits: PolicyLimits {
            max_expression_nodes: 64,
            max_caveat_nodes: 64,
            max_path_depth: 4,
            max_relationship_witnesses: 32,
            max_merkle_depth: 32,
            max_string_bytes: 256,
            max_context_entries: 16,
            max_total_private_input_bytes: 4096,
        },
    }
}

fn relation(
    name: &str,
    object_type: &str,
    subject_relation: Option<&str>,
    caveat_name: Option<&str>,
) -> RelationDefinition {
    RelationDefinition {
        name: name.into(),
        allowed_subject_types: vec![AllowedSubjectType {
            object_type: object_type.into(),
            subject_relation: subject_relation.map(Into::into),
            caveat_name: caveat_name.map(Into::into),
        }],
    }
}
