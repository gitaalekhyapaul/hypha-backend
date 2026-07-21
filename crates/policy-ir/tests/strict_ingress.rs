#![cfg(feature = "strict-protobuf")]

mod support;

use prost::Message;
use raziel_policy_ir::{PolicyError, PolicyIngressError, decode_policy_program_strict};
use raziel_proto::raziel::v1 as proto;
use sha2::{Digest, Sha256};

use support::ceilings;

const GO_COMPILER_GOLDEN: &[u8] = include_bytes!(
    "../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);
const POLICY_IR_V1_DESCRIPTOR: &[u8] = include_bytes!("../descriptors/raziel-policy-ir-v1.binpb");
const POLICY_IR_V1_DESCRIPTOR_SHA256: [u8; 32] = [
    0x56, 0x16, 0x4f, 0xf5, 0xef, 0xd5, 0x7d, 0x13, 0x14, 0xec, 0xd2, 0x74, 0x26, 0xfe, 0x91, 0x5f,
    0x7c, 0x18, 0x59, 0x6d, 0x7b, 0x3f, 0x0d, 0x3b, 0x65, 0x0c, 0xf1, 0x80, 0x06, 0x6e, 0xbc, 0xc3,
];

#[test]
fn frozen_descriptor_matches_the_approved_checksum() {
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(POLICY_IR_V1_DESCRIPTOR)),
        POLICY_IR_V1_DESCRIPTOR_SHA256
    );
}

#[test]
fn accepts_the_deterministic_go_compiler_golden() {
    let mut trusted_ceilings = ceilings();
    trusted_ceilings.policy_limits.max_string_bytes = 256;

    let program = decode_policy_program_strict(
        GO_COMPILER_GOLDEN,
        byte_length(GO_COMPILER_GOLDEN),
        &trusted_ceilings,
    )
    .unwrap();

    assert_eq!(program.program().compiler_name, "authzed/spicedb");
}

#[test]
fn rejects_unknown_fields_at_top_level_and_nested_positions() {
    let mut top_level = GO_COMPILER_GOLDEN.to_vec();
    top_level.extend_from_slice(&unknown_varint_field());
    assert_eq!(
        decode_policy_program_strict(&top_level, byte_length(&top_level), &ceilings()),
        Err(PolicyIngressError::UnknownField)
    );

    let mut definition = proto::ObjectDefinition {
        name: "user".into(),
        relations: Vec::new(),
    }
    .encode_to_vec();
    definition.extend_from_slice(&unknown_varint_field());
    assert!(definition.len() < 128);

    let mut nested = vec![0x2a, definition.len() as u8];
    nested.extend_from_slice(&definition);
    assert_eq!(
        decode_policy_program_strict(&nested, byte_length(&nested), &ceilings()),
        Err(PolicyIngressError::UnknownField)
    );
}

#[test]
fn rejects_alternate_protobuf_spellings() {
    let mut duplicate_scalar = GO_COMPILER_GOLDEN.to_vec();
    duplicate_scalar.extend_from_slice(&[0x08, 0x01]);

    let cases: &[(&str, &[u8])] = &[
        ("duplicate scalar", &duplicate_scalar),
        ("reordered fields", &[0x12, 0x01, b'x', 0x08, 0x01]),
        ("non-minimal varint", &[0x08, 0x81, 0x00]),
        ("explicit default", &[0x08, 0x00]),
    ];

    for (name, bytes) in cases {
        assert_eq!(
            decode_policy_program_strict(bytes, byte_length(bytes), &ceilings()),
            Err(PolicyIngressError::NonCanonicalProtobuf),
            "{name} must fail"
        );
    }
}

#[test]
fn rejects_oversized_malformed_and_semantically_invalid_input() {
    assert_eq!(
        decode_policy_program_strict(
            GO_COMPILER_GOLDEN,
            byte_length(GO_COMPILER_GOLDEN) - 1,
            &ceilings()
        ),
        Err(PolicyIngressError::InputTooLarge)
    );
    assert_eq!(
        decode_policy_program_strict(&[0x0a], 1, &ceilings()),
        Err(PolicyIngressError::MalformedProtobuf)
    );

    let unsupported = proto::PolicyProgram {
        ir_version: 99,
        source_schema_digest: vec![0; 32],
        limits: Some(proto::PolicyLimits::default()),
        ..proto::PolicyProgram::default()
    }
    .encode_to_vec();
    assert_eq!(
        decode_policy_program_strict(&unsupported, byte_length(&unsupported), &ceilings()),
        Err(PolicyIngressError::InvalidPolicy(
            PolicyError::UnsupportedVersion
        ))
    );
}

fn unknown_varint_field() -> [u8; 3] {
    [0x98, 0x06, 0x01]
}

fn byte_length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap()
}
