use raziel_authorization_core::{
    AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationPublicValuesV1, CapabilityRequestBindingV1,
    capability_request_commitment, capability_scope_hash, encode_public_values, issuance_nullifier,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use serde::Deserialize;

const VECTOR_JSON: &str =
    include_str!("../../../tests/vectors/authorization-public-values-v1.json");

#[derive(Deserialize)]
struct Vector {
    vector_version: u32,
    capability_request_commitment_hex: String,
    capability_scope_hash_hex: String,
    issuance_nullifier_hex: String,
    authorization_public_values_hex: String,
}

#[test]
fn authorization_commitments_match_the_checked_in_vector() {
    let vector: Vector = serde_json::from_str(VECTOR_JSON).unwrap();
    assert_eq!(vector.vector_version, 1);

    let request = CapabilityRequestBindingV1 {
        format_version: 1,
        agent_key_thumbprint: [0x11; 32],
        task_id: "reconcile-july".into(),
        purpose: "payment-reconciliation".into(),
        action: "view".into(),
        resource_type: "payment_batch".into(),
        resource_scope: "batch-caveat".into(),
        requested_expires_at_unix_seconds: 1_800_000_000,
        issuance_nonce: [0x22; 32],
        snapshot_id: "snapshot-2026-07-24".into(),
        request_context: vec![ContextEntry {
            name: "requested_amount".into(),
            value: CaveatValue::Int(900),
        }],
    };
    let issued_at = 1_799_999_100;
    let expires_at = 1_799_999_700;
    let request_commitment = capability_request_commitment(&request).unwrap();
    let scope_hash = capability_scope_hash(&request, issued_at, expires_at).unwrap();
    let nullifier = issuance_nullifier(&request_commitment).unwrap();
    let public_values = encode_public_values(&AuthorizationPublicValuesV1 {
        format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
        policy_hash: [0x33; 32],
        relationship_snapshot_root: [0x44; 32],
        capability_request_commitment: request_commitment,
        capability_scope_hash: scope_hash,
        issued_at_unix_seconds: issued_at,
        expires_at_unix_seconds: expires_at,
        issuance_nullifier: nullifier,
    })
    .unwrap();

    assert_eq!(
        encode_hex(&request_commitment),
        vector.capability_request_commitment_hex
    );
    assert_eq!(encode_hex(&scope_hash), vector.capability_scope_hash_hex);
    assert_eq!(encode_hex(&nullifier), vector.issuance_nullifier_hex);
    assert_eq!(
        encode_hex(&public_values),
        vector.authorization_public_values_hex
    );
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
