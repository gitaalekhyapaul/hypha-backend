use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AuthorizationError, AuthorizationGuestInputV1,
    CapabilityRequestBindingV1, TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS, agent_subject,
    authorize, capability_request_commitment, capability_scope_hash, issuance_nullifier,
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

const KEY_THUMBPRINT: [u8; 32] = [0xab; 32];
const NONCE: [u8; 32] = [0xcd; 32];
const SNAPSHOT_SALT: [u8; 32] = [0xef; 32];

#[test]
fn authorizes_a_key_bound_direct_relationship() {
    let input = direct_input(false);
    let result = authorize(&input).unwrap();

    assert_eq!(result.public_values.policy_hash, input.expected_policy_hash);
    assert_eq!(
        result.public_values.relationship_snapshot_root,
        input.relationship_snapshot_root
    );
    assert_eq!(
        result.public_values.capability_request_commitment,
        capability_request_commitment(&input.request).unwrap()
    );
    assert_eq!(
        result.public_values.capability_scope_hash,
        capability_scope_hash(
            &input.request,
            input.issued_at_unix_seconds,
            input.expires_at_unix_seconds
        )
        .unwrap()
    );
    assert_eq!(
        result.public_values.issuance_nullifier,
        issuance_nullifier(&result.public_values.capability_request_commitment).unwrap()
    );
}

#[test]
fn authorizes_bound_request_relationship_and_capability_caveat_values() {
    let input = caveated_input();
    assert!(authorize(&input).is_ok());

    let mut wrong_task = input.clone();
    wrong_task.request.task_id = "different-task".into();
    assert_eq!(
        authorize(&wrong_task),
        Err(AuthorizationError::AuthorizationDenied)
    );

    let mut excessive_amount = input;
    excessive_amount.request.request_context[0].value = CaveatValue::Int(101);
    assert_eq!(
        authorize(&excessive_amount),
        Err(AuthorizationError::AuthorizationDenied)
    );
}

#[test]
fn rejects_a_different_key_and_an_unused_valid_opening() {
    let mut different_key = direct_input(false);
    different_key.request.agent_key_thumbprint = [0x11; 32];
    assert_eq!(
        authorize(&different_key),
        Err(AuthorizationError::AuthorizationDenied)
    );

    let unused = direct_input(true);
    assert_eq!(
        authorize(&unused),
        Err(AuthorizationError::InvalidWitnessUsage)
    );
}

#[test]
fn rejects_mutated_roots_hashes_context_and_time() {
    let input = direct_input(false);

    let mut policy_hash = input.clone();
    policy_hash.expected_policy_hash[0] ^= 1;
    assert_eq!(
        authorize(&policy_hash),
        Err(AuthorizationError::PolicyHashMismatch)
    );

    let mut root = input.clone();
    root.relationship_snapshot_root[0] ^= 1;
    assert_eq!(authorize(&root), Err(AuthorizationError::InvalidWitness));

    let mut context = input.clone();
    context.request.request_context = vec![
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(true),
        },
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(false),
        },
    ];
    assert_eq!(authorize(&context), Err(AuthorizationError::InvalidContext));

    let mut time = input;
    time.expires_at_unix_seconds = time.issued_at_unix_seconds;
    assert_eq!(authorize(&time), Err(AuthorizationError::InvalidTimestamp));
}

#[test]
fn rejects_invalid_request_headers_and_time_relations() {
    assert_request_error(
        |input| input.format_version += 1,
        AuthorizationError::UnsupportedInputVersion,
    );
    assert_request_error(
        |input| input.request.format_version += 1,
        AuthorizationError::UnsupportedRequestVersion,
    );
    assert_request_error(
        |input| input.request.agent_key_thumbprint = [0; 32],
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.issuance_nonce = [0; 32],
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.task_id.clear(),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.purpose.clear(),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.action.clear(),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.resource_type.clear(),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.resource_scope.clear(),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.snapshot_id.clear(),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.request.task_id = "x".repeat(257),
        AuthorizationError::InvalidRequest,
    );
    assert_request_error(
        |input| input.issued_at_unix_seconds = -1,
        AuthorizationError::InvalidTimestamp,
    );
    assert_request_error(
        |input| input.expires_at_unix_seconds = input.issued_at_unix_seconds,
        AuthorizationError::InvalidTimestamp,
    );
    assert_request_error(
        |input| {
            input.expires_at_unix_seconds = input.request.requested_expires_at_unix_seconds + 1;
        },
        AuthorizationError::InvalidTimestamp,
    );
}

