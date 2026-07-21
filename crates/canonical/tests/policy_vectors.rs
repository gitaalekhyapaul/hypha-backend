use raziel_canonical::{
    CANONICAL_POLICY_ENCODING_VERSION, POLICY_HASH_DOMAIN, encode_policy, policy_hash,
    policy_hash_preimage,
};
use raziel_policy_ir::{
    CollectionKind, PolicyError, PolicyIngressError, PolicyLimits, ValidationCeilings,
    decode_policy_program_strict,
};
use serde_json::Value;

const POLICY_IR: &[u8] = include_bytes!(
    "../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);
const VECTOR_JSON: &str = include_str!("../../../tests/vectors/policy-ir-v1.json");

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

#[test]
fn checked_in_policy_vector_matches_rust_encoding_and_hashing() {
    let vector: Value = serde_json::from_str(VECTOR_JSON).unwrap();
    assert_eq!(vector["vector_format_version"], 1);
    let case = &vector["policy_cases"][0];
    assert_eq!(case["id"], "supported-policy-ir-v1");
    assert_eq!(case["hash_algorithm"], "sha256");
    assert_eq!(case["domain_separator"], "raziel/policy/v1");
    assert_eq!(
        case["canonical_encoding_version"],
        CANONICAL_POLICY_ENCODING_VERSION
    );
    assert_eq!(POLICY_HASH_DOMAIN, b"raziel/policy/v1");

    let program =
        decode_policy_program_strict(POLICY_IR, byte_length(POLICY_IR), &TRUSTED_CEILINGS).unwrap();
    let payload = encode_policy(&program).unwrap();
    let preimage = policy_hash_preimage(&program).unwrap();
    let digest = policy_hash(&program).unwrap();

    assert_eq!(case["canonical_payload_byte_length"], payload.len());
    assert_eq!(case["canonical_payload_hex"], hex(&payload));
    assert_eq!(case["hash_preimage_byte_length"], preimage.len());
    assert_eq!(case["hash_preimage_hex"], hex(&preimage));
    assert_eq!(case["expected_digest_hex"], hex(&digest));
    assert_eq!(case["expected_rust_result"], "match");
}

#[test]
fn protobuf_negative_vectors_fail_with_the_recorded_error() {
    let vector: Value = serde_json::from_str(VECTOR_JSON).unwrap();
    for case in vector["negative_cases"].as_array().unwrap() {
        if case["target"] != "protobuf" {
            continue;
        }
        let mutated = apply_mutation(POLICY_IR, &case["mutation"]);
        let error =
            decode_policy_program_strict(&mutated, byte_length(&mutated), &TRUSTED_CEILINGS)
                .unwrap_err();
        let expected = match case["expected_rust_result"].as_str().unwrap() {
            "unknown_field" => PolicyIngressError::UnknownField,
            "noncanonical_protobuf" => PolicyIngressError::NonCanonicalProtobuf,
            "unsupported_policy_ir_version" => {
                PolicyIngressError::InvalidPolicy(PolicyError::UnsupportedVersion)
            }
            "not_strictly_sorted_definitions" => PolicyIngressError::InvalidPolicy(
                PolicyError::NotStrictlySorted(CollectionKind::Definitions),
            ),
            "unknown_input_source" => {
                PolicyIngressError::InvalidPolicy(PolicyError::UnknownInputSource)
            }
            "unknown_value_type" => {
                PolicyIngressError::InvalidPolicy(PolicyError::UnknownValueType)
            }
            value => panic!("unsupported expected ingress result {value}"),
        };
        assert_eq!(error, expected, "negative case {}", case["id"]);
    }
}

fn apply_mutation(original: &[u8], mutation: &Value) -> Vec<u8> {
    let offset = usize::try_from(mutation["offset"].as_u64().unwrap()).unwrap();
    let remove_bytes = usize::try_from(mutation["remove_bytes"].as_u64().unwrap()).unwrap();
    let replacement = decode_hex(mutation["value_hex"].as_str().unwrap());
    let mut output = original.to_vec();

    match mutation["operation"].as_str().unwrap() {
        "append" => {
            assert_eq!(offset, output.len());
            output.extend_from_slice(&replacement);
        }
        "replace" => {
            output.splice(offset..offset + remove_bytes, replacement);
        }
        "truncate" => output.truncate(offset),
        value => panic!("unsupported mutation operation {value}"),
    }
    output
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(pair, 16).unwrap()
        })
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

fn byte_length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap()
}
