use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, EvaluationRequest, ObjectDefinition, ObjectRef,
    PermissionDefinition, PermissionExpr, PolicyLimits, PolicyProgram, RelationDefinition,
    ValidatedPolicyProgram, ValidationCeilings,
};
use raziel_snapshot::{
    RelationshipRecord, SnapshotError, SnapshotLimits, SnapshotTree, StoredCaveat,
    StoredContextEntry, verify_witness_set,
};
use raziel_witness::{WitnessBuildError, build_positive_witness};

const SNAPSHOT_SALT: [u8; 32] = [0x5a; 32];
const SNAPSHOT_LIMITS: SnapshotLimits = SnapshotLimits {
    max_relationships: 16,
    max_relationship_bytes: 512,
    max_total_canonical_bytes: 4096,
};

#[test]
fn builds_only_the_first_canonical_positive_path() {
    let policy = policy();
    let relationships = relationships();
    let root = snapshot_root(&policy, &relationships);

    let direct = build_positive_witness(
        &policy,
        &request("access", "private_alice"),
        relationships.clone(),
        SNAPSHOT_SALT,
        root,
        SNAPSHOT_LIMITS,
    )
    .unwrap();
    assert_eq!(leaf_indices(&direct), [0]);

    let intersection = build_positive_witness(
        &policy,
        &request("dual_access", "private_alice"),
        relationships.clone(),
        SNAPSHOT_SALT,
        root,
        SNAPSHOT_LIMITS,
    )
    .unwrap();
    assert_eq!(leaf_indices(&intersection), [0, 2]);

    let caveated = build_positive_witness(
        &policy,
        &request("conditional_access", "private_alice"),
        relationships,
        SNAPSHOT_SALT,
        root,
        SNAPSHOT_LIMITS,
    )
    .unwrap();
    assert_eq!(leaf_indices(&caveated), [1]);
    assert!(
        caveated.entries[0].canonical_relationship.stored_caveat
            == Some(StoredCaveat {
                name: "approved".into(),
                context: vec![StoredContextEntry {
                    name: "granted".into(),
                    value: CaveatValue::Bool(true),
                }],
            })
    );
    verify_witness_set(&policy, &caveated, &root, SNAPSHOT_LIMITS).unwrap();
}

#[test]
fn denied_requests_return_no_witness() {
    let policy = policy();
    let relationships = relationships();
    let root = snapshot_root(&policy, &relationships);

    assert_eq!(
        build_positive_witness(
            &policy,
            &request("access", "private_bob"),
            relationships,
            SNAPSHOT_SALT,
            root,
            SNAPSHOT_LIMITS,
        ),
        Err(WitnessBuildError::AuthorizationDenied)
    );
}

#[test]
fn committed_root_and_snapshot_order_fail_closed() {
    let policy = policy();
    let relationships = relationships();
    let root = snapshot_root(&policy, &relationships);
    let mut wrong_root = root;
    wrong_root[0] ^= 1;

    assert_eq!(
        build_positive_witness(
            &policy,
            &request("access", "private_alice"),
            relationships.clone(),
            SNAPSHOT_SALT,
            wrong_root,
            SNAPSHOT_LIMITS,
        ),
        Err(WitnessBuildError::SnapshotRootMismatch)
    );

    let mut unsorted = relationships.clone();
    unsorted.swap(0, 1);
    assert_eq!(
        build_positive_witness(
            &policy,
            &request("access", "private_alice"),
            unsorted,
            SNAPSHOT_SALT,
            root,
            SNAPSHOT_LIMITS,
        ),
        Err(WitnessBuildError::InvalidSnapshot(
            SnapshotError::RelationshipsNotStrictlySorted
        ))
    );

    let bounded_limits = SnapshotLimits {
        max_relationships: 2,
        ..SNAPSHOT_LIMITS
    };
    assert_eq!(
        build_positive_witness(
            &policy,
            &request("access", "private_alice"),
            relationships,
            SNAPSHOT_SALT,
            root,
            bounded_limits,
        ),
        Err(WitnessBuildError::InvalidSnapshot(
            SnapshotError::LimitExceeded(raziel_snapshot::SnapshotLimit::Relationships)
        ))
    );
}

#[test]
fn errors_and_witness_debug_output_redact_private_values() {
    let policy = policy();
    let relationships = relationships();
    let root = snapshot_root(&policy, &relationships);
    let witness = build_positive_witness(
        &policy,
        &request("access", "private_alice"),
        relationships.clone(),
        SNAPSHOT_SALT,
        root,
        SNAPSHOT_LIMITS,
    )
    .unwrap();
    let denied = build_positive_witness(
        &policy,
        &request("access", "private_bob"),
        relationships,
        SNAPSHOT_SALT,
        root,
        SNAPSHOT_LIMITS,
    )
    .unwrap_err();

    for output in [
        format!("{witness:?}"),
        format!("{denied:?}"),
        denied.to_string(),
    ] {
        assert!(!output.contains("private_payments"));
        assert!(!output.contains("private_alice"));
        assert!(!output.contains("private_bob"));
        assert!(!output.contains("granted"));
    }
}

