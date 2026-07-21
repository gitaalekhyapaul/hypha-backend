use std::fs;
use std::path::PathBuf;

use prost::Message;
use raziel_canonical::{
    CANONICAL_POLICY_ENCODING_VERSION, POLICY_HASH_DOMAIN, encode_policy, policy_hash,
    policy_hash_preimage,
};
use raziel_policy_ir::{PolicyLimits, ValidationCeilings, decode_policy_program_strict};
use raziel_proto::raziel::v1 as proto;
use serde_json::json;

const POLICY_IR: &[u8] = include_bytes!(
    "../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);

const TRUSTED_CEILINGS: ValidationCeilings = ValidationCeilings {
    policy_limits: PolicyLimits {
        max_expression_nodes: 64,
        max_caveat_nodes: 64,
        max_path_depth: 4,
        max_relationship_witnesses: 64,
        max_merkle_depth: 32,
        max_string_bytes: 256,
        max_context_entries: 32,
        max_total_private_input_bytes: 4096,
    },
    max_definitions: 16,
    max_relations_per_definition: 16,
    max_allowed_subject_types_per_relation: 16,
    max_permissions: 32,
    max_caveats: 16,
    max_parameters_per_caveat: 16,
    max_syntax_depth: 16,
};

fn main() {
    let program =
        decode_policy_program_strict(POLICY_IR, byte_length(POLICY_IR), &TRUSTED_CEILINGS)
            .expect("reviewed PolicyIR must pass strict ingress");
    let payload = encode_policy(&program).expect("reviewed PolicyIR must encode");
    let preimage = policy_hash_preimage(&program).expect("reviewed PolicyIR must frame");
    let digest = policy_hash(&program).expect("reviewed PolicyIR must hash");
    let payload_length_hex = hex(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    let wire = proto::PolicyProgram::decode(POLICY_IR).expect("reviewed PolicyIR must decode");

    let mut unsupported_ir_version = wire.clone();
    unsupported_ir_version.ir_version = 2;

    let mut unsorted_definitions = wire.clone();
    assert!(
        unsorted_definitions.definitions[0].name < unsorted_definitions.definitions[1].name,
        "reviewed definitions must begin in strict order"
    );
    unsorted_definitions.definitions.swap(0, 1);

    let mut unknown_input_source = wire.clone();
    first_parameter_mut(&mut unknown_input_source).source = 99;

    let mut unknown_value_type = wire;
    first_parameter_mut(&mut unknown_value_type).value_type = 99;

    let vector = json!({
        "vector_format_version": 1,
        "primitive_cases": {
            "i64": [
                { "value": i64::MIN.to_string(), "canonical_hex": "8000000000000000" },
                { "value": "-1", "canonical_hex": "ffffffffffffffff" },
                { "value": "0", "canonical_hex": "0000000000000000" },
                { "value": "1", "canonical_hex": "0000000000000001" },
                { "value": i64::MAX.to_string(), "canonical_hex": "7fffffffffffffff" }
            ],
            "utf8_string": [
                { "id": "nfd-e-acute", "value": "e\u{301}", "canonical_hex": "0000000365cc81" },
                { "id": "nfc-e-acute", "value": "\u{e9}", "canonical_hex": "00000002c3a9" }
            ],
            "optional_string": [
                { "id": "absent", "value": null, "canonical_hex": "00" },
                { "id": "present-empty", "value": "", "canonical_hex": "0100000000" }
            ]
        },
        "policy_cases": [{
            "id": "supported-policy-ir-v1",
            "human_input_path": "tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.json",
            "wire_input_path": "tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb",
            "hash_algorithm": "sha256",
            "domain_separator": String::from_utf8(POLICY_HASH_DOMAIN.to_vec()).unwrap(),
            "canonical_encoding_version": CANONICAL_POLICY_ENCODING_VERSION,
            "canonical_payload_byte_length": payload.len(),
            "canonical_payload_hex": hex(&payload),
            "hash_preimage_byte_length": preimage.len(),
            "hash_preimage_hex": hex(&preimage),
            "expected_digest_hex": hex(&digest),
            "expected_rust_result": "match",
            "expected_typescript_parser_result": "match_human_input"
        }],
        "negative_cases": [
            mutation("unsupported-canonical-version", "canonical_payload", "replace", 0, 4, "00000002", "not_applicable"),
            canonical_mutation(&payload, "unsupported-policy-ir-version", 4, &[0, 0, 0, 1], "00000002"),
            mutation("truncated-canonical-payload", "canonical_payload", "truncate", payload.len() - 1, 1, "", "not_applicable"),
            mutation("trailing-canonical-payload-byte", "canonical_payload", "append", payload.len(), 0, "00", "not_applicable"),
            mutation("invalid-compiler-name-utf8", "canonical_payload", "replace", 12, 1, "ff", "not_applicable"),
            canonical_mutation(&payload, "invalid-optional-presence-tag", 120, &[0], "02"),
            canonical_mutation(&payload, "unknown-permission-expression-tag", 460, &[1], "00"),
            canonical_mutation(&payload, "unknown-parameter-input-source-tag", 918, &[1], "00"),
            canonical_mutation(&payload, "unknown-parameter-value-type-tag", 924, &[2], "00"),
            canonical_mutation(&payload, "unknown-caveat-expression-tag", 953, &[11], "00"),
            canonical_mutation(&payload, "invalid-boolean-tag", 959, &[1], "02"),
            canonical_mutation(&payload, "unknown-input-reference-source-tag", 962, &[1], "00"),
            canonical_mutation(&payload, "unknown-input-reference-value-type-tag", 968, &[2], "00"),
            mutation("mutated-domain-byte", "hash_preimage", "replace", 4, 1, "00", "not_applicable"),
            mutation("mutated-payload-length", "hash_preimage", "replace", 20, 4, &increment_u32_hex(&payload_length_hex), "not_applicable"),
            mutation("unknown-top-level-protobuf-field", "protobuf", "append", POLICY_IR.len(), 0, "980601", "unknown_field"),
            mutation("unknown-nested-protobuf-field", "protobuf", "append", POLICY_IR.len(), 0, "2a03980601", "unknown_field"),
            mutation("duplicate-protobuf-ir-version", "protobuf", "append", POLICY_IR.len(), 0, "0801", "noncanonical_protobuf"),
            protobuf_replacement("reordered-protobuf-fields", &[0x12, 0x01, b'x', 0x08, 0x01], "noncanonical_protobuf"),
            protobuf_replacement("nonminimal-protobuf-varint", &[0x08, 0x81, 0x00], "noncanonical_protobuf"),
            protobuf_replacement("explicit-default-protobuf-field", &[0x08, 0x00], "noncanonical_protobuf"),
            protobuf_replacement("unsupported-protobuf-policy-ir-version", &unsupported_ir_version.encode_to_vec(), "unsupported_policy_ir_version"),
            protobuf_replacement("unsorted-protobuf-definitions", &unsorted_definitions.encode_to_vec(), "not_strictly_sorted_definitions"),
            protobuf_replacement("unknown-protobuf-input-source", &unknown_input_source.encode_to_vec(), "unknown_input_source"),
            protobuf_replacement("unknown-protobuf-value-type", &unknown_value_type.encode_to_vec(), "unknown_value_type"),
            mutation("mutated-digest", "digest", "replace", 0, 1, &flip_first_byte_hex(&digest), "mismatch")
        ]
    });

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = workspace.join("tests/vectors/policy-ir-v1.json");
    fs::create_dir_all(output.parent().unwrap()).expect("create vector directory");
    fs::write(
        output,
        format!("{}\n", serde_json::to_string_pretty(&vector).unwrap()),
    )
    .expect("write policy vector");
}

fn mutation(
    id: &str,
    target: &str,
    operation: &str,
    offset: usize,
    remove_bytes: usize,
    value_hex: &str,
    expected_result: &str,
) -> serde_json::Value {
    json!({
        "id": id,
        "target": target,
        "mutation": {
            "operation": operation,
            "offset": offset,
            "remove_bytes": remove_bytes,
            "value_hex": value_hex
        },
        "expected_rust_result": expected_result,
        "expected_typescript_parser_result": match target {
            "canonical_payload" | "hash_preimage" => "reject",
            "digest" => "not_checked",
            _ => "not_applicable"
        }
    })
}

fn canonical_mutation(
    payload: &[u8],
    id: &str,
    offset: usize,
    expected_bytes: &[u8],
    value_hex: &str,
) -> serde_json::Value {
    assert_eq!(
        &payload[offset..offset + expected_bytes.len()],
        expected_bytes,
        "canonical mutation {id} no longer targets the expected field"
    );
    mutation(
        id,
        "canonical_payload",
        "replace",
        offset,
        expected_bytes.len(),
        value_hex,
        "not_applicable",
    )
}

fn protobuf_replacement(id: &str, replacement: &[u8], expected_result: &str) -> serde_json::Value {
    mutation(
        id,
        "protobuf",
        "replace",
        0,
        POLICY_IR.len(),
        &hex(replacement),
        expected_result,
    )
}

fn first_parameter_mut(program: &mut proto::PolicyProgram) -> &mut proto::CaveatParameter {
    program
        .caveats
        .first_mut()
        .and_then(|caveat| caveat.parameters.first_mut())
        .expect("reviewed PolicyIR must contain a caveat parameter")
}

fn increment_u32_hex(value: &str) -> String {
    let value = u32::from_str_radix(value, 16).unwrap();
    format!("{:08x}", value.checked_add(1).unwrap())
}

fn flip_first_byte_hex(value: &[u8]) -> String {
    format!("{:02x}", value[0] ^ 0x01)
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

fn byte_length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap()
}
