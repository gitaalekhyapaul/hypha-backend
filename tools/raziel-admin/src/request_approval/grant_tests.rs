use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatParameter, ObjectDefinition,
    PermissionDefinition, PolicyProgram, RelationDefinition,
};

use crate::policy_package::{PolicyPackageKey, PolicyPackageStore};
use crate::policy_publication::catalogue::tests::{Fixture, NOW};

use super::*;

const KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

fn version() -> String {
    "b".repeat(32)
}
fn root() -> String {
    format!("dataset_{}", version())
}
fn agent() -> String {
    crate::io::lowercase_hex(&validate_agent_api_key(&KEY).unwrap())
}
fn grant() -> proto::AccessGrantInputs {
    proto::AccessGrantInputs {
        format_version: 1,
        team_id: "finance-team".into(),
        organization_id: "bank-org".into(),
        approved_amount: 45000,
        is_approved: true,
        is_suspended: false,
        is_override: true,
    }
}
fn original() -> proto::CreateAccessRequestRequest {
    proto::CreateAccessRequestRequest {
        owner: Some(proto::AccessRequestOwner {
            agent_id: "d".repeat(32),
            agent_public_key: KEY.to_vec(),
        }),
        intent: Some(proto::AccessIntent {
            format_version: 1,
            request_id: "11111111-1111-4111-8111-111111111111".into(),
            task_id: "reconciliation".into(),
            purpose: "match settlements".into(),
            action: "query".into(),
            dataset_id: "ledger".into(),
            dataset_revision: 3,
            request_context: vec![proto::AuthorizationContextEntry {
                name: "requested_amount".into(),
                value: Some(proto::authorization_context_entry::Value::IntValue(123)),
            }],
        }),
        dataset: Some(proto::AccessDatasetRevision {
            dataset_id: "ledger".into(),
            revision: 3,
            checksum: vec![0x33; 32],
        }),
    }
}

fn direct(name: &str) -> PermissionExpr {
    PermissionExpr::RelationRef { name: name.into() }
}
fn allowed(kind: &str, subject_relation: Option<&str>, caveat: Option<&str>) -> AllowedSubjectType {
    AllowedSubjectType {
        object_type: kind.into(),
        subject_relation: subject_relation.map(Into::into),
        caveat_name: caveat.map(Into::into),
    }
}
fn relation(name: &str, subject: AllowedSubjectType) -> RelationDefinition {
    RelationDefinition {
        name: name.into(),
        allowed_subject_types: vec![subject],
    }
}
fn program(expression: PermissionExpr) -> PolicyProgram {
    PolicyProgram {
        ir_version: 1,
        compiler_name: "authzed/spicedb".into(),
        compiler_version: "1.54.0".into(),
        source_schema_digest: [0x42; 32],
        definitions: vec![
            ObjectDefinition {
                name: "agent".into(),
                relations: vec![],
            },
            ObjectDefinition {
                name: root(),
                relations: vec![relation("viewer", allowed("agent", None, None))],
            },
        ],
        permissions: vec![PermissionDefinition {
            object_type: root(),
            name: "query".into(),
            expression,
        }],
        caveats: vec![],
        limits: TRUSTED_POLICY_CEILINGS.policy_limits,
    }
}
fn validated(mut policy: PolicyProgram) -> ValidatedPolicyProgram {
    policy.definitions.sort_by(|a, b| a.name.cmp(&b.name));
    for definition in &mut policy.definitions {
        definition.relations.sort();
    }
    policy
        .permissions
        .sort_by(|a, b| (&a.object_type, &a.name).cmp(&(&b.object_type, &b.name)));
    for caveat in &mut policy.caveats {
        caveat.parameters.sort();
    }
    ValidatedPolicyProgram::new(policy, &TRUSTED_POLICY_CEILINGS).unwrap()
}
fn rows(policy: PolicyProgram) -> Vec<RelationshipRecord> {
    materialize(&validated(policy), &version(), "ledger", &agent(), &grant()).unwrap()
}

#[test]
fn public_builder_uses_registered_key_and_immutable_dataset() {
    let mut fixture = Fixture::new();
    fixture.retain('b', 1, 1);
    let package = PolicyPackageStore::new(
        fixture.config.package_root.clone(),
        fixture.config.registry_root.clone(),
        &fixture.config.trust_domain_id,
    )
    .unwrap()
    .read_approved(&PolicyPackageKey::new(version(), 1).unwrap(), NOW)
    .unwrap();
    let original = original();
    let records = build_grant(&package, &original, &grant()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].resource_type, root());
    assert_eq!(records[0].resource_id, "ledger");
    assert_eq!(records[0].subject_id, agent());
    assert_eq!(records[0].subject_type, "agent");
    assert_ne!(records[0].subject_id, original.owner.unwrap().agent_id);
    assert_eq!(records[0].relation, "viewer");
    assert!(records[0].stored_caveat.is_none());
}

