mod support;

use prost::Message;
use raziel_policy_ir::ValidatedPolicyProgram;
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

    let validated = ValidatedPolicyProgram::from_proto(wire, &trusted_ceilings).unwrap();
    let program = validated.program();

    assert_eq!(program.compiler_name, "authzed/spicedb");
    assert_eq!(program.compiler_version, "1.54.0");
    assert_eq!(
        program.source_schema_digest,
        [
            0x9e, 0x87, 0xdf, 0x40, 0xb2, 0x30, 0xcd, 0xdf, 0x2c, 0xa9, 0x35, 0x36, 0x57, 0x37,
            0x36, 0xde, 0x11, 0xd8, 0xd7, 0xd6, 0x28, 0x16, 0x26, 0xf4, 0x1b, 0xaf, 0x43, 0x5d,
            0x50, 0xce, 0xac, 0x58,
        ]
    );
}
