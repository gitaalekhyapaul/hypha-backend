mod support;

use raziel_policy_ir::{
    CaveatExpr, CaveatValue, ContextEntry, EvaluationError, EvaluationRequest, LimitKind,
    PermissionExpr, RelationshipCaveat, ResolvedRelationship, ValidatedPolicyProgram, evaluate,
    evaluate_with_usage,
};

use support::{
    CAVEAT_NAME, Resolver, all_caveat_nodes_program, base_program, caveated_relation,
    caveated_relationship, ceilings, object, relation_ref, relationship,
};

fn request(permission: &str) -> EvaluationRequest {
    EvaluationRequest {
        resource: object("document", "doc_1"),
        subject: object("user", "alice"),
        permission: permission.into(),
        request_context: vec![],
        capability_claims: vec![
            ContextEntry {
                name: "max_amount".into(),
                value: CaveatValue::Int(100),
            },
            ContextEntry {
                name: "purpose".into(),
                value: CaveatValue::String("reconciliation".into()),
            },
        ],
    }
}

#[test]
fn evaluates_direct_union_and_intersection_paths() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let alice = relationship(object("user", "alice"));

    let mut direct = Resolver::default();
    direct.insert(document.clone(), "editor", vec![alice.clone()]);
    assert!(evaluate(&program, &request("direct"), &direct).unwrap());
    assert!(evaluate(&program, &request("access"), &direct).unwrap());

    let mut intersection = Resolver::default();
    intersection.insert(document.clone(), "editor", vec![alice.clone()]);
    intersection.insert(document, "viewer", vec![alice]);
    assert!(evaluate(&program, &request("constrained"), &intersection).unwrap());

    let denied = Resolver::default();
    assert!(!evaluate(&program, &request("direct"), &denied).unwrap());
    assert!(!evaluate(&program, &request("constrained"), &direct).unwrap());
}

#[test]
fn traced_union_propagates_a_later_budget_error_after_an_allowing_branch() {
    let mut program = base_program();
    program.limits.max_expression_nodes = 13;
    let program = ValidatedPolicyProgram::new(program, &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let mut editor = relationship(object("user", "alice"));
    editor.witness_id = Some(1);
    let parents = (0..10)
        .map(|index| {
            let mut parent = relationship(object("folder", &format!("folder_{index:02}")));
            parent.witness_id = Some(index + 2);
            parent
        })
        .collect();
    let mut resolver = Resolver::default();
    resolver.insert(document.clone(), "editor", vec![editor]);
    resolver.insert(document, "parent", parents);

    assert_eq!(
        evaluate_with_usage(&program, &request("access"), &resolver),
        Err(EvaluationError::BudgetExceeded(LimitKind::ExpressionNodes))
    );
}

#[test]
fn traces_the_first_canonical_positive_path() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let group = object("group", "ops");
    let folder = object("folder", "payments");

    let mut editor = relationship(object("user", "alice"));
    editor.witness_id = Some(7);
    let mut viewer = relationship(object("user", "alice"));
    viewer.witness_id = Some(3);
    let mut group_tuple = relationship(group.clone());
    group_tuple.subject_relation = Some("member".into());
    group_tuple.witness_id = Some(4);
    let mut member = relationship(object("user", "alice"));
    member.witness_id = Some(2);
    let mut parent = relationship(folder.clone());
    parent.witness_id = Some(9);
    let mut reader = relationship(object("user", "alice"));
    reader.witness_id = Some(5);

    let mut resolver = Resolver::default();
    resolver.insert(document.clone(), "editor", vec![editor]);
    resolver.insert(document.clone(), "viewer", vec![group_tuple, viewer]);
    resolver.insert(document.clone(), "parent", vec![parent]);
    resolver.insert(group, "member", vec![member]);
    resolver.insert(folder, "reader", vec![reader]);

    assert_eq!(
        evaluate_with_usage(&program, &request("access"), &resolver)
            .unwrap()
            .used_witness_ids,
        [7]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("constrained"), &resolver)
            .unwrap()
            .used_witness_ids,
        [3, 7]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("group_access"), &resolver)
            .unwrap()
            .used_witness_ids,
        [2, 4]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("parent_access"), &resolver)
            .unwrap()
            .used_witness_ids,
        [5, 9]
    );
}