#[test]
fn rejects_malformed_request_context_and_missing_required_context() {
    assert_context_error(vec![
        ContextEntry {
            name: "z_value".into(),
            value: CaveatValue::Bool(true),
        },
        ContextEntry {
            name: "a_value".into(),
            value: CaveatValue::Bool(true),
        },
    ]);
    assert_context_error(vec![
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(true),
        },
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(false),
        },
    ]);
    assert_context_error(vec![ContextEntry {
        name: "Invalid".into(),
        value: CaveatValue::Bool(true),
    }]);
    assert_context_error(vec![ContextEntry {
        name: "value".into(),
        value: CaveatValue::String("x".repeat(257)),
    }]);
    assert_context_error(
        (0..17)
            .map(|index| ContextEntry {
                name: format!("value_{index:02}"),
                value: CaveatValue::Bool(true),
            })
            .collect(),
    );

    let mut missing = caveated_input();
    missing.request.request_context.clear();
    assert_eq!(
        authorize(&missing),
        Err(AuthorizationError::EvaluationFailed)
    );
}

#[test]
fn derives_every_fixed_capability_claim_and_rejects_unsupported_declarations() {
    let cases = [
        (
            "action",
            CaveatValueType::String,
            CaveatExpr::ConstString("conditional".into()),
        ),
        (
            "agent_key_thumbprint",
            CaveatValueType::String,
            CaveatExpr::ConstString(
                "abababababababababababababababababababababababababababababababab".into(),
            ),
        ),
        (
            "expires_at",
            CaveatValueType::Int,
            CaveatExpr::ConstInt(1_800),
        ),
        (
            "issued_at",
            CaveatValueType::Int,
            CaveatExpr::ConstInt(1_000),
        ),
        (
            "purpose",
            CaveatValueType::String,
            CaveatExpr::ConstString("reconciliation".into()),
        ),
        (
            "requested_expires_at",
            CaveatValueType::Int,
            CaveatExpr::ConstInt(1_900),
        ),
        (
            "resource_scope",
            CaveatValueType::String,
            CaveatExpr::ConstString("payments".into()),
        ),
        (
            "resource_type",
            CaveatValueType::String,
            CaveatExpr::ConstString("dataset".into()),
        ),
        (
            "snapshot_id",
            CaveatValueType::String,
            CaveatExpr::ConstString("snapshot_1".into()),
        ),
        (
            "task_id",
            CaveatValueType::String,
            CaveatExpr::ConstString("task_1".into()),
        ),
    ];

    for (name, value_type, expected) in cases {
        let input = capability_claim_input(name, value_type, expected);
        assert!(authorize(&input).is_ok(), "fixed claim {name} was rejected");
    }

    let unknown = capability_claim_input(
        "unknown_claim",
        CaveatValueType::String,
        CaveatExpr::ConstString("unknown".into()),
    );
    assert_eq!(authorize(&unknown), Err(AuthorizationError::InvalidPolicy));

    let wrong_type =
        capability_claim_input("task_id", CaveatValueType::Int, CaveatExpr::ConstInt(1_000));
    assert_eq!(
        authorize(&wrong_type),
        Err(AuthorizationError::InvalidPolicy)
    );
}

#[test]
fn key_subject_is_fixed_lowercase_hex() {
    assert_eq!(
        agent_subject(&KEY_THUMBPRINT).object_id,
        "abababababababababababababababababababababababababababababababab"
    );
}

fn assert_request_error(
    mutate: impl FnOnce(&mut AuthorizationGuestInputV1),
    expected: AuthorizationError,
) {
    let mut input = direct_input(false);
    mutate(&mut input);
    assert_eq!(authorize(&input), Err(expected));
}

fn assert_context_error(context: Vec<ContextEntry>) {
    let mut input = direct_input(false);
    input.request.request_context = context;
    assert_eq!(authorize(&input), Err(AuthorizationError::InvalidContext));
}

fn direct_input(include_unused: bool) -> AuthorizationGuestInputV1 {
    let policy = policy();
    let subject = agent_subject(&KEY_THUMBPRINT);
    let mut relationships = vec![RelationshipRecord {
        resource_type: "dataset".into(),
        resource_id: "payments".into(),
        relation: "direct".into(),
        subject_type: subject.object_type,
        subject_id: subject.object_id,
        subject_relation: None,
        stored_caveat: None,
    }];
    if include_unused {
        relationships.push(RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "payments_2".into(),
            relation: "direct".into(),
            subject_type: "agent".into(),
            subject_id: agent_subject(&KEY_THUMBPRINT).object_id,
            subject_relation: None,
            stored_caveat: None,
        });
    }
    build_input(policy, relationships, "query", vec![])
}

