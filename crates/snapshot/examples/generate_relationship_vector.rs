use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use raziel_policy_ir::CaveatValue;
use raziel_snapshot::{
    CANONICAL_RELATIONSHIP_ENCODING_VERSION, RELATIONSHIP_EMPTY_DOMAIN, RELATIONSHIP_LEAF_DOMAIN,
    RELATIONSHIP_NODE_DOMAIN, RELATIONSHIP_ROOT_DOMAIN, RELATIONSHIP_TREE_VERSION,
    RelationshipRecord, SnapshotTree, empty_tree_digest, empty_tree_preimage, encode_relationship,
    relationship_leaf_digest, relationship_leaf_preimage, relationship_node_digest,
    relationship_node_preimage, relationship_root, relationship_root_preimage,
};
use serde_json::{Value, json};

#[path = "../tests/support/mod.rs"]
mod support;

use support::{TEST_SALT, TEST_SNAPSHOT_LIMITS, policy, relationships};

fn main() {
    let policy = policy();
    let relationships = relationships();
    let relationship_ids = [
        "synthetic-caveated-direct",
        "synthetic-direct",
        "synthetic-subject-set",
        "synthetic-second-resource",
        "synthetic-third-resource",
    ];
    let canonical = relationships
        .iter()
        .map(encode_relationship)
        .collect::<Result<Vec<_>, _>>()
        .expect("synthetic relationships must encode");
    let leaf_digests = canonical
        .iter()
        .enumerate()
        .map(|(index, bytes)| {
            relationship_leaf_digest(u32::try_from(index).unwrap(), &TEST_SALT, bytes).unwrap()
        })
        .collect::<Vec<_>>();

    let relationship_cases = relationships
        .iter()
        .enumerate()
        .map(|(index, relationship)| {
            let index = u32::try_from(index).unwrap();
            json!({
                "id": relationship_ids[index as usize],
                "private_test_only": true,
                "human_input": relationship_json(relationship),
                "canonical_byte_length": canonical[index as usize].len(),
                "canonical_hex": hex(&canonical[index as usize]),
                "leaf_index": index,
                "leaf_preimage_hex": hex(
                    &relationship_leaf_preimage(index, &TEST_SALT, &canonical[index as usize])
                        .unwrap()
                ),
                "leaf_digest_hex": hex(&leaf_digests[index as usize]),
                "expected_rust_result": "match",
                "expected_typescript_parser_result": "match_human_input"
            })
        })
        .collect::<Vec<_>>();

    let tree_cases = [0_usize, 1, 2, 3, 5]
        .into_iter()
        .map(|count| {
            tree_case(
                &policy,
                &relationships,
                &relationship_ids,
                &leaf_digests,
                count,
            )
        })
        .collect::<Vec<_>>();

    let first_offsets = canonical_offsets(&canonical[0]);
    let negative_cases = negative_cases(&canonical[0], &first_offsets);
    let vector = json!({
        "vector_format_version": 1,
        "private_test_only": true,
        "warning": "Synthetic private-test records and fixed salt. Never use this salt in production.",
        "hash_algorithm": "sha256",
        "canonical_relationship_version": CANONICAL_RELATIONSHIP_ENCODING_VERSION,
        "relationship_tree_version": RELATIONSHIP_TREE_VERSION,
        "domains": {
            "leaf": String::from_utf8(RELATIONSHIP_LEAF_DOMAIN.to_vec()).unwrap(),
            "node": String::from_utf8(RELATIONSHIP_NODE_DOMAIN.to_vec()).unwrap(),
            "empty": String::from_utf8(RELATIONSHIP_EMPTY_DOMAIN.to_vec()).unwrap(),
            "root": String::from_utf8(RELATIONSHIP_ROOT_DOMAIN.to_vec()).unwrap()
        },
        "fixed_private_test_salt_hex": hex(&TEST_SALT),
        "test_ceilings": {
            "max_relationships": TEST_SNAPSHOT_LIMITS.max_relationships,
            "max_relationship_bytes": TEST_SNAPSHOT_LIMITS.max_relationship_bytes,
            "max_total_canonical_bytes": TEST_SNAPSHOT_LIMITS.max_total_canonical_bytes
        },
        "relationship_cases": relationship_cases,
        "tree_cases": tree_cases,
        "negative_cases": negative_cases
    });

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = workspace.join("tests/vectors/relationship-tree-v1.json");
    fs::create_dir_all(output.parent().unwrap()).expect("create vector directory");
    fs::write(
        output,
        format!("{}\n", serde_json::to_string_pretty(&vector).unwrap()),
    )
    .expect("write relationship vector");
}

