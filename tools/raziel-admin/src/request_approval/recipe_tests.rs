use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_policy_ir::{
    CaveatDefinition, CaveatExpr, CaveatParameter, PermissionDefinition, RelationDefinition,
};

use super::*;

fn base() -> PolicyProgram {
    PolicyProgram {
        ir_version: 1,
        compiler_name: "authzed/spicedb".into(),
        compiler_version: "1.54.0".into(),
        source_schema_digest: [0; 32],
        definitions: vec![
            ObjectDefinition {
                name: "agent".into(),
                relations: vec![],
            },
            ObjectDefinition {
                name: format!("dataset_{}", "b".repeat(32)),
                relations: vec![RelationDefinition {
                    name: "viewer".into(),
                    allowed_subject_types: vec![direct_subject(None)],
                }],
            },
        ],
        permissions: vec![PermissionDefinition {
            object_type: format!("dataset_{}", "b".repeat(32)),
            name: "query".into(),
            expression: PermissionExpr::RelationRef {
                name: "viewer".into(),
            },
        }],
        caveats: vec![],
        limits: TRUSTED_POLICY_CEILINGS.policy_limits,
    }
}

fn caveated(source: CaveatInputSource, name: &str, value_type: CaveatValueType) -> PolicyProgram {
    let mut policy = base();
    let caveat_name = format!("caveat_{}_0", "b".repeat(32));
    policy.definitions[1].relations[0].allowed_subject_types[0].caveat_name =
        Some(caveat_name.clone());
    policy.permissions[0].expression = PermissionExpr::CaveatedRelation {
        relation: "viewer".into(),
        caveat_name: caveat_name.clone(),
    };
    policy.caveats.push(CaveatDefinition {
        name: caveat_name,
        parameters: vec![CaveatParameter {
            source,
            name: name.into(),
            value_type,
        }],
        expression: CaveatExpr::ConstBool(false),
    });
    policy
}

#[test]
fn direct_and_all_supported_stored_inputs_are_representable_without_evaluation() {
    assert!(
        validate_program(&base(), &"b".repeat(32))
            .unwrap()
            .is_empty()
    );
    for (name, kind, field) in [
        (
            "approved_amount",
            CaveatValueType::Int,
            GrantInputField::ApprovedAmount,
        ),
        (
            "is_approved",
            CaveatValueType::Bool,
            GrantInputField::IsApproved,
        ),
        (
            "is_suspended",
            CaveatValueType::Bool,
            GrantInputField::IsSuspended,
        ),
        (
            "is_override",
            CaveatValueType::Bool,
            GrantInputField::IsOverride,
        ),
    ] {
        // Constant false is intentional: recipe validation never claims ALLOW.
        assert_eq!(
            validate_program(
                &caveated(CaveatInputSource::RelationshipContext, name, kind),
                &"b".repeat(32),
            )
            .unwrap(),
            vec![field]
        );
    }
}

#[test]
fn unknown_or_mistyped_stored_inputs_are_rejected() {
    for (name, kind) in [
        ("unknown_secret", CaveatValueType::Bool),
        ("approved_amount", CaveatValueType::String),
        ("is_approved", CaveatValueType::Int),
        ("is_suspended", CaveatValueType::String),
        ("is_override", CaveatValueType::Int),
    ] {
        assert_eq!(
            validate_program(
                &caveated(CaveatInputSource::RelationshipContext, name, kind),
                &"b".repeat(32)
            ),
            Err(ApprovalError::Invalid)
        );
    }
    // Caller context and capability claims are not values supplied by this recipe.
    validate_program(
        &caveated(
            CaveatInputSource::RequestContext,
            "requested_amount",
            CaveatValueType::Int,
        ),
        &"b".repeat(32),
    )
    .unwrap();
    validate_program(
        &caveated(
            CaveatInputSource::CapabilityClaim,
            "task_id",
            CaveatValueType::String,
        ),
        &"b".repeat(32),
    )
    .unwrap();
}

#[test]
fn team_arrow_and_boolean_combinations_fit_the_private_recipe() {
    let mut policy = base();
    let id = "b".repeat(32);
    let team = format!("team_{id}");
    let org = format!("organization_{id}");
    policy.definitions.push(ObjectDefinition {
        name: team.clone(),
        relations: vec![RelationDefinition {
            name: "member".into(),
            allowed_subject_types: vec![direct_subject(None)],
        }],
    });
    policy.definitions.push(ObjectDefinition {
        name: org.clone(),
        relations: vec![RelationDefinition {
            name: "auditor".into(),
            allowed_subject_types: vec![direct_subject(None)],
        }],
    });
    policy.definitions[1].relations.push(RelationDefinition {
        name: "team_viewer".into(),
        allowed_subject_types: vec![AllowedSubjectType {
            object_type: team,
            subject_relation: Some("member".into()),
            caveat_name: None,
        }],
    });
    policy.definitions[1].relations.push(RelationDefinition {
        name: "organization".into(),
        allowed_subject_types: vec![AllowedSubjectType {
            object_type: org.clone(),
            subject_relation: None,
            caveat_name: None,
        }],
    });
    policy.permissions[0].expression = PermissionExpr::AllOf(vec![
        PermissionExpr::RelationRef {
            name: "viewer".into(),
        },
        PermissionExpr::AnyOf(vec![
            PermissionExpr::SubjectRelationRef {
                relation: "team_viewer".into(),
                subject_relation: "member".into(),
            },
            PermissionExpr::Arrow {
                source_relation: "organization".into(),
                target_permission: "audit".into(),
            },
        ]),
    ]);
    policy.permissions.push(PermissionDefinition {
        object_type: org,
        name: "audit".into(),
        expression: PermissionExpr::RelationRef {
            name: "auditor".into(),
        },
    });
    assert_eq!(
        validate_program(&policy, &id).unwrap(),
        vec![GrantInputField::TeamId, GrantInputField::OrganizationId]
    );
    let mut any = policy.clone();
    if let PermissionExpr::AllOf(children) = any.permissions[0].expression.clone() {
        any.permissions[0].expression = PermissionExpr::AnyOf(children);
    }
    assert_eq!(
        validate_program(&any, &id).unwrap(),
        vec![GrantInputField::TeamId, GrantInputField::OrganizationId]
    );
    for (expression, relation, field) in [
        (
            PermissionExpr::SubjectRelationRef {
                relation: "team_viewer".into(),
                subject_relation: "member".into(),
            },
            "team_viewer",
            GrantInputField::TeamId,
        ),
        (
            PermissionExpr::Arrow {
                source_relation: "organization".into(),
                target_permission: "audit".into(),
            },
            "organization",
            GrantInputField::OrganizationId,
        ),
    ] {
        let mut single = policy.clone();
        single.permissions[0].expression = expression;
        single.definitions[1]
            .relations
            .retain(|r| r.name == relation);
        assert_eq!(validate_program(&single, &id).unwrap(), vec![field]);
    }
    policy.definitions[2].relations[0].name = "admin".into();
    assert_eq!(validate_program(&policy, &id), Err(ApprovalError::Invalid));
}