#[test]
fn trace_selection_uses_the_first_success_in_resolver_order() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let group_a = object("group", "a");
    let group_b = object("group", "b");
    let folder_a = object("folder", "a");
    let folder_b = object("folder", "b");

    let with_id = |mut relationship: ResolvedRelationship, witness_id| {
        relationship.witness_id = Some(witness_id);
        relationship
    };

    let mut group_tuple_a = with_id(relationship(group_a.clone()), 20);
    group_tuple_a.subject_relation = Some("member".into());
    let mut group_tuple_b = with_id(relationship(group_b.clone()), 21);
    group_tuple_b.subject_relation = Some("member".into());

    let mut resolver = Resolver::default();
    resolver.insert(
        document.clone(),
        "editor",
        vec![with_id(relationship(object("user", "alice")), 10)],
    );
    resolver.insert(
        document.clone(),
        "viewer",
        vec![
            group_tuple_a,
            group_tuple_b,
            with_id(caveated_relationship(object("user", "alice"), 20), 12),
        ],
    );
    resolver.insert(
        group_a,
        "member",
        vec![with_id(relationship(object("user", "alice")), 22)],
    );
    resolver.insert(
        group_b,
        "member",
        vec![with_id(relationship(object("user", "alice")), 23)],
    );
    resolver.insert(
        document.clone(),
        "parent",
        vec![
            with_id(relationship(folder_a.clone()), 30),
            with_id(relationship(folder_b.clone()), 31),
        ],
    );
    resolver.insert(
        folder_a,
        "reader",
        vec![with_id(relationship(object("user", "alice")), 32)],
    );
    resolver.insert(
        folder_b,
        "reader",
        vec![with_id(relationship(object("user", "alice")), 33)],
    );

    assert_eq!(
        evaluate_with_usage(&program, &request("direct"), &resolver)
            .unwrap()
            .used_witness_ids,
        [10]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("conditional"), &resolver)
            .unwrap()
            .used_witness_ids,
        [12]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("group_access"), &resolver)
            .unwrap()
            .used_witness_ids,
        [20, 22]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("parent_access"), &resolver)
            .unwrap()
            .used_witness_ids,
        [30, 32]
    );
    assert_eq!(
        evaluate_with_usage(&program, &request("access"), &resolver)
            .unwrap()
            .used_witness_ids,
        [10]
    );
}

#[test]
fn reused_caveated_witness_is_charged_once_at_the_context_ceiling() {
    let mut program = base_program();
    program.limits.max_context_entries = 3;
    program
        .permissions
        .iter_mut()
        .find(|permission| permission.name == "conditional")
        .unwrap()
        .expression = PermissionExpr::AllOf(vec![
        PermissionExpr::AnyOf(vec![relation_ref("editor"), caveated_relation("viewer")]),
        caveated_relation("viewer"),
    ]);
    let program = ValidatedPolicyProgram::new(program, &ceilings()).unwrap();

    let mut caveated = caveated_relationship(object("user", "alice"), 10);
    caveated.witness_id = Some(7);
    let mut resolver = Resolver::default();
    resolver.insert(object("document", "doc_1"), "viewer", vec![caveated]);

    let outcome = evaluate_with_usage(&program, &request("conditional"), &resolver).unwrap();
    assert!(outcome.allowed);
    assert_eq!(outcome.used_witness_ids, [7]);
}

#[test]
fn resolver_rejects_duplicate_structural_identity_across_caveat_forms() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let mut direct = relationship(object("user", "alice"));
    direct.witness_id = Some(1);
    let mut caveated = caveated_relationship(object("user", "alice"), 10);
    caveated.witness_id = Some(2);
    let mut resolver = Resolver::default();
    resolver.insert(
        object("document", "doc_1"),
        "viewer",
        vec![direct, caveated],
    );

    assert_eq!(
        evaluate_with_usage(&program, &request("conditional"), &resolver),
        Err(EvaluationError::InvalidRelationshipData)
    );
}

