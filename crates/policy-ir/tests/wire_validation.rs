mod support;

use raziel_policy_ir::{
    CaveatExpr, CaveatValueType, CollectionKind, LimitKind, PermissionDefinition, PermissionExpr,
    PolicyError, ReferenceKind, ValidatedPolicyProgram,
};
use raziel_proto::raziel::v1 as proto;

use support::{all_caveat_nodes_program, base_program, ceilings, to_proto};

type LimitMutation = fn(&mut raziel_policy_ir::PolicyLimits);

#[test]
fn wire_converts_every_supported_node() {
    let program = all_caveat_nodes_program();
    let validated =
        ValidatedPolicyProgram::from_trusted_proto(to_proto(&program), &ceilings()).unwrap();
    assert_eq!(validated.program(), &program);
}

#[test]
fn wire_rejects_unknown_version_enum_and_node() {
    let mut unknown_version = to_proto(&base_program());
    unknown_version.ir_version = 99;
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(unknown_version, &ceilings()),
        Err(PolicyError::UnsupportedVersion)
    );

    let mut unknown_enum = to_proto(&base_program());
    unknown_enum.caveats[0].parameters[0].source = 99;
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(unknown_enum, &ceilings()),
        Err(PolicyError::UnknownInputSource)
    );

    let mut missing_node = to_proto(&base_program());
    missing_node.permissions[0].expression = Some(proto::PermissionExpr { node: None });
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(missing_node, &ceilings()),
        Err(PolicyError::MissingNode)
    );
}

#[test]
fn wire_requires_a_32_byte_source_schema_digest() {
    for length in [0, 31, 33, 64] {
        let mut wire = to_proto(&base_program());
        wire.source_schema_digest = vec![0; length];

        assert_eq!(
            ValidatedPolicyProgram::from_trusted_proto(wire, &ceilings()),
            Err(PolicyError::InvalidSourceSchemaDigest),
            "digest length {length} must fail"
        );
    }
}

#[test]
fn wire_rejects_missing_limits_operands_and_excess_syntax_depth() {
    let mut missing_limits = to_proto(&base_program());
    missing_limits.limits = None;
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(missing_limits, &ceilings()),
        Err(PolicyError::MissingLimits)
    );

    let mut missing_operand = to_proto(&base_program());
    let proto::caveat_expr::Node::And(and) = missing_operand.caveats[0]
        .expression
        .as_mut()
        .unwrap()
        .node
        .as_mut()
        .unwrap()
    else {
        panic!("fixture root must be And");
    };
    let proto::caveat_expr::Node::Equal(equal) = and.children[0].node.as_mut().unwrap() else {
        panic!("fixture first child must be Equal");
    };
    equal.left = None;
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(missing_operand, &ceilings()),
        Err(PolicyError::MissingOperand)
    );

    let mut shallow = ceilings();
    shallow.max_syntax_depth = 1;
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(to_proto(&base_program()), &shallow),
        Err(PolicyError::ProgramLimitExceeded(LimitKind::SyntaxDepth))
    );
}

#[test]
fn validation_accepts_exact_declared_ceilings() {
    assert!(ValidatedPolicyProgram::new(base_program(), &ceilings()).is_ok());
}

#[test]
fn validation_rejects_each_limit_above_its_trusted_ceiling() {
    let cases: &[(LimitKind, LimitMutation)] = &[
        (LimitKind::ExpressionNodes, |value| {
            value.max_expression_nodes += 1
        }),
        (LimitKind::CaveatNodes, |value| value.max_caveat_nodes += 1),
        (LimitKind::PathDepth, |value| value.max_path_depth += 1),
        (LimitKind::RelationshipWitnesses, |value| {
            value.max_relationship_witnesses += 1;
        }),
        (LimitKind::MerkleDepth, |value| value.max_merkle_depth += 1),
        (LimitKind::StringBytes, |value| value.max_string_bytes += 1),
        (LimitKind::ContextEntries, |value| {
            value.max_context_entries += 1
        }),
        (LimitKind::TotalPrivateInputBytes, |value| {
            value.max_total_private_input_bytes += 1;
        }),
    ];

    for (kind, mutate) in cases {
        let mut program = base_program();
        mutate(&mut program.limits);
        assert_eq!(
            ValidatedPolicyProgram::new(program, &ceilings()),
            Err(PolicyError::LimitAboveCeiling(*kind))
        );
    }
}

#[test]
fn validation_rejects_one_over_static_node_and_path_limits() {
    let mut expression_limit = base_program();
    expression_limit.limits.max_expression_nodes = 12;
    assert_eq!(
        ValidatedPolicyProgram::new(expression_limit, &ceilings()),
        Err(PolicyError::ProgramLimitExceeded(
            LimitKind::ExpressionNodes
        ))
    );

    let mut caveat_limit = base_program();
    caveat_limit.limits.max_caveat_nodes = 6;
    assert_eq!(
        ValidatedPolicyProgram::new(caveat_limit, &ceilings()),
        Err(PolicyError::ProgramLimitExceeded(LimitKind::CaveatNodes))
    );

    let mut path_limit = base_program();
    path_limit.limits.max_path_depth = 1;
    assert_eq!(
        ValidatedPolicyProgram::new(path_limit, &ceilings()),
        Err(PolicyError::ProgramLimitExceeded(LimitKind::PathDepth))
    );
}