#[test]
fn public_builder_rejects_invalid_identity_metadata_and_grant_bounds() {
    let mut fixture = Fixture::new();
    fixture.retain('b', 1, 1);
    let package = PolicyPackageStore::new(
        fixture.config.package_root.clone(),
        fixture.config.registry_root.clone(),
        &fixture.config.trust_domain_id,
    )
    .unwrap()
    .read_approved(&PolicyPackageKey::new(version(), 1).unwrap(), NOW)
    .unwrap();
    let mutations: &[fn(&mut proto::CreateAccessRequestRequest)] = &[
        |r| r.owner.as_mut().unwrap().agent_public_key = vec![0; 32],
        |r| r.owner.as_mut().unwrap().agent_public_key = vec![0xff; 32],
        |r| {
            r.owner
                .as_mut()
                .unwrap()
                .agent_public_key
                .pop()
                .map(|_| ())
                .unwrap()
        },
        |r| r.owner.as_mut().unwrap().agent_id = "invalid".into(),
        |r| r.intent.as_mut().unwrap().request_id = "not-a-uuid".into(),
        |r| r.intent.as_mut().unwrap().action = "write".into(),
        |r| r.intent.as_mut().unwrap().dataset_id = "different".into(),
        |r| r.intent.as_mut().unwrap().dataset_revision += 1,
        |r| r.dataset.as_mut().unwrap().checksum.clear(),
        |r| r.dataset = None,
    ];
    for mutate in mutations {
        let mut input = original();
        mutate(&mut input);
        assert!(build_grant(&package, &input, &grant()).is_err());
    }
    let mutations: &[fn(&mut proto::AccessGrantInputs)] = &[
        |g| g.format_version = 2,
        |g| g.team_id = "../team".into(),
        |g| g.organization_id = "o".repeat(129),
        |g| g.approved_amount = 0,
        |g| g.approved_amount = 9_007_199_254_740_992,
    ];
    for mutate in mutations {
        let mut input = grant();
        mutate(&mut input);
        assert_eq!(
            build_grant(&package, &original(), &input),
            Err(ApprovalError::Invalid)
        );
    }
}

#[test]
fn direct_conjunction_and_alternative_paths_materialize_every_leaf() {
    assert_eq!(rows(program(direct("viewer"))).len(), 1);
    let children = vec![direct("owner"), direct("viewer")];
    for expr in [
        PermissionExpr::AllOf(children.clone()),
        PermissionExpr::AnyOf(children),
    ] {
        let mut p = program(expr);
        p.definitions[1]
            .relations
            .push(relation("owner", allowed("agent", None, None)));
        let records = rows(p);
        assert_eq!(
            records
                .iter()
                .map(|r| r.relation.as_str())
                .collect::<Vec<_>>(),
            vec!["owner", "viewer"]
        );
        assert!(
            records
                .iter()
                .all(|r| r.subject_id == agent() && r.resource_id == "ledger")
        );
    }
}

#[test]
fn team_membership_uses_namespaced_type_and_private_team_id() {
    let team = format!("team_{}", version());
    let mut p = program(PermissionExpr::SubjectRelationRef {
        relation: "team_viewer".into(),
        subject_relation: "member".into(),
    });
    p.definitions[1].relations = vec![relation(
        "team_viewer",
        allowed(&team, Some("member"), None),
    )];
    p.definitions.push(ObjectDefinition {
        name: team.clone(),
        relations: vec![relation("member", allowed("agent", None, None))],
    });
    let records = rows(p);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].subject_type, team);
    assert_eq!(records[0].subject_relation.as_deref(), Some("member"));
    assert_eq!(records[0].subject_id, "finance-team");
    assert_eq!(records[1].resource_type, team);
    assert_eq!(records[1].resource_id, "finance-team");
    assert_eq!(records[1].subject_id, agent());
}

#[test]
fn organization_delegation_uses_namespaced_type_and_private_organization_id() {
    let org = format!("organization_{}", version());
    let mut p = program(PermissionExpr::Arrow {
        source_relation: "organization".into(),
        target_permission: "audit".into(),
    });
    p.definitions[1].relations = vec![relation("organization", allowed(&org, None, None))];
    p.definitions.push(ObjectDefinition {
        name: org.clone(),
        relations: vec![relation("auditor", allowed("agent", None, None))],
    });
    p.permissions.push(PermissionDefinition {
        object_type: org.clone(),
        name: "audit".into(),
        expression: direct("auditor"),
    });
    let records = rows(p);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].subject_type, org);
    assert_eq!(records[0].subject_id, "bank-org");
    assert_eq!(records[1].resource_type, org);
    assert_eq!(records[1].resource_id, "bank-org");
    assert_eq!(records[1].relation, "auditor");
    assert_eq!(records[1].subject_id, agent());
}