#[test]
fn evaluates_one_hop_subject_sets() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let group = object("group", "ops");
    let mut group_tuple = relationship(group.clone());
    group_tuple.subject_relation = Some("member".into());

    let mut resolver = Resolver::default();
    resolver.insert(document, "viewer", vec![group_tuple]);
    resolver.insert(group, "member", vec![relationship(object("user", "alice"))]);

    assert!(evaluate(&program, &request("group_access"), &resolver).unwrap());
}

#[test]
fn evaluates_one_hop_arrows() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let folder = object("folder", "payments");
    let mut resolver = Resolver::default();
    resolver.insert(document, "parent", vec![relationship(folder.clone())]);
    resolver.insert(
        folder,
        "reader",
        vec![relationship(object("user", "alice"))],
    );

    assert!(evaluate(&program, &request("parent_access"), &resolver).unwrap());
}

#[test]
fn evaluates_tuple_bound_caveats_and_all_operators() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let mut resolver = Resolver::default();
    resolver.insert(
        document.clone(),
        "viewer",
        vec![caveated_relationship(object("user", "alice"), 80)],
    );
    assert!(evaluate(&program, &request("conditional"), &resolver).unwrap());

    let mut denied = Resolver::default();
    denied.insert(
        document.clone(),
        "viewer",
        vec![caveated_relationship(object("user", "alice"), 120)],
    );
    assert!(!evaluate(&program, &request("conditional"), &denied).unwrap());

    let all_nodes = ValidatedPolicyProgram::new(all_caveat_nodes_program(), &ceilings()).unwrap();
    let mut all_nodes_resolver = Resolver::default();
    let mut relationship = caveated_relationship(object("user", "alice"), 1);
    relationship.caveat.as_mut().unwrap().context.clear();
    all_nodes_resolver.insert(document, "viewer", vec![relationship]);
    assert!(evaluate(&all_nodes, &request("conditional"), &all_nodes_resolver).unwrap());
}

#[test]
fn caveat_booleans_absorb_only_decisive_missing_inputs() {
    let document = object("document", "doc_1");
    let mut resolver = Resolver::default();
    resolver.insert(
        document,
        "viewer",
        vec![caveated_relationship(object("user", "alice"), 80)],
    );

    let and_program = base_program();
    let CaveatExpr::And(children) = and_program.caveats[0].expression.clone() else {
        panic!("test caveat must start as an AND expression");
    };
    let mut or_program = and_program.clone();
    or_program.caveats[0].expression = CaveatExpr::Or(children);
    let and_program = ValidatedPolicyProgram::new(and_program, &ceilings()).unwrap();
    let or_program = ValidatedPolicyProgram::new(or_program, &ceilings()).unwrap();

    let mut true_branch = request("conditional");
    true_branch
        .capability_claims
        .retain(|entry| entry.name != "max_amount");
    let mut false_branch = true_branch.clone();
    false_branch
        .capability_claims
        .iter_mut()
        .find(|entry| entry.name == "purpose")
        .unwrap()
        .value = CaveatValue::String("wrong-purpose".into());

    assert!(!evaluate(&and_program, &false_branch, &resolver).unwrap());
    assert_eq!(
        evaluate_with_usage(&and_program, &false_branch, &resolver).map(|outcome| outcome.allowed),
        Ok(false)
    );
    assert_eq!(
        evaluate(&and_program, &true_branch, &resolver),
        Err(EvaluationError::MissingCaveatInput)
    );
    assert_eq!(
        evaluate_with_usage(&and_program, &true_branch, &resolver),
        Err(EvaluationError::MissingCaveatInput)
    );
    assert!(evaluate(&or_program, &true_branch, &resolver).unwrap());
    assert_eq!(
        evaluate_with_usage(&or_program, &true_branch, &resolver).map(|outcome| outcome.allowed),
        Ok(true)
    );
    assert_eq!(
        evaluate(&or_program, &false_branch, &resolver),
        Err(EvaluationError::MissingCaveatInput)
    );
    assert_eq!(
        evaluate_with_usage(&or_program, &false_branch, &resolver),
        Err(EvaluationError::MissingCaveatInput)
    );

    let mut mistyped_branch = false_branch;
    mistyped_branch.capability_claims.push(ContextEntry {
        name: "max_amount".into(),
        value: CaveatValue::String("not-an-integer".into()),
    });
    mistyped_branch.capability_claims.sort();
    assert_eq!(
        evaluate(&and_program, &mistyped_branch, &resolver),
        Err(EvaluationError::CaveatInputTypeMismatch)
    );
    assert_eq!(
        evaluate_with_usage(&and_program, &mistyped_branch, &resolver),
        Err(EvaluationError::CaveatInputTypeMismatch)
    );
}

