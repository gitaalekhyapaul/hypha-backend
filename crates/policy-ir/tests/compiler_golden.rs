mod support;

use prost::Message;
use raziel_policy_ir::ValidatedPolicyProgram;
#[cfg(feature = "strict-protobuf")]
use raziel_policy_ir::decode_policy_program_strict;
use raziel_proto::raziel::v1 as proto;

use support::ceilings;

const GO_COMPILER_GOLDEN: &[u8] = include_bytes!(
    "../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);

#[test]
fn go_compiler_golden_decodes_and_validates() {
    let wire = proto::PolicyProgram::decode(GO_COMPILER_GOLDEN).unwrap();
    let mut trusted_ceilings = ceilings();
    trusted_ceilings.policy_limits.max_string_bytes = 256;

    let validated = ValidatedPolicyProgram::from_trusted_proto(wire, &trusted_ceilings).unwrap();
    let program = validated.program();

    assert_eq!(program.compiler_name, "authzed/spicedb");
    assert_eq!(program.compiler_version, "1.54.0");
    assert_eq!(
        program.source_schema_digest,
        [
            0x65, 0x85, 0xfe, 0x03, 0xc0, 0x01, 0xea, 0x94, 0x8b, 0x40, 0xfa, 0xa3, 0x05, 0x01,
            0x01, 0x91, 0x09, 0x54, 0xdf, 0x43, 0x47, 0xe5, 0xa8, 0x4d, 0x9d, 0x4e, 0xc4, 0x26,
            0x6e, 0x72, 0x01, 0xa2,
        ]
    );
}

#[cfg(feature = "strict-protobuf")]
#[test]
fn go_compiler_golden_passes_strict_ingress() {
    let mut trusted_ceilings = ceilings();
    trusted_ceilings.policy_limits.max_string_bytes = 256;

    decode_policy_program_strict(GO_COMPILER_GOLDEN, golden_length(), &trusted_ceilings).unwrap();
}

#[cfg(feature = "strict-protobuf")]
fn golden_length() -> u64 {
    u64::try_from(GO_COMPILER_GOLDEN.len()).unwrap()
}
