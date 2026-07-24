use std::fs;

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
use raziel_prover::{MAX_EXECUTION_CYCLE_LIMIT, ProverError, Sp1Executor, prepare_guest_input};
use raziel_snapshot::{
    RELATIONSHIP_WITNESS_SET_VERSION, RelationshipRecord, RelationshipWitnessSetV1, SnapshotTree,
};
use sp1_sdk::{
    Elf, StatusCode,
    blocking::{Prover, ProverClient, SP1Stdin},
};

mod support;

use support::{KEY_THUMBPRINT, SNAPSHOT_SALT, guest_elf_path};

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn sp1_execute_matches_the_host_for_the_supported_positive_paths() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let executor = Sp1Executor::local();

    for (case_id, input) in inputs() {
        let prepared = prepare_guest_input(input).unwrap_or_else(|error| {
            panic!("{case_id}: host input preparation failed: {error}");
        });
        let report = executor
            .execute(&guest_elf, &prepared)
            .unwrap_or_else(|error| panic!("{case_id}: SP1 execution failed: {error}"));

        assert_eq!(
            report.public_values,
            prepared.expected_public_values_bytes(),
            "{case_id}: public journal mismatch"
        );
        assert!(
            report.instruction_count > 0,
            "{case_id}: missing instruction count"
        );
        eprintln!("{case_id}: instructions={}", report.instruction_count);
    }
}

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn sp1_guest_rejects_invalid_private_inputs_without_a_public_journal() {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let valid = inputs();
    let case = |case_id: &str| {
        valid
            .iter()
            .find(|(id, _)| *id == case_id)
            .map(|(_, input)| input.clone())
            .unwrap()
    };

    let mut wrong_input_version = case("direct");
    wrong_input_version.format_version += 1;

    let mut wrong_policy_hash = case("direct");
    wrong_policy_hash.expected_policy_hash[0] ^= 1;

    let mut wrong_snapshot_root = case("direct");
    wrong_snapshot_root.relationship_snapshot_root[0] ^= 1;

    let mut invalid_time = case("direct");
    invalid_time.expires_at_unix_seconds = invalid_time.issued_at_unix_seconds;

    let mut duplicate_context = case("direct");
    duplicate_context.request.request_context = vec![
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(true),
        },
        ContextEntry {
            name: "duplicate".into(),
            value: CaveatValue::Bool(false),
        },
    ];

    let mut missing_context = case("caveated");
    missing_context.request.request_context.clear();

    let mut oversized_request = case("direct");
    oversized_request.request.task_id = "x".repeat(257);

    let mut unused_opening = case("direct");
    unused_opening.relationship_witness_set.entries.insert(
        0,
        case("caveated").relationship_witness_set.entries[0].clone(),
    );

    let mut duplicate_opening = case("direct");
    duplicate_opening
        .relationship_witness_set
        .entries
        .push(duplicate_opening.relationship_witness_set.entries[0].clone());

    let mut out_of_order_openings = case("intersection");
    out_of_order_openings
        .relationship_witness_set
        .entries
        .swap(0, 1);

    let mut cases = [
        ("wrong-input-version", wrong_input_version),
        ("wrong-policy-hash", wrong_policy_hash),
        ("wrong-snapshot-root", wrong_snapshot_root),
        ("invalid-time", invalid_time),
        ("duplicate-context", duplicate_context),
        ("missing-context", missing_context),
        ("oversized-request", oversized_request),
        ("unused-opening", unused_opening),
        ("duplicate-opening", duplicate_opening),
        ("out-of-order-openings", out_of_order_openings),
    ]
    .into_iter()
    .map(|(case_id, input)| {
        let mut stdin = SP1Stdin::new();
        stdin.write(&input);
        (case_id, stdin, StatusCode::PANIC)
    })
    .collect::<Vec<_>>();
    cases.push((
        "malformed-serialized-input",
        SP1Stdin::from(&[0xff]),
        StatusCode::INVALID_HINT,
    ));

    let prover = ProverClient::builder().light().build();
    for (case_id, stdin, expected_status) in cases {
        let (public_values, report) = prover
            .execute(Elf::from(guest_elf.as_slice()), stdin)
            .cycle_limit(MAX_EXECUTION_CYCLE_LIMIT)
            .calculate_gas(true)
            .expected_exit_code(StatusCode::ANY)
            .run()
            .unwrap_or_else(|error| {
                panic!("{case_id}: executor failed before returning the guest result: {error}")
            });
        assert_eq!(
            report.exit_code,
            u64::from(expected_status.as_u32()),
            "{case_id}: invalid input returned the wrong guest status"
        );
        assert!(
            public_values.as_slice().is_empty(),
            "{case_id}: invalid input produced a public journal"
        );
    }
}

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn executor_rejects_invalid_cycle_limits_and_enforces_a_low_limit() {
    assert!(matches!(
        Sp1Executor::local().with_cycle_limit(0),
        Err(ProverError::InvalidCycleLimit)
    ));
    assert!(matches!(
        Sp1Executor::local().with_cycle_limit(MAX_EXECUTION_CYCLE_LIMIT + 1),
        Err(ProverError::InvalidCycleLimit)
    ));
    assert!(
        Sp1Executor::local()
            .with_cycle_limit(MAX_EXECUTION_CYCLE_LIMIT)
            .is_ok()
    );

    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let input = inputs().remove(0).1;
    let prepared = prepare_guest_input(input).unwrap();
    let result = Sp1Executor::local()
        .with_cycle_limit(1)
        .unwrap()
        .execute(&guest_elf, &prepared);
    assert_eq!(result, Err(ProverError::GuestExecutionFailed));
}

fn inputs() -> Vec<(&'static str, AuthorizationGuestInputV1)> {
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
