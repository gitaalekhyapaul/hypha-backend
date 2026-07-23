use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityError, AuthorityKeyRecordV1, AuthorityKeyStatus,
    AuthorityRole, AuthoritySigner, PolicyApprovalRole, SnapshotRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, encode_policy_approval_claims,
    policy_approval_signature_message,
};
use raziel_snapshot::{
    RELATIONSHIP_TREE_VERSION, SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SnapshotDescriptorClaimsV1,
    SnapshotDescriptorLimits, ValidatedSnapshotDescriptorClaimsV1,
    encode_snapshot_descriptor_claims, snapshot_descriptor_signature_message,
};
use serde_json::Value;

const VECTOR: &str = include_str!("../../../tests/vectors/authority-signatures-v1.json");
const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

#[test]
fn checked_in_authority_vector_matches_exact_bytes_and_signatures() {
    let vector: Value = serde_json::from_str(VECTOR).unwrap();
    let policy_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x01)).unwrap();
    let snapshot_signer =
        AuthoritySigner::<SnapshotRole>::from_pkcs8_der(&test_pkcs8(0x02)).unwrap();
    let policy = policy_claims(policy_signer.key_id());
    let snapshot = snapshot_claims(snapshot_signer.key_id());
    let validated_policy =
        ValidatedPolicyApprovalClaimsV1::new(policy.clone(), policy_limits()).unwrap();
    let validated_snapshot =
        ValidatedSnapshotDescriptorClaimsV1::new(snapshot.clone(), snapshot_limits()).unwrap();

    let policy_case = &vector["policy_approval"];
    assert_eq!(
        hex(&policy_signer.public_key_bytes()),
        string(policy_case, "public_key_hex")
    );
    assert_eq!(policy_signer.key_id(), string(policy_case, "key_id"));
    assert_eq!(
        hex(&encode_policy_approval_claims(&policy).unwrap()),
        string(policy_case, "canonical_payload_hex")
    );
    assert_eq!(
        hex(&policy_approval_signature_message(&policy).unwrap()),
        string(policy_case, "signature_message_hex")
    );
    assert_eq!(
        hex(&policy_signer
            .sign_policy_approval(&validated_policy)
            .unwrap()),
        string(policy_case, "signature_hex")
    );

    let snapshot_case = &vector["snapshot_descriptor"];
    assert_eq!(
        hex(&snapshot_signer.public_key_bytes()),
        string(snapshot_case, "public_key_hex")
    );
    assert_eq!(snapshot_signer.key_id(), string(snapshot_case, "key_id"));
    assert_eq!(
        hex(&encode_snapshot_descriptor_claims(&snapshot).unwrap()),
        string(snapshot_case, "canonical_payload_hex")
    );
    assert_eq!(
        hex(&snapshot_descriptor_signature_message(&snapshot).unwrap()),
        string(snapshot_case, "signature_message_hex")
    );
    assert_eq!(
        hex(&snapshot_signer
            .sign_snapshot_descriptor(&validated_snapshot)
            .unwrap()),
        string(snapshot_case, "signature_hex")
    );
}

#[test]
fn every_recorded_negative_case_executes_its_expected_result() {
    let vector: Value = serde_json::from_str(VECTOR).unwrap();
    let policy_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x01)).unwrap();
    let snapshot_signer =
        AuthoritySigner::<SnapshotRole>::from_pkcs8_der(&test_pkcs8(0x02)).unwrap();
    let policy = policy_claims(policy_signer.key_id());
    let snapshot = snapshot_claims(snapshot_signer.key_id());
    let policy_signature = policy_signer
        .sign_policy_approval(
            &ValidatedPolicyApprovalClaimsV1::new(policy.clone(), policy_limits()).unwrap(),
        )
        .unwrap();
    let snapshot_signature = snapshot_signer
        .sign_snapshot_descriptor(
            &ValidatedSnapshotDescriptorClaimsV1::new(snapshot.clone(), snapshot_limits()).unwrap(),
        )
        .unwrap();
    let policy_key = trusted_policy_key(&policy_signer);
    let snapshot_key = trusted_snapshot_key(&snapshot_signer);

    for case in vector["negative_cases"].as_array().unwrap() {
        let target = string(case, "target");
        let expected = string(case, "expected_rust_result");
        let actual = if let Some(field) = target.strip_prefix("policy.") {
            execute_policy_mutation(field, policy.clone(), &policy_signature, &policy_key)
        } else {
            execute_snapshot_mutation(
                target.strip_prefix("snapshot.").unwrap(),
                snapshot.clone(),
                &snapshot_signature,
                &snapshot_key,
            )
        };
        assert_eq!(actual, expected, "{target}");
    }
}