fn caveat_policy(parameters: Vec<CaveatParameter>) -> PolicyProgram {
    let caveat = format!("caveat_{}_0", version());
    let mut p = program(PermissionExpr::CaveatedRelation {
        relation: "viewer".into(),
        caveat_name: caveat.clone(),
    });
    p.definitions[1].relations[0] = relation("viewer", allowed("agent", None, Some(&caveat)));
    // Materialization is valid even for a policy expression that always denies.
    p.caveats.push(CaveatDefinition {
        name: caveat,
        parameters,
        expression: CaveatExpr::ConstBool(false),
    });
    p
}
fn parameter(
    name: &str,
    source: CaveatInputSource,
    value_type: CaveatValueType,
) -> CaveatParameter {
    CaveatParameter {
        name: name.into(),
        source,
        value_type,
    }
}

#[test]
fn all_caveat_templates_copy_only_relationship_parameters() {
    use CaveatInputSource::{
        CapabilityClaim as Claim, RelationshipContext as Stored, RequestContext as Request,
    };
    use CaveatValueType::{Bool, Int, String as Text};
    let amount = parameter("approved_amount", Stored, Int);
    let requested = parameter("requested_amount", Request, Int);
    let approved = parameter("is_approved", Stored, Bool);
    let scope = vec![
        parameter("task_id", Claim, Text),
        parameter("purpose", Claim, Text),
        parameter("resource_scope", Claim, Text),
    ];
    let mut reconciliation = scope.clone();
    reconciliation.extend([amount.clone(), requested.clone(), approved.clone()]);
    for parameters in [
        vec![amount.clone(), requested.clone()],
        vec![amount, requested, parameter("is_override", Stored, Bool)],
        vec![approved, parameter("is_suspended", Stored, Bool)],
        scope,
        reconciliation,
    ] {
        let expected = parameters.iter().filter(|p| p.source == Stored).count();
        let records = rows(caveat_policy(parameters));
        let caveat = records[0].stored_caveat.as_ref().unwrap();
        assert_eq!(caveat.context.len(), expected);
        for entry in &caveat.context {
            let expected = match entry.name.as_str() {
                "approved_amount" => CaveatValue::Int(45000),
                "is_approved" | "is_override" => CaveatValue::Bool(true),
                "is_suspended" => CaveatValue::Bool(false),
                _ => panic!("unexpected context source"),
            };
            assert_eq!(entry.value, expected);
        }
    }
}

#[test]
fn unknown_and_mistyped_stored_parameters_fail_closed() {
    for p in [
        parameter(
            "unknown",
            CaveatInputSource::RelationshipContext,
            CaveatValueType::Bool,
        ),
        parameter(
            "approved_amount",
            CaveatInputSource::RelationshipContext,
            CaveatValueType::String,
        ),
    ] {
        let policy = validated(caveat_policy(vec![p]));
        assert_eq!(
            materialize(&policy, &version(), "ledger", &agent(), &grant()),
            Err(ApprovalError::Invalid)
        );
    }
}

#[test]
fn records_deduplicate_identical_values_but_reject_conflicting_identity() {
    let p = validated(caveat_policy(vec![parameter(
        "is_approved",
        CaveatInputSource::RelationshipContext,
        CaveatValueType::Bool,
    )]));
    let records = materialize(&p, &version(), "ledger", &agent(), &grant()).unwrap();
    assert_eq!(
        finish(&p, vec![records[0].clone(), records[0].clone()]).unwrap(),
        records
    );
    let mut conflicting = records[0].clone();
    conflicting.stored_caveat.as_mut().unwrap().context[0].value = CaveatValue::Bool(false);
    assert_eq!(
        finish(&p, vec![records[0].clone(), conflicting]),
        Err(ApprovalError::Conflict)
    );
}

#[test]
fn snapshot_string_and_relationship_count_limits_are_not_relaxed() {
    let p = validated(program(direct("viewer")));
    let records = materialize(&p, &version(), "ledger", &agent(), &grant()).unwrap();
    assert_eq!(
        finish(&p, vec![records[0].clone(); 257]),
        Err(ApprovalError::Invalid)
    );
    let mut invalid = records;
    invalid[0].subject_id = "a".repeat(257);
    assert_eq!(finish(&p, invalid), Err(ApprovalError::Invalid));
    assert_eq!(p.program().limits.max_total_private_input_bytes, 4096);
}
