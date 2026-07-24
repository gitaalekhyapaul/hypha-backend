use std::fs;

use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AuthorizationGuestInputV1, CapabilityRequestBindingV1,
    MAX_CANONICAL_POLICY_BYTES, TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS, agent_subject,
};
use raziel_canonical::{StoredCaveat, compare_relationship_identity, encode_policy, policy_hash};
use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ContextEntry, ObjectDefinition, PermissionDefinition,
    PermissionExpr, PolicyProgram, RelationDefinition, ValidatedPolicyProgram,
};
use raziel_prover::{MAX_EXECUTION_CYCLE_LIMIT, Sp1Executor, prepare_guest_input};
use raziel_snapshot::{
    RELATIONSHIP_WITNESS_SET_VERSION, RelationshipRecord, RelationshipWitnessSetV1, SnapshotTree,
    verify_witness_set,
};

mod support;

use support::{KEY_THUMBPRINT, SNAPSHOT_SALT, guest_elf_path};

const MAX_DEFINITIONS: usize = 16;
const MAX_RELATIONS_PER_DEFINITION: usize = 16;
const MAX_ALLOWED_SUBJECT_TYPES: usize = 16;
const MAX_PERMISSIONS: usize = 32;
const MAX_CAVEATS: usize = 16;
const MAX_PARAMETERS_PER_CAVEAT: usize = 16;

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn accepted_ceiling_input_stays_below_the_execution_cycle_limit() {
    execute_fixture("accepted-ceiling", ceiling_fixture());
}

#[test]
#[ignore = "requires a prior pinned cargo prove build of the authorization guest"]
fn accepted_maximum_snapshot_depth_stays_below_the_execution_cycle_limit() {
    execute_fixture("maximum-snapshot-depth", maximum_snapshot_depth_fixture());
}

fn execute_fixture(case_id: &str, fixture: CeilingFixture) {
    let guest_elf = fs::read(guest_elf_path()).expect("authorization guest ELF must exist");
    let prepared = prepare_guest_input(fixture.input)
        .expect("accepted ceiling input must prepare on the host");
    let report = Sp1Executor::local()
        .execute(&guest_elf, &prepared)
        .expect("accepted ceiling input must execute in SP1");

    assert_eq!(
        report.public_values,
        prepared.expected_public_values_bytes(),
        "ceiling input public journal mismatch"
    );
    assert!(report.instruction_count > 0);
    assert!(
        report.instruction_count < MAX_EXECUTION_CYCLE_LIMIT,
        "ceiling input reached the compiled execution limit"
    );
    eprintln!(
        "{case_id}: policy_bytes={}, private_input_bytes={}, witnesses={}, merkle_depth={}, instructions={}",
        fixture.canonical_policy_bytes,
        fixture.private_input_bytes,
        fixture.relationship_count,
        fixture.max_merkle_depth,
        report.instruction_count
    );
}

struct CeilingFixture {
    input: AuthorizationGuestInputV1,
    canonical_policy_bytes: usize,
    private_input_bytes: u32,
    relationship_count: u32,
    max_merkle_depth: u32,
}