#[test]
fn validation_rejects_noncanonical_collections_and_child_counts() {
    let mut definitions = base_program();
    definitions.definitions.swap(0, 1);
    assert_eq!(
        ValidatedPolicyProgram::new(definitions, &ceilings()),
        Err(PolicyError::NotStrictlySorted(CollectionKind::Definitions))
    );

    let mut children = base_program();
    children.permissions[0].expression = PermissionExpr::AnyOf(vec![PermissionExpr::RelationRef {
        name: "editor".into(),
    }]);
    assert_eq!(
        ValidatedPolicyProgram::new(children, &ceilings()),
        Err(PolicyError::InvalidChildCount(
            CollectionKind::PermissionChildren
        ))
    );
}

#[test]
fn validation_rejects_unresolved_and_ambiguous_references() {
    let mut unresolved = base_program();
    unresolved.permissions[3].expression = PermissionExpr::RelationRef {
        name: "missing".into(),
    };
    assert_eq!(
        ValidatedPolicyProgram::new(unresolved, &ceilings()),
        Err(PolicyError::MissingReference(ReferenceKind::Relation))
    );

    let mut ambiguous = base_program();
    ambiguous.definitions[0].relations[1]
        .allowed_subject_types
        .push(support::direct_allowed("group"));
    ambiguous.definitions[0].relations[1]
        .allowed_subject_types
        .sort();
    assert_eq!(
        ValidatedPolicyProgram::new(ambiguous, &ceilings()),
        Err(PolicyError::AmbiguousReference(ReferenceKind::Object))
    );
}

#[test]
fn validation_rejects_caveat_type_errors_and_caveated_usersets() {
    let mut type_error = base_program();
    type_error.caveats[0].expression = CaveatExpr::LessThan(
        Box::new(CaveatExpr::ConstString("a".into())),
        Box::new(CaveatExpr::ConstString("b".into())),
    );
    assert_eq!(
        ValidatedPolicyProgram::new(type_error, &ceilings()),
        Err(PolicyError::CaveatTypeMismatch)
    );

    let mut userset = base_program();
    userset.definitions[0].relations[2].allowed_subject_types[0].caveat_name =
        Some(support::CAVEAT_NAME.into());
    assert_eq!(
        ValidatedPolicyProgram::new(userset, &ceilings()),
        Err(PolicyError::UnsupportedRelationshipShape)
    );
}

#[test]
fn validation_rejects_two_arrow_paths_and_cycles() {
    let mut two_arrows = base_program();
    two_arrows.definitions[1].relations.insert(
        0,
        raziel_policy_ir::RelationDefinition {
            name: "parent".into(),
            allowed_subject_types: vec![support::direct_allowed("group")],
        },
    );
    two_arrows.permissions[6].expression = PermissionExpr::Arrow {
        source_relation: "parent".into(),
        target_permission: "read".into(),
    };
    two_arrows.permissions.push(PermissionDefinition {
        object_type: "group".into(),
        name: "read".into(),
        expression: PermissionExpr::RelationRef {
            name: "member".into(),
        },
    });
    assert_eq!(
        ValidatedPolicyProgram::new(two_arrows, &ceilings()),
        Err(PolicyError::ArrowDepthExceeded)
    );

    let mut cycle = base_program();
    cycle.definitions[1]
        .relations
        .push(raziel_policy_ir::RelationDefinition {
            name: "root".into(),
            allowed_subject_types: vec![support::direct_allowed("document")],
        });
    cycle.permissions.insert(
        6,
        PermissionDefinition {
            object_type: "folder".into(),
            name: "loop".into(),
            expression: PermissionExpr::Arrow {
                source_relation: "root".into(),
                target_permission: "parent_access".into(),
            },
        },
    );
    cycle.permissions[5].expression = PermissionExpr::Arrow {
        source_relation: "parent".into(),
        target_permission: "loop".into(),
    };
    assert_eq!(
        ValidatedPolicyProgram::new(cycle, &ceilings()),
        Err(PolicyError::PermissionCycle)
    );
}

#[test]
fn validation_rejects_unknown_value_type_and_non_boolean_root() {
    let mut unknown_type = to_proto(&base_program());
    unknown_type.caveats[0].parameters[0].value_type = 99;
    assert_eq!(
        ValidatedPolicyProgram::from_trusted_proto(unknown_type, &ceilings()),
        Err(PolicyError::UnknownValueType)
    );

    let mut non_boolean = base_program();
    non_boolean.caveats[0].parameters.clear();
    non_boolean.caveats[0].expression = CaveatExpr::ConstInt(1);
    assert_eq!(
        ValidatedPolicyProgram::new(non_boolean, &ceilings()),
        Err(PolicyError::CaveatRootNotBoolean)
    );

    let _ = CaveatValueType::Bool;
}