fn execute_policy_mutation(
    field: &str,
    mut claims: PolicyApprovalClaimsV1,
    signature: &[u8; 64],
    key: &TrustedAuthorityKey,
) -> &'static str {
    match field {
        "format_version" => claims.format_version = 2,
        "trust_domain_id" => claims.trust_domain_id.push('x'),
        "record_id" => claims.record_id.push('x'),
        "policy_name" => claims.policy_name.push('x'),
        "policy_version" => claims.policy_version.push('x'),
        "approval_epoch" => claims.approval_epoch += 1,
        "policy_hash" => claims.policy_hash[0] ^= 1,
        "valid_from" => claims.valid_from_unix_seconds += 1,
        "valid_until" => claims.valid_until_unix_seconds -= 1,
        "approved_by" => claims.approved_by.push('x'),
        "signing_key_id" => claims.signing_key_id.replace_range(8..9, "a"),
        _ => panic!("unhandled policy mutation {field}"),
    }
    let Ok(claims) = ValidatedPolicyApprovalClaimsV1::new(claims, policy_limits()) else {
        return "claim_rejected";
    };
    authority_result(key.verify_policy_approval_signature(&claims, signature))
}

fn execute_snapshot_mutation(
    field: &str,
    mut claims: SnapshotDescriptorClaimsV1,
    signature: &[u8; 64],
    key: &TrustedAuthorityKey,
) -> &'static str {
    match field {
        "format_version" => claims.format_version = 2,
        "trust_domain_id" => claims.trust_domain_id.push('x'),
        "snapshot_id" => claims.snapshot_id.push('x'),
        "relationship_root" => claims.relationship_snapshot_root[0] ^= 1,
        "policy_record_id" => claims.policy_record_id.push('x'),
        "policy_hash" => claims.policy_hash[0] ^= 1,
        "spicedb_revision" => claims.spicedb_revision.push('x'),
        "epoch" => claims.epoch += 1,
        "issued_at" => claims.issued_at_unix_seconds += 1,
        "expires_at" => claims.expires_at_unix_seconds -= 1,
        "canonicalization_version" => claims.canonicalization_version = 2,
        "signing_key_id" => claims.signing_key_id.replace_range(8..9, "a"),
        _ => panic!("unhandled snapshot mutation {field}"),
    }
    let Ok(claims) = ValidatedSnapshotDescriptorClaimsV1::new(claims, snapshot_limits()) else {
        return "claim_rejected";
    };
    authority_result(key.verify_snapshot_descriptor_signature(&claims, signature))
}

fn authority_result(result: Result<(), AuthorityError>) -> &'static str {
    match result {
        Err(AuthorityError::InvalidKeyId) => "invalid_key_id",
        Err(AuthorityError::InvalidSignature) => "invalid_signature",
        other => panic!("unexpected authority result {other:?}"),
    }
}

fn policy_claims(key_id: &str) -> PolicyApprovalClaimsV1 {
    PolicyApprovalClaimsV1 {
        format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
        trust_domain_id: "raziel-test".into(),
        record_id: "policy-reconciliation-v1".into(),
        policy_name: "reconciliation".into(),
        policy_version: "v1".into(),
        approval_epoch: 1,
        policy_hash: [0x11; 32],
        valid_from_unix_seconds: 1_700_000_000,
        valid_until_unix_seconds: 1_700_003_600,
        approved_by: "synthetic-maintainer".into(),
        signing_key_id: key_id.into(),
    }
}

fn snapshot_claims(key_id: &str) -> SnapshotDescriptorClaimsV1 {
    SnapshotDescriptorClaimsV1 {
        format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
        trust_domain_id: "raziel-test".into(),
        snapshot_id: "snapshot-reconciliation-1".into(),
        relationship_snapshot_root: [0x22; 32],
        policy_record_id: "policy-reconciliation-v1".into(),
        policy_hash: [0x11; 32],
        spicedb_revision: "synthetic-zed-token-1".into(),
        epoch: 1,
        issued_at_unix_seconds: 1_700_000_100,
        expires_at_unix_seconds: 1_700_001_000,
        canonicalization_version: RELATIONSHIP_TREE_VERSION,
        signing_key_id: key_id.into(),
    }
}

fn trusted_policy_key(signer: &AuthoritySigner<PolicyApprovalRole>) -> TrustedAuthorityKey {
    trusted_key(
        signer.key_id(),
        signer.public_key_bytes(),
        AuthorityRole::PolicyApproval,
    )
}

fn trusted_snapshot_key(signer: &AuthoritySigner<SnapshotRole>) -> TrustedAuthorityKey {
    trusted_key(
        signer.key_id(),
        signer.public_key_bytes(),
        AuthorityRole::Snapshot,
    )
}

fn trusted_key(key_id: &str, public_key: [u8; 32], role: AuthorityRole) -> TrustedAuthorityKey {
    TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            key_id: key_id.into(),
            role,
            public_key,
            status: AuthorityKeyStatus::Active,
            valid_from_unix_seconds: 1_699_999_000,
            valid_until_unix_seconds: 1_700_004_000,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        },
        "raziel-test",
    )
    .unwrap()
}

const fn policy_limits() -> PolicyApprovalLimits {
    PolicyApprovalLimits {
        max_string_bytes: 256,
        max_lifetime_seconds: 3_600,
    }
}

const fn snapshot_limits() -> SnapshotDescriptorLimits {
    SnapshotDescriptorLimits {
        max_string_bytes: 256,
        max_lifetime_seconds: 900,
    }
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
}

fn string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap()
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
