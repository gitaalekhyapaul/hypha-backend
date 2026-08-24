use raziel_canonical::{
    ProtectedRequestSignatureInputV1, canonicalize_request_method, canonicalize_request_target,
    protected_request_signature_message,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    format_version: u32,
    capability_id: String,
    input_method: String,
    canonical_method: String,
    input_target: String,
    canonical_target: String,
    request_body_hash_hex: String,
    request_nonce_hex: String,
    request_timestamp_unix_seconds: i64,
    signature_message_hex: String,
}

#[test]
fn protected_request_bytes_match_the_language_neutral_vector() {
    let vector: Vector = serde_json::from_str(include_str!(
        "../../../tests/vectors/protected-request-signature-v1.json"
    ))
    .unwrap();
    assert_eq!(
        canonicalize_request_method(&vector.input_method).unwrap(),
        vector.canonical_method
    );
    assert_eq!(
        canonicalize_request_target(&vector.input_target).unwrap(),
        vector.canonical_target
    );

    let body_hash = decode_digest(&vector.request_body_hash_hex);
    let nonce = decode_digest(&vector.request_nonce_hex);
    let message = protected_request_signature_message(&ProtectedRequestSignatureInputV1 {
        format_version: vector.format_version,
        capability_id: &vector.capability_id,
        request_method: &vector.canonical_method,
        canonical_request_target: &vector.canonical_target,
        request_body_hash: &body_hash,
        request_nonce: &nonce,
        request_timestamp_unix_seconds: vector.request_timestamp_unix_seconds,
    })
    .unwrap();
    assert_eq!(lowercase_hex(&message), vector.signature_message_hex);
}

fn decode_digest(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut digest = [0_u8; 32];
    for (index, output) in digest.iter_mut().enumerate() {
        *output = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    digest
}

fn lowercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}