fn ceiling_fixture() -> CeilingFixture {
    let (policy, canonical_policy_bytes) = largest_accepted_policy();
    let relationships = ceiling_relationships(&policy, "p");
    let tree = SnapshotTree::build(
        &policy,
        relationships,
        &SNAPSHOT_SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .expect("ceiling snapshot must build");
    let relationship_snapshot_root = tree.root();
    let relationship_witness_set = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: SNAPSHOT_SALT,
        leaf_count: MAX_RELATIONS_PER_DEFINITION as u32,
        entries: (0..MAX_RELATIONS_PER_DEFINITION as u32)
            .map(|index| tree.opening(index).expect("ceiling opening must exist"))
            .collect(),
    };
    let metrics = verify_witness_set(
        &policy,
        &relationship_witness_set,
        &relationship_snapshot_root,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .expect("ceiling witness set must verify");

    assert_eq!(
        metrics.relationship_count,
        MAX_RELATIONS_PER_DEFINITION as u32
    );
    assert_eq!(metrics.max_merkle_depth, 4);
    assert!(
        metrics.private_input_bytes
            <= TRUSTED_POLICY_CEILINGS
                .policy_limits
                .max_total_private_input_bytes
    );

    CeilingFixture {
        input: AuthorizationGuestInputV1 {
            format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
            expected_policy_hash: policy_hash(&policy).expect("ceiling policy must hash"),
            relationship_snapshot_root,
            relationship_witness_set,
            policy: policy.program().clone(),
            request: ceiling_request(&policy, "p00".into(), "p".into()),
            issued_at_unix_seconds: 1_000,
            expires_at_unix_seconds: 1_800,
        },
        canonical_policy_bytes,
        private_input_bytes: metrics.private_input_bytes,
        relationship_count: metrics.relationship_count,
        max_merkle_depth: metrics.max_merkle_depth,
    }
}

fn maximum_snapshot_depth_fixture() -> CeilingFixture {
    let (policy, canonical_policy_bytes) = largest_accepted_policy();
    let leaf_count = TRUSTED_SNAPSHOT_LIMITS.max_relationships;
    let subject = agent_subject(&KEY_THUMBPRINT);
    let mut relationships = (0..leaf_count - 3)
        .map(|index| RelationshipRecord {
            resource_type: "d".into(),
            resource_id: format!("p{index:03}"),
            relation: "r01".into(),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: None,
        })
        .collect::<Vec<_>>();
    relationships.extend(path_relationships(&policy, "target"));
    relationships.sort_by(compare_relationship_identity);
    let target_indices = relationships
        .iter()
        .enumerate()
        .filter_map(|(index, relationship)| {
            matches!(
                (
                    relationship.resource_id.as_str(),
                    relationship.relation.as_str()
                ),
                ("target", "r00") | ("node", "r00") | ("group", "r00")
            )
            .then_some(index as u32)
        })
        .collect::<Vec<_>>();
    assert_eq!(target_indices.len(), 3);
    let tree = SnapshotTree::build(
        &policy,
        relationships,
        &SNAPSHOT_SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .expect("maximum-depth snapshot must build");
    let relationship_snapshot_root = tree.root();
    let relationship_witness_set = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: SNAPSHOT_SALT,
        leaf_count,
        entries: target_indices
            .into_iter()
            .map(|index| {
                tree.opening(index)
                    .expect("maximum-depth opening must exist")
            })
            .collect(),
    };
    let metrics = verify_witness_set(
        &policy,
        &relationship_witness_set,
        &relationship_snapshot_root,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .expect("maximum-depth witness set must verify");
    assert_eq!(metrics.relationship_count, 3);
    assert_eq!(metrics.max_merkle_depth, 8);

    let action = policy
        .program()
        .permissions
        .iter()
        .find(|permission| matches!(permission.expression, PermissionExpr::Arrow { .. }))
        .expect("arrow ceiling permission must exist")
        .name
        .clone();
    CeilingFixture {
        input: AuthorizationGuestInputV1 {
            format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
            expected_policy_hash: policy_hash(&policy).expect("ceiling policy must hash"),
            relationship_snapshot_root,
            relationship_witness_set,
            policy: policy.program().clone(),
            request: ceiling_request(&policy, action, "target".into()),
            issued_at_unix_seconds: 1_000,
            expires_at_unix_seconds: 1_800,
        },
        canonical_policy_bytes,
        private_input_bytes: metrics.private_input_bytes,
        relationship_count: metrics.relationship_count,
        max_merkle_depth: metrics.max_merkle_depth,
    }
}

fn ceiling_request(
    policy: &ValidatedPolicyProgram,
    action: String,
    resource_scope: String,
) -> CapabilityRequestBindingV1 {
    let max_string_bytes = TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize;
    let request_context: Vec<_> = policy
        .program()
        .caveats
        .iter()
        .find(|caveat| caveat.name == "c00")
        .expect("ceiling caveat must exist")
        .parameters
        .iter()
        .map(|parameter| ContextEntry {
            name: parameter.name.clone(),
            value: CaveatValue::String("v".repeat(max_string_bytes)),
        })
        .collect();
    assert_eq!(request_context.len(), MAX_PARAMETERS_PER_CAVEAT);
    assert!(request_context.iter().all(|entry| {
        matches!(
            &entry.value,
            CaveatValue::String(value) if value.len() == max_string_bytes
        )
    }));
    CapabilityRequestBindingV1 {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_key_thumbprint: KEY_THUMBPRINT,
        task_id: "t".repeat(max_string_bytes),
        purpose: "u".repeat(max_string_bytes),
        action,
        resource_type: "d".into(),
        resource_scope,
        requested_expires_at_unix_seconds: 2_000,
        issuance_nonce: [0x73; 32],
        snapshot_id: "s".repeat(max_string_bytes),
        request_context,
    }
}

fn largest_accepted_policy() -> (ValidatedPolicyProgram, usize) {
    let mut low = 3_usize;
    let mut high = TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize;
    let mut best = None;

    while low <= high {
        let candidate = low + (high - low) / 2;
        let policy = validated_policy(candidate);
        let encoded = encode_policy(&policy).expect("ceiling policy must encode");
        if encoded.len() <= MAX_CANONICAL_POLICY_BYTES {
            best = Some((candidate, encoded.len()));
            low = candidate + 1;
        } else {
            high = candidate - 1;
        }
    }

    let (long_name_bytes, canonical_policy_bytes) =
        best.expect("at least the shortest ceiling policy must fit");
    let policy = validated_policy(long_name_bytes);
    let string_byte_limit = TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize;
    if long_name_bytes < string_byte_limit {
        assert!(
            canonical_policy_bytes > MAX_CANONICAL_POLICY_BYTES - 10_000,
            "ceiling policy is not close enough to the canonical byte limit"
        );
        let next = validated_policy(long_name_bytes + 1);
        assert!(
            encode_policy(&next).expect("next policy must encode").len()
                > MAX_CANONICAL_POLICY_BYTES,
            "binary search did not select the largest accepted policy"
        );
    }
    (policy, canonical_policy_bytes)
}

fn validated_policy(long_name_bytes: usize) -> ValidatedPolicyProgram {
    ValidatedPolicyProgram::new(ceiling_policy(long_name_bytes), &TRUSTED_POLICY_CEILINGS)
        .expect("ceiling policy must satisfy every structural limit")
}

fn ceiling_policy(long_name_bytes: usize) -> PolicyProgram {
    let object_types = object_types(long_name_bytes);
    assert_eq!(object_types.len(), MAX_ALLOWED_SUBJECT_TYPES);
    let arrow_target_type = object_types[2].clone();
    let subject_set_type = object_types[3].clone();
    let filler_type = object_types[4].clone();
    let mut allowed_subject_types = object_types
        .iter()
        .filter(|object_type| {
            **object_type != arrow_target_type && **object_type != subject_set_type
        })
        .map(|object_type| AllowedSubjectType {
            object_type: object_type.clone(),
            subject_relation: None,
            caveat_name: None,
        })
        .collect::<Vec<_>>();
    allowed_subject_types.extend([
        AllowedSubjectType {
            object_type: filler_type.clone(),
            subject_relation: None,
            caveat_name: Some("c00".into()),
        },
        AllowedSubjectType {
            object_type: filler_type,
            subject_relation: Some(padded_identifier("r00", long_name_bytes)),
            caveat_name: None,
        },
    ]);
    allowed_subject_types.sort();
    assert_eq!(allowed_subject_types.len(), MAX_ALLOWED_SUBJECT_TYPES);
    let definitions = object_types
        .into_iter()
        .map(|name| ObjectDefinition {
            relations: ceiling_relations(
                &name,
                long_name_bytes,
                &allowed_subject_types,
                &arrow_target_type,
                &subject_set_type,
            ),
            name,
        })
        .collect();

    let program = PolicyProgram {
        ir_version: 1,
        compiler_name: "c".repeat(TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize),
        compiler_version: "v"
            .repeat(TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize),
        source_schema_digest: [0x84; 32],
        definitions,
        permissions: ceiling_permissions(long_name_bytes, &arrow_target_type),
        caveats: ceiling_caveats(long_name_bytes),
        limits: TRUSTED_POLICY_CEILINGS.policy_limits,
    };
    let selected_permission = program
        .permissions
        .iter()
        .find(|permission| permission.object_type == "d" && permission.name == "p00")
        .expect("selected ceiling permission must exist");
    assert_eq!(permission_node_count(&selected_permission.expression), 33);
    assert_eq!(
        program
            .permissions
            .iter()
            .map(|permission| permission_node_count(&permission.expression))
            .sum::<u32>(),
        TRUSTED_POLICY_CEILINGS.policy_limits.max_expression_nodes
    );
    let active_caveat = program
        .caveats
        .iter()
        .find(|caveat| caveat.name == "c00")
        .expect("active ceiling caveat must exist");
    assert_eq!(caveat_node_count(&active_caveat.expression), 34);
    assert_eq!(
        program
            .caveats
            .iter()
            .map(|caveat| caveat_node_count(&caveat.expression))
            .sum::<u32>(),
        TRUSTED_POLICY_CEILINGS.policy_limits.max_caveat_nodes
    );
    program
}

fn ceiling_relations(
    definition_name: &str,
    long_name_bytes: usize,
    allowed_subject_types: &[AllowedSubjectType],
    arrow_target_type: &str,
    subject_set_type: &str,
) -> Vec<RelationDefinition> {
    (0..MAX_RELATIONS_PER_DEFINITION)
        .map(|index| {
            let mut allowed_subject_types = allowed_subject_types.to_vec();
            if definition_name == "d" && index == 0 {
                allowed_subject_types = vec![AllowedSubjectType {
                    object_type: arrow_target_type.into(),
                    subject_relation: None,
                    caveat_name: None,
                }];
            } else if definition_name == "d" && index == MAX_RELATIONS_PER_DEFINITION - 1 {
                allowed_subject_types
                    .iter_mut()
                    .find(|allowed| allowed.object_type == "agent")
                    .expect("agent subject type must exist")
                    .caveat_name = Some("c00".into());
            } else if definition_name == arrow_target_type && index == 0 {
                allowed_subject_types.retain(|allowed| allowed.object_type != "d");
                allowed_subject_types.push(AllowedSubjectType {
                    object_type: subject_set_type.into(),
                    subject_relation: Some("r00".into()),
                    caveat_name: None,
                });
                allowed_subject_types.sort();
            }
            RelationDefinition {
                name: if definition_name == "d"
                    || (index == 0
                        && (definition_name == arrow_target_type
                            || definition_name == subject_set_type))
                {
                    format!("r{index:02}")
                } else {
                    padded_identifier(&format!("r{index:02}"), long_name_bytes)
                },
                allowed_subject_types,
            }
        })
        .collect()
}

fn object_types(long_name_bytes: usize) -> Vec<String> {
    let mut object_types = vec!["agent".into(), "d".into(), "x00".into(), "x01".into()];
    object_types.extend(
        (2..MAX_DEFINITIONS - 2)
            .map(|index| padded_identifier(&format!("x{index:02}"), long_name_bytes)),
    );
    object_types
}

fn ceiling_permissions(
    long_name_bytes: usize,
    arrow_target_type: &str,
) -> Vec<PermissionDefinition> {
    let arrow_target_permission = padded_identifier("z31", long_name_bytes);
    let mut permissions = (0..MAX_PERMISSIONS - 1)
        .map(|index| PermissionDefinition {
            object_type: "d".into(),
            name: if index == 0 {
                "p00".into()
            } else {
                padded_identifier(&format!("z{index:02}"), long_name_bytes)
            },
            expression: match index {
                0 => ceiling_permission_expression(&arrow_target_permission),
                1 => PermissionExpr::Arrow {
                    source_relation: "r00".into(),
                    target_permission: arrow_target_permission.clone(),
                },
                _ => relation_ref(1),
            },
        })
        .collect::<Vec<_>>();
    permissions.push(PermissionDefinition {
        object_type: arrow_target_type.into(),
        name: arrow_target_permission,
        expression: PermissionExpr::SubjectRelationRef {
            relation: "r00".into(),
            subject_relation: "r00".into(),
        },
    });
    permissions
}

fn ceiling_permission_expression(arrow_target_permission: &str) -> PermissionExpr {
    let mut children = (2..=13).map(relation_ref).collect::<Vec<_>>();
    children.extend([
        PermissionExpr::AllOf(vec![relation_ref(2), relation_ref(3)]),
        PermissionExpr::AllOf(vec![relation_ref(4), relation_ref(5)]),
        PermissionExpr::AllOf(vec![relation_ref(6), relation_ref(7), relation_ref(8)]),
        PermissionExpr::AllOf(vec![relation_ref(8), relation_ref(9), relation_ref(10)]),
        PermissionExpr::AllOf(vec![relation_ref(11), relation_ref(12), relation_ref(13)]),
        PermissionExpr::Arrow {
            source_relation: "r00".into(),
            target_permission: arrow_target_permission.into(),
        },
        PermissionExpr::CaveatedRelation {
            relation: "r15".into(),
            caveat_name: "c00".into(),
        },
    ]);
    PermissionExpr::AllOf(children)
}

fn relation_ref(index: usize) -> PermissionExpr {
    PermissionExpr::RelationRef {
        name: format!("r{index:02}"),
    }
}

fn permission_node_count(expression: &PermissionExpr) -> u32 {
    match expression {
        PermissionExpr::AnyOf(children) | PermissionExpr::AllOf(children) => {
            1 + children.iter().map(permission_node_count).sum::<u32>()
        }
        PermissionExpr::RelationRef { .. }
        | PermissionExpr::SubjectRelationRef { .. }
        | PermissionExpr::Arrow { .. }
        | PermissionExpr::CaveatedRelation { .. } => 1,
    }
}

fn ceiling_caveats(long_name_bytes: usize) -> Vec<CaveatDefinition> {
    (0..MAX_CAVEATS)
        .map(|index| CaveatDefinition {
            name: if index == 0 {
                "c00".into()
            } else {
                padded_identifier(&format!("c{index:02}"), long_name_bytes)
            },
            parameters: (0..MAX_PARAMETERS_PER_CAVEAT)
                .map(|parameter| CaveatParameter {
                    source: CaveatInputSource::RequestContext,
                    name: padded_identifier(&format!("q{parameter:02}"), long_name_bytes),
                    value_type: CaveatValueType::String,
                })
                .collect(),
            expression: match index {
                0 => ceiling_caveat_expression(long_name_bytes),
                1 => deepest_caveat_expression(15),
                _ => CaveatExpr::ConstBool(true),
            },
        })
        .collect()
}

fn padded_identifier(prefix: &str, bytes: usize) -> String {
    format!("{prefix}{}", "a".repeat(bytes - prefix.len()))
}

fn ceiling_caveat_expression(long_name_bytes: usize) -> CaveatExpr {
    let pairs = [
        (0, 0),
        (0, 1),
        (0, 2),
        (2, 3),
        (4, 5),
        (6, 7),
        (8, 9),
        (10, 11),
        (12, 13),
        (14, 14),
        (14, 15),
    ];
    let mut referenced = pairs
        .iter()
        .flat_map(|(left, right)| [*left, *right])
        .collect::<Vec<_>>();
    referenced.sort_unstable();
    referenced.dedup();
    assert_eq!(
        referenced,
        (0..MAX_PARAMETERS_PER_CAVEAT).collect::<Vec<_>>()
    );
    CaveatExpr::And(
        pairs
            .into_iter()
            .map(|(left, right)| {
                CaveatExpr::Equal(
                    Box::new(request_context_ref(left, long_name_bytes)),
                    Box::new(request_context_ref(right, long_name_bytes)),
                )
            })
            .collect(),
    )
}

fn request_context_ref(index: usize, long_name_bytes: usize) -> CaveatExpr {
    CaveatExpr::InputRef {
        source: CaveatInputSource::RequestContext,
        name: padded_identifier(&format!("q{index:02}"), long_name_bytes),
        value_type: CaveatValueType::String,
    }
}

fn caveat_node_count(expression: &CaveatExpr) -> u32 {
    match expression {
        CaveatExpr::Equal(left, right)
        | CaveatExpr::NotEqual(left, right)
        | CaveatExpr::LessThan(left, right)
        | CaveatExpr::LessThanOrEqual(left, right)
        | CaveatExpr::GreaterThan(left, right)
        | CaveatExpr::GreaterThanOrEqual(left, right) => {
            1 + caveat_node_count(left) + caveat_node_count(right)
        }
        CaveatExpr::And(children) | CaveatExpr::Or(children) => {
            1 + children.iter().map(caveat_node_count).sum::<u32>()
        }
        CaveatExpr::Not(child) => 1 + caveat_node_count(child),
        CaveatExpr::ConstBool(_)
        | CaveatExpr::ConstInt(_)
        | CaveatExpr::ConstString(_)
        | CaveatExpr::InputRef { .. } => 1,
    }
}

fn deepest_caveat_expression(depth: usize) -> CaveatExpr {
    if depth == 0 {
        CaveatExpr::ConstBool(true)
    } else {
        CaveatExpr::Not(Box::new(deepest_caveat_expression(depth - 1)))
    }
}

fn ceiling_relationships(
    policy: &ValidatedPolicyProgram,
    resource_scope: &str,
) -> Vec<RelationshipRecord> {
    let subject = agent_subject(&KEY_THUMBPRINT);
    let mut relationships = (2..=13)
        .map(|index| RelationshipRecord {
            resource_type: "d".into(),
            resource_id: resource_scope.into(),
            relation: format!("r{index:02}"),
            subject_type: subject.object_type.clone(),
            subject_id: subject.object_id.clone(),
            subject_relation: None,
            stored_caveat: None,
        })
        .collect::<Vec<_>>();
    relationships.push(RelationshipRecord {
        resource_type: "d".into(),
        resource_id: resource_scope.into(),
        relation: "r15".into(),
        subject_type: subject.object_type,
        subject_id: subject.object_id,
        subject_relation: None,
        stored_caveat: Some(StoredCaveat {
            name: "c00".into(),
            context: vec![],
        }),
    });
    relationships.extend(path_relationships(policy, resource_scope));
    relationships.sort_by(compare_relationship_identity);
    assert_eq!(relationships.len(), MAX_RELATIONS_PER_DEFINITION);
    relationships
}

fn path_relationships(
    policy: &ValidatedPolicyProgram,
    resource_scope: &str,
) -> Vec<RelationshipRecord> {
    let subject = agent_subject(&KEY_THUMBPRINT);
    let data_definition = policy
        .program()
        .definitions
        .iter()
        .find(|definition| definition.name == "d")
        .expect("data definition must exist");
    let arrow_relation = data_definition
        .relations
        .iter()
        .find(|relation| relation.name == "r00")
        .expect("arrow relation must exist");
    let arrow_target_type = arrow_relation.allowed_subject_types[0].object_type.clone();
    let subject_set_relation = policy
        .program()
        .definitions
        .iter()
        .find(|definition| definition.name == arrow_target_type)
        .expect("arrow target definition must exist")
        .relations
        .iter()
        .find(|relation| relation.name == "r00")
        .expect("subject-set relation must exist");
    let subject_set_type = subject_set_relation
        .allowed_subject_types
        .iter()
        .find(|allowed| allowed.subject_relation.as_deref() == Some("r00"))
        .expect("subject-set target must exist")
        .object_type
        .clone();

    vec![
        RelationshipRecord {
            resource_type: "d".into(),
            resource_id: resource_scope.into(),
            relation: "r00".into(),
            subject_type: arrow_target_type.clone(),
            subject_id: "node".into(),
            subject_relation: None,
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: arrow_target_type,
            resource_id: "node".into(),
            relation: "r00".into(),
            subject_type: subject_set_type.clone(),
            subject_id: "group".into(),
            subject_relation: Some("r00".into()),
            stored_caveat: None,
        },
        RelationshipRecord {
            resource_type: subject_set_type,
            resource_id: "group".into(),
            relation: "r00".into(),
            subject_type: subject.object_type,
            subject_id: subject.object_id,
            subject_relation: None,
            stored_caveat: None,
        },
    ]
}
