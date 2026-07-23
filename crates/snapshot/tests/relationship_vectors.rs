mod support;

use std::collections::{BTreeMap, BTreeSet};

use raziel_policy_ir::CaveatValue;
use raziel_snapshot::{
    CANONICAL_RELATIONSHIP_ENCODING_VERSION, RELATIONSHIP_EMPTY_DOMAIN, RELATIONSHIP_LEAF_DOMAIN,
    RELATIONSHIP_NODE_DOMAIN, RELATIONSHIP_ROOT_DOMAIN, RELATIONSHIP_TREE_VERSION,
    RELATIONSHIP_WITNESS_SET_VERSION, RelationshipRecord, RelationshipWitnessSetV1, SnapshotError,
    SnapshotLimits, SnapshotTree, StoredCaveat, StoredContextEntry, empty_tree_digest,
    empty_tree_preimage, encode_relationship, relationship_leaf_digest, relationship_leaf_preimage,
    relationship_node_digest, relationship_node_preimage, relationship_root,
    relationship_root_preimage, verify_opening, verify_witness_set,
};
use serde::Deserialize;

use support::{TEST_SALT, TEST_SNAPSHOT_LIMITS, policy, relationships};

const VECTOR_JSON: &str = include_str!("../../../tests/vectors/relationship-tree-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorDocument {
    vector_format_version: u32,
    private_test_only: bool,
    warning: String,
    hash_algorithm: String,
    canonical_relationship_version: u32,
    relationship_tree_version: u32,
    domains: Domains,
    fixed_private_test_salt_hex: String,
    test_ceilings: VectorLimits,
    relationship_cases: Vec<RelationshipCase>,
    tree_cases: Vec<TreeCase>,
    negative_cases: Vec<NegativeCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Domains {
    leaf: String,
    node: String,
    empty: String,
    root: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorLimits {
    max_relationships: u32,
    max_relationship_bytes: u32,
    max_total_canonical_bytes: u64,
}

impl VectorLimits {
    const fn snapshot_limits(&self) -> SnapshotLimits {
        SnapshotLimits {
            max_relationships: self.max_relationships,
            max_relationship_bytes: self.max_relationship_bytes,
            max_total_canonical_bytes: self.max_total_canonical_bytes,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipCase {
    id: String,
    private_test_only: bool,
    human_input: HumanRelationship,
    canonical_byte_length: usize,
    canonical_hex: String,
    leaf_index: u32,
    leaf_preimage_hex: String,
    leaf_digest_hex: String,
    expected_rust_result: String,
    expected_typescript_parser_result: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanRelationship {
    resource_type: String,
    resource_id: String,
    relation: String,
    subject_type: String,
    subject_id: String,
    subject_relation: Option<String>,
    stored_caveat: Option<HumanCaveat>,
}

impl HumanRelationship {
    fn into_record(self) -> RelationshipRecord {
        RelationshipRecord {
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            relation: self.relation,
            subject_type: self.subject_type,
            subject_id: self.subject_id,
            subject_relation: self.subject_relation,
            stored_caveat: self.stored_caveat.map(HumanCaveat::into_caveat),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanCaveat {
    name: String,
    context: Vec<HumanContextEntry>,
}

impl HumanCaveat {
    fn into_caveat(self) -> StoredCaveat {
        StoredCaveat {
            name: self.name,
            context: self
                .context
                .into_iter()
                .map(HumanContextEntry::into_entry)
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanContextEntry {
    name: String,
    value: HumanValue,
}

impl HumanContextEntry {
    fn into_entry(self) -> StoredContextEntry {
        StoredContextEntry {
            name: self.name,
            value: self.value.into_value(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum HumanValue {
    Bool(bool),
    Int(String),
    String(String),
}

impl HumanValue {
    fn into_value(self) -> CaveatValue {
        match self {
            Self::Bool(value) => CaveatValue::Bool(value),
            Self::Int(value) => CaveatValue::Int(value.parse().unwrap()),
            Self::String(value) => CaveatValue::String(value),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeCase {
    id: String,
    leaf_count: u32,
    relationship_ids: Vec<String>,
    leaf_digests_hex: Vec<String>,
    empty_preimage_hex: Option<String>,
    node_steps: Vec<NodeStep>,
    tree_levels: Vec<TreeLevel>,
    tree_digest_hex: String,
    root_preimage_hex: String,
    expected_root_hex: String,
    openings: Vec<Opening>,
    expected_rust_result: String,
    expected_typescript_parser_result: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeStep {
    leaf_start: u32,
    leaf_count: u32,
    height: u32,
    left_digest_hex: String,
    right_digest_hex: String,
    preimage_hex: String,
    digest_hex: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeLevel {
    height: u32,
    digests_hex: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Opening {
    leaf_index: u32,
    relationship_id: String,
    sibling_hashes_hex: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NegativeCase {
    id: String,
    target: String,
    mutation: Option<Mutation>,
    expected_rust_result: String,
    expected_typescript_parser_result: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    operation: String,
    offset: usize,
    remove_bytes: usize,
    value_hex: String,
}

#[test]
fn checked_in_relationship_vector_matches_rust_commitments_and_openings() {
    let vector: VectorDocument = serde_json::from_str(VECTOR_JSON).unwrap();
    assert_eq!(vector.vector_format_version, 1);
    assert!(vector.private_test_only);
    assert!(vector.warning.contains("Never use this salt in production"));
    assert_eq!(vector.hash_algorithm, "sha256");
    assert_eq!(
        vector.canonical_relationship_version,
        CANONICAL_RELATIONSHIP_ENCODING_VERSION
    );
    assert_eq!(vector.relationship_tree_version, RELATIONSHIP_TREE_VERSION);
    assert_eq!(vector.domains.leaf.as_bytes(), RELATIONSHIP_LEAF_DOMAIN);
    assert_eq!(vector.domains.node.as_bytes(), RELATIONSHIP_NODE_DOMAIN);
    assert_eq!(vector.domains.empty.as_bytes(), RELATIONSHIP_EMPTY_DOMAIN);
    assert_eq!(vector.domains.root.as_bytes(), RELATIONSHIP_ROOT_DOMAIN);
    assert_eq!(
        digest(&vector.fixed_private_test_salt_hex),
        TEST_SALT,
        "the checked-in salt is test-only and intentionally fixed"
    );

    let mut records = BTreeMap::new();
    let mut leaf_digests = BTreeMap::new();
    for relationship_case in vector.relationship_cases {
        assert!(relationship_case.private_test_only);
        assert_eq!(relationship_case.expected_rust_result, "match");
        assert_eq!(
            relationship_case.expected_typescript_parser_result,
            "match_human_input"
        );
        let record = relationship_case.human_input.into_record();
        let canonical = encode_relationship(&record).unwrap();
        assert_eq!(canonical.len(), relationship_case.canonical_byte_length);
        assert_eq!(hex(&canonical), relationship_case.canonical_hex);
        assert_eq!(
            hex(
                &relationship_leaf_preimage(relationship_case.leaf_index, &TEST_SALT, &canonical)
                    .unwrap()
            ),
            relationship_case.leaf_preimage_hex
        );
        let leaf =
            relationship_leaf_digest(relationship_case.leaf_index, &TEST_SALT, &canonical).unwrap();
        assert_eq!(hex(&leaf), relationship_case.leaf_digest_hex);
        leaf_digests.insert(relationship_case.id.clone(), leaf);
        records.insert(relationship_case.id, record);
    }

    let policy = policy();
    let limits = vector.test_ceilings.snapshot_limits();
    for tree_case in vector.tree_cases {
        assert_eq!(tree_case.expected_rust_result, "match", "{}", tree_case.id);
        assert_eq!(
            tree_case.expected_typescript_parser_result, "match_structure",
            "{}",
            tree_case.id
        );
        assert_eq!(
            tree_case.relationship_ids.len(),
            tree_case.leaf_count as usize
        );
        assert_eq!(
            tree_case.leaf_digests_hex.len(),
            tree_case.leaf_count as usize
        );
        let tree_records = tree_case
            .relationship_ids
            .iter()
            .map(|id| records.get(id).unwrap().clone())
            .collect::<Vec<_>>();
        let tree = SnapshotTree::build(&policy, tree_records, &TEST_SALT, limits).unwrap();
        assert_eq!(hex(&tree.root()), tree_case.expected_root_hex);

        for (id, expected_leaf) in tree_case
            .relationship_ids
            .iter()
            .zip(&tree_case.leaf_digests_hex)
        {
            assert_eq!(hex(leaf_digests.get(id).unwrap()), *expected_leaf);
        }
        for node in &tree_case.node_steps {
            assert!(node.leaf_count > 1);
            assert!(node.leaf_start < tree_case.leaf_count);
            assert!(node.height > 0);
            let left = digest(&node.left_digest_hex);
            let right = digest(&node.right_digest_hex);
            assert_eq!(
                hex(&relationship_node_preimage(&left, &right).unwrap()),
                node.preimage_hex
            );
            assert_eq!(
                hex(&relationship_node_digest(&left, &right).unwrap()),
                node.digest_hex
            );
        }
        validate_levels(&tree_case.tree_levels, &tree_case.node_steps);

        let tree_digest = digest(&tree_case.tree_digest_hex);
        if tree_case.leaf_count == 0 {
            assert_eq!(
                tree_case.empty_preimage_hex.as_deref(),
                Some(hex(&empty_tree_preimage().unwrap()).as_str())
            );
            assert_eq!(tree_digest, empty_tree_digest().unwrap());
        } else {
            assert!(tree_case.empty_preimage_hex.is_none());
        }
        assert_eq!(
            hex(
                &relationship_root_preimage(tree_case.leaf_count, &TEST_SALT, &tree_digest)
                    .unwrap()
            ),
            tree_case.root_preimage_hex
        );
        assert_eq!(
            hex(&relationship_root(tree_case.leaf_count, &TEST_SALT, &tree_digest).unwrap()),
            tree_case.expected_root_hex
        );

        assert_eq!(tree_case.openings.len(), tree_case.leaf_count as usize);
        for opening_case in tree_case.openings {
            assert_eq!(
                opening_case.relationship_id,
                tree_case.relationship_ids[opening_case.leaf_index as usize]
            );
            let opening = tree.opening(opening_case.leaf_index).unwrap();
            assert_eq!(
                opening
                    .sibling_hashes
                    .iter()
                    .map(|digest| hex(digest))
                    .collect::<Vec<_>>(),
                opening_case.sibling_hashes_hex
            );
        }
    }

    validate_negative_catalog(&vector.negative_cases);
}

#[test]
fn checked_in_rust_negative_cases_execute_the_recorded_result() {
    let vector: VectorDocument = serde_json::from_str(VECTOR_JSON).unwrap();
    validate_negative_catalog(&vector.negative_cases);

    for case in &vector.negative_cases {
        if case.expected_rust_result == "not_applicable" {
            continue;
        }
        assert_eq!(
            execute_rust_negative_case(&case.id),
            case.expected_rust_result,
            "{}",
            case.id
        );
    }
}

fn validate_levels(levels: &[TreeLevel], nodes: &[NodeStep]) {
    let expected = nodes
        .iter()
        .fold(BTreeMap::<u32, Vec<String>>::new(), |mut levels, node| {
            levels
                .entry(node.height)
                .or_default()
                .push(node.digest_hex.clone());
            levels
        });
    let observed = levels
        .iter()
        .map(|level| (level.height, level.digests_hex.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observed, expected);
}

fn validate_negative_catalog(cases: &[NegativeCase]) {
    let required = [
        "unsupported-relationship-version",
        "invalid-subject-relation-option-tag",
        "invalid-caveat-option-tag",
        "unknown-bool-value-tag",
        "mutated-resource-type",
        "mutated-resource-id",
        "mutated-relation",
        "mutated-subject-type",
        "mutated-subject-id",
        "mutated-subject-relation",
        "mutated-caveat",
        "mutated-context-name",
        "mutated-context-bool",
        "mutated-context-int",
        "mutated-context-string",
        "mutated-salt",
        "mutated-leaf-index",
        "mutated-sibling",
        "extra-sibling-depth",
        "mutated-root",
        "reversed-node-children",
        "mutated-leaf-count",
        "unsorted-relationships",
        "duplicate-relationship-identity",
        "phantom-index-three",
        "mixed-snapshot-salt",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let observed = cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(required.is_subset(&observed));

    for case in cases {
        assert!(!case.target.is_empty());
        assert!(!case.expected_rust_result.is_empty());
        assert!(!case.expected_typescript_parser_result.is_empty());
        if let Some(mutation) = &case.mutation {
            assert!(matches!(
                mutation.operation.as_str(),
                "append" | "replace" | "truncate"
            ));
            assert!(mutation.offset.checked_add(mutation.remove_bytes).is_some());
            decode_hex(&mutation.value_hex);
        }
    }
}

fn execute_rust_negative_case(id: &str) -> &'static str {
    match id {
        "mutated-resource-type"
        | "mutated-resource-id"
        | "mutated-relation"
        | "mutated-subject-type"
        | "mutated-subject-id"
        | "mutated-subject-relation"
        | "mutated-caveat"
        | "mutated-context-name"
        | "mutated-context-bool"
        | "mutated-context-int"
        | "mutated-context-string" => execute_record_mutation(id),
        "mutated-salt"
        | "mutated-leaf-index"
        | "mutated-sibling"
        | "extra-sibling-depth"
        | "mutated-root"
        | "mutated-leaf-count"
        | "phantom-index-three"
        | "mixed-snapshot-salt" => execute_opening_mutation(id),
        "reversed-node-children" => {
            let left = [1; 32];
            let right = [2; 32];
            if relationship_node_digest(&left, &right).unwrap()
                != relationship_node_digest(&right, &left).unwrap()
            {
                "distinct_digest"
            } else {
                "same_digest"
            }
        }
        "unsorted-relationships" => {
            let mut records = relationships();
            records.swap(0, 1);
            build_result(records)
        }
        "duplicate-relationship-identity" => {
            let original = relationships()[0].clone();
            let mut duplicate = original.clone();
            duplicate.stored_caveat.as_mut().unwrap().context[1].value = CaveatValue::Int(501);
            build_result(vec![original, duplicate])
        }
        _ => panic!("unhandled Rust negative vector: {id}"),
    }
}

fn execute_record_mutation(id: &str) -> &'static str {
    let policy = policy();
    let mut record = relationships()[0].clone();
    let original = encode_relationship(&record).unwrap();
    let original_digest = relationship_leaf_digest(0, &TEST_SALT, &original).unwrap();

    match id {
        "mutated-resource-type" => record.resource_type = "Dataset".into(),
        "mutated-resource-id" => record.resource_id.push('x'),
        "mutated-relation" => record.relation = "unknown".into(),
        "mutated-subject-type" => record.subject_type = "team".into(),
        "mutated-subject-id" => record.subject_id.push('x'),
        "mutated-subject-relation" => record.subject_relation = Some("member".into()),
        "mutated-caveat" => record.stored_caveat.as_mut().unwrap().name = "other".into(),
        "mutated-context-name" => {
            record.stored_caveat.as_mut().unwrap().context[0].name = "approvez".into();
        }
        "mutated-context-bool" => {
            record.stored_caveat.as_mut().unwrap().context[0].value = CaveatValue::Bool(false);
        }
        "mutated-context-int" => {
            record.stored_caveat.as_mut().unwrap().context[1].value = CaveatValue::Int(501);
        }
        "mutated-context-string" => {
            record.stored_caveat.as_mut().unwrap().context[2].value =
                CaveatValue::String("other-region".into());
        }
        _ => panic!("unhandled relationship mutation: {id}"),
    }

    if SnapshotTree::build(
        &policy,
        vec![record.clone()],
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .is_err()
    {
        return "reject";
    }
    let mutated = encode_relationship(&record).unwrap();
    if relationship_leaf_digest(0, &TEST_SALT, &mutated).unwrap() != original_digest {
        "distinct_digest"
    } else {
        "same_digest"
    }
}

fn execute_opening_mutation(id: &str) -> &'static str {
    let policy = policy();
    let tree = SnapshotTree::build(
        &policy,
        relationships()[..3].to_vec(),
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let expected_root = tree.root();

    match id {
        "mutated-salt" => opening_result(verify_opening(
            &policy,
            &[0x5b; 32],
            3,
            &tree.opening(0).unwrap(),
            &expected_root,
            TEST_SNAPSHOT_LIMITS,
        )),
        "mutated-leaf-index" => {
            let mut opening = tree.opening(0).unwrap();
            opening.leaf_index = 1;
            opening_result(verify_opening(
                &policy,
                &TEST_SALT,
                3,
                &opening,
                &expected_root,
                TEST_SNAPSHOT_LIMITS,
            ))
        }
        "mutated-sibling" => {
            let mut opening = tree.opening(0).unwrap();
            opening.sibling_hashes[0][0] ^= 1;
            opening_result(verify_opening(
                &policy,
                &TEST_SALT,
                3,
                &opening,
                &expected_root,
                TEST_SNAPSHOT_LIMITS,
            ))
        }
        "extra-sibling-depth" => {
            let mut opening = tree.opening(0).unwrap();
            opening.sibling_hashes.push([0; 32]);
            opening_result(verify_opening(
                &policy,
                &TEST_SALT,
                3,
                &opening,
                &expected_root,
                TEST_SNAPSHOT_LIMITS,
            ))
        }
        "mutated-root" => {
            let mut root = expected_root;
            root[0] ^= 1;
            opening_result(verify_opening(
                &policy,
                &TEST_SALT,
                3,
                &tree.opening(0).unwrap(),
                &root,
                TEST_SNAPSHOT_LIMITS,
            ))
        }
        "mutated-leaf-count" => opening_result(verify_opening(
            &policy,
            &TEST_SALT,
            4,
            &tree.opening(0).unwrap(),
            &expected_root,
            TEST_SNAPSHOT_LIMITS,
        )),
        "phantom-index-three" => {
            let mut opening = tree.opening(2).unwrap();
            opening.leaf_index = 3;
            opening_result(verify_opening(
                &policy,
                &TEST_SALT,
                3,
                &opening,
                &expected_root,
                TEST_SNAPSHOT_LIMITS,
            ))
        }
        "mixed-snapshot-salt" => {
            let witness = RelationshipWitnessSetV1 {
                format_version: RELATIONSHIP_WITNESS_SET_VERSION,
                snapshot_secret_salt: [0x5b; 32],
                leaf_count: 3,
                entries: vec![tree.opening(0).unwrap()],
            };
            match verify_witness_set(&policy, &witness, &expected_root, TEST_SNAPSHOT_LIMITS) {
                Err(SnapshotError::RootMismatch) => "root_mismatch",
                Err(_) => "other_error",
                Ok(_) => "accept",
            }
        }
        _ => panic!("unhandled opening mutation: {id}"),
    }
}

fn opening_result(result: Result<(), SnapshotError>) -> &'static str {
    match result {
        Err(SnapshotError::RootMismatch) => "root_mismatch",
        Err(SnapshotError::SiblingCountMismatch) => "sibling_count_mismatch",
        Err(SnapshotError::LeafIndexOutOfRange) => "leaf_index_out_of_range",
        Err(_) => "other_error",
        Ok(()) => "accept",
    }
}

fn build_result(records: Vec<RelationshipRecord>) -> &'static str {
    if SnapshotTree::build(&policy(), records, &TEST_SALT, TEST_SNAPSHOT_LIMITS).is_err() {
        "reject"
    } else {
        "accept"
    }
}

fn digest(value: &str) -> [u8; 32] {
    decode_hex(value).try_into().unwrap()
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    (0..value.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).unwrap())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}
