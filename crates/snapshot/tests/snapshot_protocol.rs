mod support;

use raziel_policy_ir::CaveatValue;
use raziel_snapshot::{
    RELATIONSHIP_WITNESS_SET_VERSION, RelationshipField, RelationshipWitnessSetV1, SnapshotError,
    SnapshotLimit, SnapshotLimits, SnapshotTree, StoredContextEntry, compare_relationship_identity,
    encode_relationship, relationship_leaf_digest, relationship_node_digest, verify_opening,
    verify_witness_set,
};

use support::{
    TEST_POLICY_LIMITS, TEST_SALT, TEST_SNAPSHOT_LIMITS, policy, policy_with_limits, relationships,
};

#[test]
fn builds_stable_empty_even_and_odd_roots_and_verifies_every_opening() {
    let policy = policy();
    let records = relationships();
    let mut observed_roots = Vec::new();

    for count in [0, 1, 2, 3, 5] {
        let tree = SnapshotTree::build(
            &policy,
            records[..count].to_vec(),
            &TEST_SALT,
            TEST_SNAPSHOT_LIMITS,
        )
        .unwrap();
        let repeated = SnapshotTree::build(
            &policy,
            records[..count].to_vec(),
            &TEST_SALT,
            TEST_SNAPSHOT_LIMITS,
        )
        .unwrap();
        assert_eq!(tree.root(), repeated.root());
        observed_roots.push(tree.root());

        for index in 0..u32::try_from(count).unwrap() {
            let opening = tree.opening(index).unwrap();
            verify_opening(
                &policy,
                &TEST_SALT,
                u32::try_from(count).unwrap(),
                &opening,
                &tree.root(),
                TEST_SNAPSHOT_LIMITS,
            )
            .unwrap();
        }
    }

    assert!(observed_roots.windows(2).all(|roots| roots[0] != roots[1]));
    assert_eq!(
        SnapshotTree::build(&policy, vec![], &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .unwrap()
            .opening(0),
        Err(SnapshotError::EmptySnapshotOpening)
    );
}

#[test]
fn one_snapshot_salt_blinds_every_root_and_leaf() {
    let policy = policy();
    let records = relationships();
    let changed_salt = [0x5b; 32];
    let first =
        SnapshotTree::build(&policy, records.clone(), &TEST_SALT, TEST_SNAPSHOT_LIMITS).unwrap();
    let second = SnapshotTree::build(
        &policy,
        records.clone(),
        &changed_salt,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    assert_ne!(first.root(), second.root());

    let canonical = encode_relationship(&records[0]).unwrap();
    assert_ne!(
        relationship_leaf_digest(0, &TEST_SALT, &canonical).unwrap(),
        relationship_leaf_digest(0, &changed_salt, &canonical).unwrap()
    );
    assert_ne!(
        relationship_leaf_digest(0, &TEST_SALT, &canonical).unwrap(),
        relationship_leaf_digest(1, &TEST_SALT, &canonical).unwrap()
    );
}

#[test]
fn verifies_a_bounded_shared_witness_set_and_derives_accounting() {
    let policy = policy();
    let records = relationships();
    let tree = SnapshotTree::build(&policy, records, &TEST_SALT, TEST_SNAPSHOT_LIMITS).unwrap();
    let witness = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: TEST_SALT,
        leaf_count: 5,
        entries: vec![
            tree.opening(0).unwrap(),
            tree.opening(2).unwrap(),
            tree.opening(4).unwrap(),
        ],
    };

    let metrics =
        verify_witness_set(&policy, &witness, &tree.root(), TEST_SNAPSHOT_LIMITS).unwrap();
    assert_eq!(metrics.relationship_count, 3);
    assert_eq!(metrics.max_merkle_depth, 3);
    assert!(metrics.private_input_bytes > 44);
    assert!(metrics.private_input_bytes < policy.program().limits.max_total_private_input_bytes);
}

#[test]
fn enforces_exact_private_input_byte_limits_for_sets_and_single_openings() {
    let policy = policy();
    let tree = SnapshotTree::build(
        &policy,
        relationships()[..3].to_vec(),
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let opening = tree.opening(0).unwrap();
    let witness = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: TEST_SALT,
        leaf_count: 3,
        entries: vec![opening.clone()],
    };
    let metrics =
        verify_witness_set(&policy, &witness, &tree.root(), TEST_SNAPSHOT_LIMITS).unwrap();

    let mut exact_limits = TEST_POLICY_LIMITS;
    exact_limits.max_total_private_input_bytes = metrics.private_input_bytes;
    let exact_policy = policy_with_limits(exact_limits);
    verify_witness_set(&exact_policy, &witness, &tree.root(), TEST_SNAPSHOT_LIMITS).unwrap();
    verify_opening(
        &exact_policy,
        &TEST_SALT,
        3,
        &opening,
        &tree.root(),
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();

    let mut overrun_limits = exact_limits;
    overrun_limits.max_total_private_input_bytes -= 1;
    let overrun_policy = policy_with_limits(overrun_limits);
    assert_eq!(
        verify_witness_set(
            &overrun_policy,
            &witness,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::LimitExceeded(
            SnapshotLimit::TotalPrivateInputBytes
        ))
    );
    assert_eq!(
        verify_opening(
            &overrun_policy,
            &TEST_SALT,
            3,
            &opening,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::LimitExceeded(
            SnapshotLimit::TotalPrivateInputBytes
        ))
    );
}

#[test]
fn enforces_exact_witness_and_aggregate_context_count_limits() {
    let base_policy = policy();
    let base_records = relationships();
    let tree = SnapshotTree::build(
        &base_policy,
        base_records[..3].to_vec(),
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let witness = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: TEST_SALT,
        leaf_count: 3,
        entries: vec![
            tree.opening(0).unwrap(),
            tree.opening(1).unwrap(),
            tree.opening(2).unwrap(),
        ],
    };

    let mut exact_witness_limits = TEST_POLICY_LIMITS;
    exact_witness_limits.max_relationship_witnesses = 3;
    verify_witness_set(
        &policy_with_limits(exact_witness_limits),
        &witness,
        &tree.root(),
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let mut overrun_witness_limits = exact_witness_limits;
    overrun_witness_limits.max_relationship_witnesses = 2;
    assert_eq!(
        verify_witness_set(
            &policy_with_limits(overrun_witness_limits),
            &witness,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::LimitExceeded(
            SnapshotLimit::RelationshipWitnesses
        ))
    );

    let first = base_records[0].clone();
    let mut second = first.clone();
    second.resource_id = "synthetic-private-test-b".into();
    let context_tree = SnapshotTree::build(
        &base_policy,
        vec![first, second],
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let context_witness = RelationshipWitnessSetV1 {
        format_version: RELATIONSHIP_WITNESS_SET_VERSION,
        snapshot_secret_salt: TEST_SALT,
        leaf_count: 2,
        entries: vec![
            context_tree.opening(0).unwrap(),
            context_tree.opening(1).unwrap(),
        ],
    };
    let mut exact_context_limits = TEST_POLICY_LIMITS;
    exact_context_limits.max_context_entries = 6;
    verify_witness_set(
        &policy_with_limits(exact_context_limits),
        &context_witness,
        &context_tree.root(),
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let mut overrun_context_limits = exact_context_limits;
    overrun_context_limits.max_context_entries = 5;
    assert_eq!(
        verify_witness_set(
            &policy_with_limits(overrun_context_limits),
            &context_witness,
            &context_tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::LimitExceeded(SnapshotLimit::ContextEntries))
    );
}

#[test]
fn rejects_phantom_mixed_salt_and_mutated_openings() {
    let policy = policy();
    let tree = SnapshotTree::build(
        &policy,
        relationships()[..3].to_vec(),
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let opening = tree.opening(2).unwrap();

    let mut phantom = opening.clone();
    phantom.leaf_index = 3;
    assert_eq!(
        verify_opening(
            &policy,
            &TEST_SALT,
            3,
            &phantom,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::LeafIndexOutOfRange)
    );

    assert_eq!(
        verify_opening(
            &policy,
            &[0x5b; 32],
            3,
            &opening,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::RootMismatch)
    );

    let mut wrong_sibling = opening.clone();
    wrong_sibling.sibling_hashes[0][0] ^= 1;
    assert_eq!(
        verify_opening(
            &policy,
            &TEST_SALT,
            3,
            &wrong_sibling,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::RootMismatch)
    );

    let mut extra_sibling = opening;
    extra_sibling.sibling_hashes.push([0; 32]);
    assert_eq!(
        verify_opening(
            &policy,
            &TEST_SALT,
            3,
            &extra_sibling,
            &tree.root(),
            TEST_SNAPSHOT_LIMITS,
        ),
        Err(SnapshotError::SiblingCountMismatch)
    );
}

#[test]
fn rejects_unsorted_duplicate_and_policy_invalid_relationships() {
    let policy = policy();

    let mut unsorted = relationships();
    unsorted.swap(0, 1);
    assert_eq!(
        SnapshotTree::build(&policy, unsorted, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .err()
            .unwrap(),
        SnapshotError::RelationshipsNotStrictlySorted
    );

    let mut duplicate = relationships()[..1].to_vec();
    let mut same_identity = duplicate[0].clone();
    same_identity.stored_caveat.as_mut().unwrap().context[1].value = CaveatValue::Int(501);
    duplicate.push(same_identity);
    assert_eq!(
        SnapshotTree::build(&policy, duplicate, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .err()
            .unwrap(),
        SnapshotError::DuplicateRelationshipIdentity
    );

    let mut invalid = relationships()[..1].to_vec();
    invalid[0].resource_type = "Dataset".into();
    let error = SnapshotTree::build(&policy, invalid, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
        .err()
        .unwrap();
    assert_eq!(
        error,
        SnapshotError::InvalidSchemaIdentifier(RelationshipField::ResourceType)
    );
    assert!(!error.to_string().contains("Dataset"));

    let mut unknown = relationships()[..1].to_vec();
    unknown[0].relation = "unknown".into();
    assert!(matches!(
        SnapshotTree::build(&policy, unknown, &TEST_SALT, TEST_SNAPSHOT_LIMITS),
        Err(SnapshotError::MissingPolicyReference(_))
    ));

    let mut wrong_shape = relationships()[1..2].to_vec();
    wrong_shape[0].subject_type = "team".into();
    assert_eq!(
        SnapshotTree::build(&policy, wrong_shape, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .err()
            .unwrap(),
        SnapshotError::UnsupportedRelationshipShape
    );
}

#[test]
fn rejects_malformed_stored_context_and_limit_overruns() {
    let policy = policy();
    let mut malformed = relationships()[..1].to_vec();
    malformed[0]
        .stored_caveat
        .as_mut()
        .unwrap()
        .context
        .swap(0, 1);
    assert_eq!(
        SnapshotTree::build(&policy, malformed, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .err()
            .unwrap(),
        SnapshotError::ContextNotStrictlySorted
    );

    let mut missing = relationships()[..1].to_vec();
    missing[0].stored_caveat.as_mut().unwrap().context.pop();
    assert_eq!(
        SnapshotTree::build(&policy, missing, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .err()
            .unwrap(),
        SnapshotError::ContextParameterMismatch
    );

    let mut wrong_type = relationships()[..1].to_vec();
    wrong_type[0].stored_caveat.as_mut().unwrap().context[1].value = CaveatValue::Bool(false);
    assert_eq!(
        SnapshotTree::build(&policy, wrong_type, &TEST_SALT, TEST_SNAPSHOT_LIMITS)
            .err()
            .unwrap(),
        SnapshotError::ContextTypeMismatch
    );

    let records = relationships();
    assert_limit(
        &policy,
        records.clone(),
        SnapshotLimits {
            max_relationships: 4,
            ..TEST_SNAPSHOT_LIMITS
        },
        SnapshotLimit::Relationships,
    );
    assert_limit(
        &policy,
        records[..1].to_vec(),
        SnapshotLimits {
            max_relationship_bytes: 1,
            ..TEST_SNAPSHOT_LIMITS
        },
        SnapshotLimit::RelationshipBytes,
    );
    assert_limit(
        &policy,
        records[..1].to_vec(),
        SnapshotLimits {
            max_total_canonical_bytes: 1,
            ..TEST_SNAPSHOT_LIMITS
        },
        SnapshotLimit::TotalCanonicalBytes,
    );
}

#[test]
fn record_and_node_mutations_change_commitments_without_changing_identity_rules() {
    let records = relationships();
    let original = &records[0];
    let canonical = encode_relationship(original).unwrap();
    let original_leaf = relationship_leaf_digest(0, &TEST_SALT, &canonical).unwrap();

    let mut identity_mutations = Vec::new();
    for mutate in [
        |record: &mut raziel_snapshot::RelationshipRecord| record.resource_id.push('x'),
        |record: &mut raziel_snapshot::RelationshipRecord| record.subject_id.push('x'),
    ] {
        let mut record = original.clone();
        mutate(&mut record);
        identity_mutations.push(record);
    }
    for mutation in identity_mutations {
        assert_ne!(
            compare_relationship_identity(original, &mutation),
            std::cmp::Ordering::Equal
        );
        let mutated = encode_relationship(&mutation).unwrap();
        assert_ne!(
            relationship_leaf_digest(0, &TEST_SALT, &mutated).unwrap(),
            original_leaf
        );
    }

    let mut caveat_mutation = original.clone();
    caveat_mutation.stored_caveat.as_mut().unwrap().name = "other".into();
    let mut context_mutation = original.clone();
    context_mutation
        .stored_caveat
        .as_mut()
        .unwrap()
        .context
        .push(StoredContextEntry {
            name: "synthetic".into(),
            value: CaveatValue::String("test-only".into()),
        });

    for mutation in [caveat_mutation, context_mutation] {
        assert_eq!(
            compare_relationship_identity(original, &mutation),
            std::cmp::Ordering::Equal
        );
        let mutated = encode_relationship(&mutation).unwrap();
        assert_ne!(
            relationship_leaf_digest(0, &TEST_SALT, &mutated).unwrap(),
            original_leaf
        );
    }

    let left = [1; 32];
    let right = [2; 32];
    assert_ne!(
        relationship_node_digest(&left, &right).unwrap(),
        relationship_node_digest(&right, &left).unwrap()
    );
}

#[test]
fn private_types_redact_debug_output() {
    let policy = policy();
    let records = relationships();
    let record_debug = format!("{:?}", records[0]);
    assert!(record_debug.contains("<redacted>"));
    assert!(!record_debug.contains("synthetic-private-test"));

    let tree = SnapshotTree::build(&policy, records, &TEST_SALT, TEST_SNAPSHOT_LIMITS).unwrap();
    let opening = tree.opening(0).unwrap();
    let witness = RelationshipWitnessSetV1 {
        format_version: 1,
        snapshot_secret_salt: TEST_SALT,
        leaf_count: 5,
        entries: vec![opening.clone()],
    };
    assert!(format!("{opening:?}").contains("<redacted>"));
    assert!(format!("{witness:?}").contains("<redacted>"));
}

fn assert_limit(
    policy: &raziel_policy_ir::ValidatedPolicyProgram,
    records: Vec<raziel_snapshot::RelationshipRecord>,
    limits: SnapshotLimits,
    expected: SnapshotLimit,
) {
    assert_eq!(
        SnapshotTree::build(policy, records, &TEST_SALT, limits)
            .err()
            .unwrap(),
        SnapshotError::LimitExceeded(expected)
    );
}
