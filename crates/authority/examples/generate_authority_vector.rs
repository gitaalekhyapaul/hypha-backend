use std::{fs, path::PathBuf};

use raziel_authority::{AuthoritySigner, PolicyApprovalRole, SnapshotRole};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, POLICY_APPROVAL_SIGNATURE_DOMAIN, PolicyApprovalClaimsV1,
    PolicyApprovalLimits, ValidatedPolicyApprovalClaimsV1, encode_policy_approval_claims,
    policy_approval_signature_message,
};
use raziel_snapshot::{
    RELATIONSHIP_TREE_VERSION, SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
    SNAPSHOT_DESCRIPTOR_SIGNATURE_DOMAIN, SnapshotDescriptorClaimsV1, SnapshotDescriptorLimits,
    ValidatedSnapshotDescriptorClaimsV1, encode_snapshot_descriptor_claims,
    snapshot_descriptor_signature_message,
};
use serde_json::json;

const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

fn main() {
    let policy_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x01)).unwrap();
    let snapshot_signer =
        AuthoritySigner::<SnapshotRole>::from_pkcs8_der(&test_pkcs8(0x02)).unwrap();
    let policy_claims = policy_claims(policy_signer.key_id());
    let snapshot_claims = snapshot_claims(snapshot_signer.key_id());
    let validated_policy =
        ValidatedPolicyApprovalClaimsV1::new(policy_claims.clone(), policy_limits()).unwrap();
    let validated_snapshot =
        ValidatedSnapshotDescriptorClaimsV1::new(snapshot_claims.clone(), snapshot_limits())
            .unwrap();
    let policy_payload = encode_policy_approval_claims(&policy_claims).unwrap();
    let policy_message = policy_approval_signature_message(&policy_claims).unwrap();
    let policy_signature = policy_signer
        .sign_policy_approval(&validated_policy)
        .unwrap();
    let snapshot_payload = encode_snapshot_descriptor_claims(&snapshot_claims).unwrap();
    let snapshot_message = snapshot_descriptor_signature_message(&snapshot_claims).unwrap();
    let snapshot_signature = snapshot_signer
        .sign_snapshot_descriptor(&validated_snapshot)
        .unwrap();

    let vector = json!({
        "vector_format_version": 1,
        "private_test_only": true,
        "warning": "Synthetic deterministic authority keys. Never use these keys in production.",
        "signature_algorithm": "Ed25519",
        "hash_algorithm": "sha256",
        "u64_cases": [
            { "value": "0", "canonical_hex": "0000000000000000" },
            { "value": "1", "canonical_hex": "0000000000000001" },
            { "value": (1_u64 << 63).to_string(), "canonical_hex": "8000000000000000" },
            { "value": u64::MAX.to_string(), "canonical_hex": "ffffffffffffffff" }
        ],
        "policy_approval": {
            "domain": String::from_utf8(POLICY_APPROVAL_SIGNATURE_DOMAIN.to_vec()).unwrap(),
            "claims": {
                "format_version": policy_claims.format_version,
                "trust_domain_id": policy_claims.trust_domain_id,
                "record_id": policy_claims.record_id,
                "policy_name": policy_claims.policy_name,
                "policy_version": policy_claims.policy_version,
                "approval_epoch": policy_claims.approval_epoch.to_string(),
                "policy_hash_hex": hex(&policy_claims.policy_hash),
                "valid_from_unix_seconds": policy_claims.valid_from_unix_seconds.to_string(),
                "valid_until_unix_seconds": policy_claims.valid_until_unix_seconds.to_string(),
                "approved_by": policy_claims.approved_by,
                "signing_key_id": policy_claims.signing_key_id
            },
            "public_key_hex": hex(&policy_signer.public_key_bytes()),
            "key_id": policy_signer.key_id(),
            "canonical_payload_hex": hex(&policy_payload),
            "signature_message_hex": hex(&policy_message),
            "signature_hex": hex(&policy_signature),
            "expected_rust_result": "match",
            "expected_typescript_parser_result": "match_structure"
        },
        "snapshot_descriptor": {
            "domain": String::from_utf8(SNAPSHOT_DESCRIPTOR_SIGNATURE_DOMAIN.to_vec()).unwrap(),
            "claims": {
                "format_version": snapshot_claims.format_version,
                "trust_domain_id": snapshot_claims.trust_domain_id,
                "snapshot_id": snapshot_claims.snapshot_id,
                "relationship_snapshot_root_hex": hex(&snapshot_claims.relationship_snapshot_root),
                "policy_record_id": snapshot_claims.policy_record_id,
                "policy_hash_hex": hex(&snapshot_claims.policy_hash),
                "spicedb_revision": snapshot_claims.spicedb_revision,
                "epoch": snapshot_claims.epoch.to_string(),
                "issued_at_unix_seconds": snapshot_claims.issued_at_unix_seconds.to_string(),
                "expires_at_unix_seconds": snapshot_claims.expires_at_unix_seconds.to_string(),
                "canonicalization_version": snapshot_claims.canonicalization_version,
                "signing_key_id": snapshot_claims.signing_key_id
            },
            "public_key_hex": hex(&snapshot_signer.public_key_bytes()),
            "key_id": snapshot_signer.key_id(),
            "canonical_payload_hex": hex(&snapshot_payload),
            "signature_message_hex": hex(&snapshot_message),
            "signature_hex": hex(&snapshot_signature),
            "expected_rust_result": "match",
            "expected_typescript_parser_result": "match_structure"
        },
        "negative_cases": [
            negative("policy.format_version", "claim_rejected"),
            negative("policy.trust_domain_id", "invalid_signature"),
            negative("policy.record_id", "invalid_signature"),
            negative("policy.policy_name", "invalid_signature"),
            negative("policy.policy_version", "invalid_signature"),
            negative("policy.approval_epoch", "invalid_signature"),
            negative("policy.policy_hash", "invalid_signature"),
            negative("policy.valid_from", "invalid_signature"),
            negative("policy.valid_until", "invalid_signature"),
            negative("policy.approved_by", "invalid_signature"),
            negative("policy.signing_key_id", "invalid_key_id"),
            negative("snapshot.format_version", "claim_rejected"),
            negative("snapshot.trust_domain_id", "invalid_signature"),
            negative("snapshot.snapshot_id", "invalid_signature"),
            negative("snapshot.relationship_root", "invalid_signature"),
            negative("snapshot.policy_record_id", "invalid_signature"),
            negative("snapshot.policy_hash", "invalid_signature"),
            negative("snapshot.spicedb_revision", "invalid_signature"),
            negative("snapshot.epoch", "invalid_signature"),
            negative("snapshot.issued_at", "invalid_signature"),
            negative("snapshot.expires_at", "invalid_signature"),
            negative("snapshot.canonicalization_version", "claim_rejected"),
            negative("snapshot.signing_key_id", "invalid_key_id")
        ]
    });

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = workspace.join("tests/vectors/authority-signatures-v1.json");
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(
        output,
        format!("{}\n", serde_json::to_string_pretty(&vector).unwrap()),
    )
    .unwrap();
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

fn negative(target: &str, expected_rust_result: &str) -> serde_json::Value {
    json!({
        "target": target,
        "expected_rust_result": expected_rust_result,
        "expected_typescript_parser_result": "match_structure"
    })
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
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