#[test]
fn direct_boolean_leaves_need_no_private_fields() {
    for all in [false, true] {
        let mut policy = base();
        policy.definitions[1].relations.push(RelationDefinition {
            name: "owner".into(),
            allowed_subject_types: vec![direct_subject(None)],
        });
        let children = vec![
            PermissionExpr::RelationRef {
                name: "viewer".into(),
            },
            PermissionExpr::RelationRef {
                name: "owner".into(),
            },
        ];
        policy.permissions[0].expression = if all {
            PermissionExpr::AllOf(children)
        } else {
            PermissionExpr::AnyOf(children)
        };
        assert!(
            validate_program(&policy, &"b".repeat(32))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn only_referenced_caveats_supply_fields_and_sources_are_not_operator_inputs() {
    let mut policy = caveated(
        CaveatInputSource::RelationshipContext,
        "is_approved",
        CaveatValueType::Bool,
    );
    policy.caveats[0].parameters.extend([
        CaveatParameter {
            name: "requested_amount".into(),
            source: CaveatInputSource::RequestContext,
            value_type: CaveatValueType::Int,
        },
        CaveatParameter {
            name: "task_id".into(),
            source: CaveatInputSource::CapabilityClaim,
            value_type: CaveatValueType::String,
        },
    ]);
    let mut unused = policy.caveats[0].clone();
    unused.name = "unused".into();
    unused.parameters = vec![CaveatParameter {
        name: "is_override".into(),
        source: CaveatInputSource::RelationshipContext,
        value_type: CaveatValueType::Bool,
    }];
    policy.caveats.push(unused);
    assert_eq!(
        validate_program(&policy, &"b".repeat(32)).unwrap(),
        vec![GrantInputField::IsApproved]
    );
    policy.permissions[0].expression = PermissionExpr::RelationRef {
        name: "viewer".into(),
    };
    policy.definitions[1].relations[0].allowed_subject_types = vec![direct_subject(None)];
    assert!(
        validate_program(&policy, &"b".repeat(32))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn caveat_field_union_is_deduplicated_and_uses_only_the_fixed_names() {
    let mut policy = caveated(
        CaveatInputSource::RelationshipContext,
        "is_approved",
        CaveatValueType::Bool,
    );
    policy.caveats[0].parameters.push(CaveatParameter {
        name: "approved_amount".into(),
        source: CaveatInputSource::RelationshipContext,
        value_type: CaveatValueType::Int,
    });
    let first = policy.permissions[0].expression.clone();
    let mut other = policy.caveats[0].clone();
    other.name = "other".into();
    other.parameters.push(CaveatParameter {
        name: "is_override".into(),
        source: CaveatInputSource::RelationshipContext,
        value_type: CaveatValueType::Bool,
    });
    policy.definitions[1].relations.push(RelationDefinition {
        name: "owner".into(),
        allowed_subject_types: vec![direct_subject(Some(other.name.clone()))],
    });
    let second = PermissionExpr::CaveatedRelation {
        relation: "owner".into(),
        caveat_name: other.name.clone(),
    };
    policy.caveats.push(other);
    for expression in [
        PermissionExpr::AnyOf(vec![first.clone(), second.clone()]),
        PermissionExpr::AllOf(vec![first, second]),
    ] {
        policy.permissions[0].expression = expression;
        let fields = validate_program(&policy, &"b".repeat(32)).unwrap();
        assert_eq!(
            serde_json::to_value(fields).unwrap(),
            serde_json::json!(["approvedAmount", "isApproved", "isOverride"])
        );
    }
}

#[test]
fn unrepresentable_policy_shapes_are_rejected() {
    let id = "b".repeat(32);
    let mut changed = base();
    changed.definitions[1].relations[0]
        .allowed_subject_types
        .push(direct_subject(None));
    assert_eq!(validate_program(&changed, &id), Err(ApprovalError::Invalid));
    changed = base();
    changed.definitions.push(ObjectDefinition {
        name: format!("custom_{id}"),
        relations: vec![],
    });
    assert_eq!(validate_program(&changed, &id), Err(ApprovalError::Invalid));
    changed = base();
    changed.permissions[0].name = "write".into();
    assert_eq!(validate_program(&changed, &id), Err(ApprovalError::Invalid));
}