#[test]
fn caveat_context_never_crosses_tuple_boundaries() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let alice = ResolvedRelationship {
        subject: object("user", "alice"),
        subject_relation: None,
        caveat: Some(RelationshipCaveat {
            name: CAVEAT_NAME.into(),
            context: vec![],
        }),
        witness_id: None,
    };
    let bob = caveated_relationship(object("user", "bob"), 10);
    let mut resolver = Resolver::default();
    resolver.insert(document, "viewer", vec![alice, bob]);

    assert_eq!(
        evaluate(&program, &request("conditional"), &resolver),
        Err(EvaluationError::MissingCaveatInput)
    );
}

#[test]
fn runtime_rejects_relationship_witness_limit_one_over_boundary() {
    let document = object("document", "doc_1");

    let mut witness_program = base_program();
    witness_program.limits.max_relationship_witnesses = 1;
    let witness_program = ValidatedPolicyProgram::new(witness_program, &ceilings()).unwrap();
    let mut witnesses = Resolver::default();
    witnesses.insert(
        document.clone(),
        "editor",
        vec![
            relationship(object("user", "alice")),
            relationship(object("user", "bob")),
        ],
    );
    assert_eq!(
        evaluate(&witness_program, &request("direct"), &witnesses),
        Err(EvaluationError::BudgetExceeded(
            LimitKind::RelationshipWitnesses
        ))
    );
}

#[test]
fn runtime_rejects_string_and_context_limits_one_over_boundary() {
    let mut string_program = base_program();
    string_program.limits.max_string_bytes = 15;
    let string_program = ValidatedPolicyProgram::new(string_program, &ceilings()).unwrap();
    let mut long_request = request("direct");
    long_request.resource.object_id = "x".repeat(16);
    assert_eq!(
        evaluate(&string_program, &long_request, &Resolver::default()),
        Err(EvaluationError::BudgetExceeded(LimitKind::StringBytes))
    );

    let mut context_program = base_program();
    context_program.limits.max_context_entries = 1;
    let context_program = ValidatedPolicyProgram::new(context_program, &ceilings()).unwrap();
    assert_eq!(
        evaluate(&context_program, &request("direct"), &Resolver::default()),
        Err(EvaluationError::BudgetExceeded(LimitKind::ContextEntries))
    );
}

#[test]
fn relationship_and_context_order_fail_closed() {
    let program = ValidatedPolicyProgram::new(base_program(), &ceilings()).unwrap();
    let document = object("document", "doc_1");
    let mut resolver = Resolver::default();
    resolver.insert(
        document,
        "editor",
        vec![
            relationship(object("user", "bob")),
            relationship(object("user", "alice")),
        ],
    );
    assert_eq!(
        evaluate(&program, &request("direct"), &resolver),
        Err(EvaluationError::InvalidRelationshipData)
    );

    let mut duplicate_context = request("direct");
    duplicate_context.capability_claims[1].name = "max_amount".into();
    assert_eq!(
        evaluate(&program, &duplicate_context, &Resolver::default()),
        Err(EvaluationError::InvalidRequest)
    );
}

#[test]
fn public_errors_do_not_contain_private_values() {
    let sentinel = "private_amount_92831";
    let error = EvaluationError::MissingCaveatInput;
    assert!(!error.to_string().contains(sentinel));
    assert!(!format!("{error:?}").contains(sentinel));
}