fn capability_claim_input(
    name: &str,
    value_type: CaveatValueType,
    expected: CaveatExpr,
) -> AuthorizationGuestInputV1 {
    let mut program = policy().program().clone();
    program.caveats[0].parameters = vec![CaveatParameter {
        source: CaveatInputSource::CapabilityClaim,
        name: name.into(),
        value_type,
    }];
    program.caveats[0].expression = CaveatExpr::Equal(
        Box::new(expected),
        Box::new(CaveatExpr::InputRef {
            source: CaveatInputSource::CapabilityClaim,
            name: name.into(),
            value_type,
        }),
    );
    let policy = ValidatedPolicyProgram::new(program, &TRUSTED_POLICY_CEILINGS).unwrap();
    let subject = agent_subject(&KEY_THUMBPRINT);
    let relationships = vec![RelationshipRecord {
        resource_type: "dataset".into(),
        resource_id: "payments".into(),
        relation: "caveated".into(),
        subject_type: subject.object_type,
        subject_id: subject.object_id,
        subject_relation: None,
        stored_caveat: Some(StoredCaveat {
            name: "reconcile_scope".into(),
            context: vec![],
        }),
    }];
    build_input(policy, relationships, "conditional", vec![])
}

fn caveated_input() -> AuthorizationGuestInputV1 {
    let policy = policy();
    let subject = agent_subject(&KEY_THUMBPRINT);
    let relationships = vec![RelationshipRecord {
        resource_type: "dataset".into(),
        resource_id: "payments".into(),
        relation: "caveated".into(),
        subject_type: subject.object_type,
        subject_id: subject.object_id,
        subject_relation: None,
        stored_caveat: Some(StoredCaveat {
            name: "reconcile_scope".into(),
            context: vec![StoredContextEntry {
                name: "approved_amount".into(),
                value: CaveatValue::Int(100),
            }],
        }),
    }];
    build_input(
        policy,
        relationships,
        "conditional",
        vec![ContextEntry {
            name: "requested_amount".into(),
            value: CaveatValue::Int(80),
        }],
    )
}

fn build_input(
    policy: ValidatedPolicyProgram,
    relationships: Vec<RelationshipRecord>,
    action: &str,
    request_context: Vec<ContextEntry>,
) -> AuthorizationGuestInputV1 {
    let leaf_count = u32::try_from(relationships.len()).unwrap();
    let tree = SnapshotTree::build(
        &policy,
        relationships,
        &SNAPSHOT_SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let entries = (0..leaf_count)
        .map(|index| tree.opening(index).unwrap())
        .collect();
    let witness = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: SNAPSHOT_SALT,
        leaf_count,
        entries,
    };
    AuthorizationGuestInputV1 {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        expected_policy_hash: policy_hash(&policy).unwrap(),
        relationship_snapshot_root: tree.root(),
        relationship_witness_set: witness,
        policy: policy.program().clone(),
        request: CapabilityRequestBindingV1 {
            format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
            agent_key_thumbprint: KEY_THUMBPRINT,
            task_id: "task_1".into(),
            purpose: "reconciliation".into(),
            action: action.into(),
            resource_type: "dataset".into(),
            resource_scope: "payments".into(),
            requested_expires_at_unix_seconds: 1_900,
            issuance_nonce: NONCE,
            snapshot_id: "snapshot_1".into(),
            request_context,
        },
        issued_at_unix_seconds: 1_000,
        expires_at_unix_seconds: 1_800,
    }
}

fn policy() -> ValidatedPolicyProgram {
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "authorization_core_test".into(),
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
                                caveat_name: Some("reconcile_scope".into()),
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
                    ],
                },
            ],
            permissions: vec![
                PermissionDefinition {
                    object_type: "dataset".into(),
                    name: "conditional".into(),
                    expression: PermissionExpr::CaveatedRelation {
                        relation: "caveated".into(),
                        caveat_name: "reconcile_scope".into(),
                    },
                },
                PermissionDefinition {
                    object_type: "dataset".into(),
                    name: "query".into(),
                    expression: PermissionExpr::RelationRef {
                        name: "direct".into(),
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
        },
        &TRUSTED_POLICY_CEILINGS,
    )
    .unwrap()
}
