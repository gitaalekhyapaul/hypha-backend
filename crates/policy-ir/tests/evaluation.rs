mod support;

use raziel_policy_ir::{
    CaveatExpr, CaveatValue, ContextEntry, EvaluationError, EvaluationRequest, LimitKind,
    RelationshipCaveat, ResolvedRelationship, ValidatedPolicyProgram, evaluate,
};

use support::{
    CAVEAT_NAME, Resolver, all_caveat_nodes_program, base_program, caveated_relationship, ceilings,
    object, relationship,
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
        evaluate(&and_program, &true_branch, &resolver),
        Err(EvaluationError::MissingCaveatInput)
    );
    assert!(evaluate(&or_program, &true_branch, &resolver).unwrap());
    assert_eq!(
        evaluate(&or_program, &false_branch, &resolver),
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
        merkle_depth: 8,
        private_input_bytes: 64,
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
fn runtime_rejects_each_private_input_limit_one_over_boundary() {
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

    let mut merkle_program = base_program();
    merkle_program.limits.max_merkle_depth = 7;
    let merkle_program = ValidatedPolicyProgram::new(merkle_program, &ceilings()).unwrap();
    let mut merkle = Resolver::default();
    merkle.insert(
        document.clone(),
        "editor",
        vec![relationship(object("user", "alice"))],
    );
    assert_eq!(
        evaluate(&merkle_program, &request("direct"), &merkle),
        Err(EvaluationError::BudgetExceeded(LimitKind::MerkleDepth))
    );

    let mut private_program = base_program();
    private_program.limits.max_total_private_input_bytes = 63;
    let private_program = ValidatedPolicyProgram::new(private_program, &ceilings()).unwrap();
    let mut private = Resolver::default();
    private.insert(
        document,
        "editor",
        vec![relationship(object("user", "alice"))],
    );
    assert_eq!(
        evaluate(&private_program, &request("direct"), &private),
        Err(EvaluationError::BudgetExceeded(
            LimitKind::TotalPrivateInputBytes
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