fn tree_case(
    policy: &raziel_policy_ir::ValidatedPolicyProgram,
    relationships: &[RelationshipRecord],
    relationship_ids: &[&str],
    all_leaf_digests: &[[u8; 32]],
    count: usize,
) -> Value {
    let tree = SnapshotTree::build(
        policy,
        relationships[..count].to_vec(),
        &TEST_SALT,
        TEST_SNAPSHOT_LIMITS,
    )
    .unwrap();
    let mut nodes = Vec::new();
    let (tree_digest, _) = if count == 0 {
        (empty_tree_digest().unwrap(), 0)
    } else {
        build_trace(&all_leaf_digests[..count], 0, &mut nodes)
    };
    let leaf_count = u32::try_from(count).unwrap();
    let expected_root = relationship_root(leaf_count, &TEST_SALT, &tree_digest).unwrap();
    assert_eq!(tree.root(), expected_root);

    let mut levels: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for node in &nodes {
        let height = node["height"].as_u64().unwrap() as u32;
        levels
            .entry(height)
            .or_default()
            .push(node["digest_hex"].as_str().unwrap().to_owned());
    }
    let tree_levels = levels
        .into_iter()
        .map(|(height, digests)| json!({ "height": height, "digests_hex": digests }))
        .collect::<Vec<_>>();

    let openings = (0..leaf_count)
        .map(|leaf_index| {
            let opening = tree.opening(leaf_index).unwrap();
            json!({
                "leaf_index": leaf_index,
                "relationship_id": relationship_ids[leaf_index as usize],
                "sibling_hashes_hex": opening
                    .sibling_hashes
                    .iter()
                    .map(|digest| hex(digest))
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();

    json!({
        "id": format!("synthetic-tree-{count}-leaves"),
        "leaf_count": count,
        "relationship_ids": relationship_ids[..count],
        "leaf_digests_hex": all_leaf_digests[..count]
            .iter()
            .map(|digest| hex(digest))
            .collect::<Vec<_>>(),
        "empty_preimage_hex": (count == 0).then(|| hex(&empty_tree_preimage().unwrap())),
        "node_steps": nodes,
        "tree_levels": tree_levels,
        "tree_digest_hex": hex(&tree_digest),
        "root_preimage_hex": hex(
            &relationship_root_preimage(leaf_count, &TEST_SALT, &tree_digest).unwrap()
        ),
        "expected_root_hex": hex(&expected_root),
        "openings": openings,
        "expected_rust_result": "match",
        "expected_typescript_parser_result": "match_structure"
    })
}

fn build_trace(leaves: &[[u8; 32]], start: usize, nodes: &mut Vec<Value>) -> ([u8; 32], u32) {
    if leaves.len() == 1 {
        return (leaves[0], 0);
    }
    let split = largest_power_of_two_less_than(leaves.len());
    let (left, left_height) = build_trace(&leaves[..split], start, nodes);
    let (right, right_height) = build_trace(&leaves[split..], start + split, nodes);
    let digest = relationship_node_digest(&left, &right).unwrap();
    let height = left_height.max(right_height) + 1;
    nodes.push(json!({
        "leaf_start": start,
        "leaf_count": leaves.len(),
        "height": height,
        "left_digest_hex": hex(&left),
        "right_digest_hex": hex(&right),
        "preimage_hex": hex(&relationship_node_preimage(&left, &right).unwrap()),
        "digest_hex": hex(&digest)
    }));
    (digest, height)
}

fn relationship_json(relationship: &RelationshipRecord) -> Value {
    json!({
        "resource_type": relationship.resource_type,
        "resource_id": relationship.resource_id,
        "relation": relationship.relation,
        "subject_type": relationship.subject_type,
        "subject_id": relationship.subject_id,
        "subject_relation": relationship.subject_relation,
        "stored_caveat": relationship.stored_caveat.as_ref().map(|caveat| json!({
            "name": caveat.name,
            "context": caveat.context.iter().map(|entry| json!({
                "name": entry.name,
                "value": match &entry.value {
                    CaveatValue::Bool(value) => json!({ "type": "bool", "value": value }),
                    CaveatValue::Int(value) => {
                        json!({ "type": "int", "value": value.to_string() })
                    }
                    CaveatValue::String(value) => {
                        json!({ "type": "string", "value": value })
                    }
                }
            })).collect::<Vec<_>>()
        }))
    })
}

fn negative_cases(canonical: &[u8], offsets: &CanonicalOffsets) -> Vec<Value> {
    vec![
        byte_mutation(
            "unsupported-relationship-version",
            "canonical_relationship",
            0,
            4,
            "00000002",
            "reject",
        ),
        byte_mutation(
            "oversized-resource-type-length",
            "canonical_relationship",
            4,
            4,
            "ffffffff",
            "reject",
        ),
        byte_mutation(
            "invalid-resource-type-utf8",
            "canonical_relationship",
            8,
            1,
            "ff",
            "reject",
        ),
        byte_mutation(
            "invalid-subject-relation-option-tag",
            "canonical_relationship",
            offsets.subject_relation_tag,
            1,
            "02",
            "reject",
        ),
        byte_mutation(
            "invalid-caveat-option-tag",
            "canonical_relationship",
            offsets.caveat_tag,
            1,
            "02",
            "reject",
        ),
        byte_mutation(
            "unknown-bool-value-tag",
            "canonical_relationship",
            offsets.bool_value_tag,
            1,
            "00",
            "reject",
        ),
        byte_mutation(
            "invalid-bool-payload",
            "canonical_relationship",
            offsets.bool_payload,
            1,
            "02",
            "reject",
        ),
        mutation(
            "truncated-relationship",
            "canonical_relationship",
            "truncate",
            canonical.len() - 1,
            1,
            "",
            "reject",
            "not_applicable",
        ),
        mutation(
            "trailing-relationship-byte",
            "canonical_relationship",
            "append",
            canonical.len(),
            0,
            "00",
            "reject",
            "not_applicable",
        ),
        semantic_case(
            "mutated-resource-type",
            "relationship.resource_type",
            "reject",
        ),
        semantic_case(
            "mutated-resource-id",
            "relationship.resource_id",
            "distinct_digest",
        ),
        semantic_case("mutated-relation", "relationship.relation", "reject"),
        semantic_case(
            "mutated-subject-type",
            "relationship.subject_type",
            "reject",
        ),
        semantic_case(
            "mutated-subject-id",
            "relationship.subject_id",
            "distinct_digest",
        ),
        semantic_case(
            "mutated-subject-relation",
            "relationship.subject_relation",
            "reject",
        ),
        semantic_case("mutated-caveat", "relationship.stored_caveat", "reject"),
        semantic_case(
            "mutated-context-name",
            "relationship.context.name",
            "reject",
        ),
        semantic_case(
            "mutated-context-bool",
            "relationship.context.bool",
            "distinct_digest",
        ),
        semantic_case(
            "mutated-context-int",
            "relationship.context.int",
            "distinct_digest",
        ),
        semantic_case(
            "mutated-context-string",
            "relationship.context.string",
            "distinct_digest",
        ),
        semantic_case("mutated-salt", "snapshot.salt", "root_mismatch"),
        semantic_case("mutated-leaf-index", "opening.leaf_index", "root_mismatch"),
        semantic_case("mutated-sibling", "opening.sibling", "root_mismatch"),
        semantic_case(
            "extra-sibling-depth",
            "opening.depth",
            "sibling_count_mismatch",
        ),
        semantic_case("mutated-root", "snapshot.root", "root_mismatch"),
        semantic_case(
            "reversed-node-children",
            "tree.node_order",
            "distinct_digest",
        ),
        semantic_case("mutated-leaf-count", "snapshot.leaf_count", "root_mismatch"),
        semantic_case(
            "unsorted-relationships",
            "snapshot.relationship_order",
            "reject",
        ),
        semantic_case(
            "duplicate-relationship-identity",
            "snapshot.duplicate_identity",
            "reject",
        ),
        semantic_case(
            "phantom-index-three",
            "opening.phantom_index",
            "leaf_index_out_of_range",
        ),
        semantic_case(
            "mixed-snapshot-salt",
            "witness.shared_salt",
            "root_mismatch",
        ),
    ]
}

fn byte_mutation(
    id: &str,
    target: &str,
    offset: usize,
    remove_bytes: usize,
    value_hex: &str,
    typescript_result: &str,
) -> Value {
    mutation(
        id,
        target,
        "replace",
        offset,
        remove_bytes,
        value_hex,
        typescript_result,
        "not_applicable",
    )
}

#[allow(clippy::too_many_arguments)]
fn mutation(
    id: &str,
    target: &str,
    operation: &str,
    offset: usize,
    remove_bytes: usize,
    value_hex: &str,
    typescript_result: &str,
    rust_result: &str,
) -> Value {
    json!({
        "id": id,
        "target": target,
        "mutation": {
            "operation": operation,
            "offset": offset,
            "remove_bytes": remove_bytes,
            "value_hex": value_hex
        },
        "expected_rust_result": rust_result,
        "expected_typescript_parser_result": typescript_result
    })
}

fn semantic_case(id: &str, target: &str, rust_result: &str) -> Value {
    json!({
        "id": id,
        "target": target,
        "mutation": null,
        "expected_rust_result": rust_result,
        "expected_typescript_parser_result": "not_checked"
    })
}

struct CanonicalOffsets {
    subject_relation_tag: usize,
    caveat_tag: usize,
    bool_value_tag: usize,
    bool_payload: usize,
}

fn canonical_offsets(bytes: &[u8]) -> CanonicalOffsets {
    let mut offset = 4;
    for _ in 0..5 {
        offset = skip_length_delimited(bytes, offset);
    }
    let subject_relation_tag = offset;
    assert_eq!(bytes[offset], 0);
    offset += 1;
    let caveat_tag = offset;
    assert_eq!(bytes[offset], 1);
    offset += 1;
    offset = skip_length_delimited(bytes, offset);
    let context_count = read_u32(bytes, offset);
    assert_eq!(context_count, 3);
    offset += 4;
    offset = skip_length_delimited(bytes, offset);
    let bool_value_tag = offset;
    assert_eq!(bytes[offset], 1);
    let bool_payload = offset + 1;
    assert_eq!(bytes[bool_payload], 1);
    CanonicalOffsets {
        subject_relation_tag,
        caveat_tag,
        bool_value_tag,
        bool_payload,
    }
}

fn skip_length_delimited(bytes: &[u8], offset: usize) -> usize {
    offset + 4 + read_u32(bytes, offset) as usize
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn largest_power_of_two_less_than(value: usize) -> usize {
    1_usize << (usize::BITS - 1 - (value - 1).leading_zeros())
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