fn leaf_indices(witness: &raziel_snapshot::RelationshipWitnessSetV1) -> Vec<u32> {
    witness
        .entries
        .iter()
        .map(|entry| entry.leaf_index)
        .collect()
}

fn snapshot_root(
    policy: &ValidatedPolicyProgram,
    relationships: &[RelationshipRecord],
) -> [u8; 32] {
    SnapshotTree::build(
        policy,
        relationships.to_vec(),
        &SNAPSHOT_SALT,
        SNAPSHOT_LIMITS,
    )
    .unwrap()
    .root()
}

fn request(permission: &str, subject_id: &str) -> EvaluationRequest {
    EvaluationRequest {
        resource: ObjectRef {
            object_type: "dataset".into(),
            object_id: "private_payments".into(),
        },
        subject: ObjectRef {
            object_type: "agent".into(),
            object_id: subject_id.into(),
        },
        permission: permission.into(),
        request_context: vec![],
        capability_claims: vec![],
    }
}

fn relationships() -> Vec<RelationshipRecord> {
    vec![
        relationship("approver", None),
        relationship(
            "conditional",
            Some(StoredCaveat {
                name: "approved".into(),
                context: vec![StoredContextEntry {
                    name: "granted".into(),
                    value: CaveatValue::Bool(true),
                }],
            }),
        ),
        relationship("viewer", None),
    ]
}

fn relationship(relation: &str, stored_caveat: Option<StoredCaveat>) -> RelationshipRecord {
    RelationshipRecord {
        resource_type: "dataset".into(),
        resource_id: "private_payments".into(),
        relation: relation.into(),
        subject_type: "agent".into(),
        subject_id: "private_alice".into(),
        subject_relation: None,
        stored_caveat,
    }
}

fn policy() -> ValidatedPolicyProgram {
    let limits = policy_limits();
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "witness_test".into(),
            compiler_version: "1".into(),
            source_schema_digest: [0x41; 32],
            definitions: vec![
                ObjectDefinition {
                    name: "agent".into(),
                    relations: vec![],
                },
                ObjectDefinition {
                    name: "dataset".into(),
                    relations: vec![
                        direct_relation("approver"),
                        RelationDefinition {
                            name: "conditional".into(),
                            allowed_subject_types: vec![AllowedSubjectType {
                                object_type: "agent".into(),
                                subject_relation: None,
                                caveat_name: Some("approved".into()),
                            }],
                        },
                        direct_relation("viewer"),
                    ],
                },
            ],
            permissions: vec![
                PermissionDefinition {
                    object_type: "dataset".into(),
                    name: "access".into(),
                    expression: PermissionExpr::AnyOf(vec![
                        relation_ref("approver"),
                        relation_ref("viewer"),
                    ]),
                },
                PermissionDefinition {
                    object_type: "dataset".into(),
                    name: "conditional_access".into(),
                    expression: PermissionExpr::CaveatedRelation {
                        relation: "conditional".into(),
                        caveat_name: "approved".into(),
                    },
                },
                PermissionDefinition {
                    object_type: "dataset".into(),
                    name: "dual_access".into(),
                    expression: PermissionExpr::AllOf(vec![
                        relation_ref("approver"),
                        relation_ref("viewer"),
                    ]),
                },
            ],
            caveats: vec![CaveatDefinition {
                name: "approved".into(),
                parameters: vec![CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "granted".into(),
                    value_type: CaveatValueType::Bool,
                }],
                expression: CaveatExpr::InputRef {
                    source: CaveatInputSource::RelationshipContext,
                    name: "granted".into(),
                    value_type: CaveatValueType::Bool,
                },
            }],
            limits,
        },
        &ValidationCeilings {
            policy_limits: limits,
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

fn direct_relation(name: &str) -> RelationDefinition {
    RelationDefinition {
        name: name.into(),
        allowed_subject_types: vec![AllowedSubjectType {
            object_type: "agent".into(),
            subject_relation: None,
            caveat_name: None,
        }],
    }
}

fn relation_ref(name: &str) -> PermissionExpr {
    PermissionExpr::RelationRef { name: name.into() }
}

const fn policy_limits() -> PolicyLimits {
    PolicyLimits {
        max_expression_nodes: 32,
        max_caveat_nodes: 16,
        max_path_depth: 4,
        max_relationship_witnesses: 8,
        max_merkle_depth: 8,
        max_string_bytes: 128,
        max_context_entries: 8,
        max_total_private_input_bytes: 4096,
    }
}
