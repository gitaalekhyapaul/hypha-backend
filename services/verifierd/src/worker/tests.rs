use prost::Message;
use raziel_authorization_core::{AuthorizationPublicValuesV1, encode_public_values};
use raziel_prover::TrustedProgramKey;

use super::wire::VerificationWorkerRequestWireV1;
use super::*;

const KEY_ID: &str = "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const CORRELATION_ID: &str = "abababababababababababababababababababababababababababababababab";

#[test]
fn request_and_result_round_trip_strictly() {
    let request = request();
    assert_eq!(
        decode_request(&encode_request(&request).unwrap()).unwrap(),
        request
    );

    let success = VerificationWorkerResult::Success(
        VerificationWorkerSuccess::new(CORRELATION_ID.to_owned(), KEY_ID.to_owned()).unwrap(),
    );
    assert_eq!(
        decode_result(&encode_result(&success).unwrap()).unwrap(),
        success
    );
}

#[test]
fn unknown_fields_and_noncanonical_defaults_are_rejected() {
    let encoded = encode_request(&request()).unwrap();
    let mut unknown = encoded.clone();
    unknown.extend_from_slice(&[0x40, 0x01]);
    assert_eq!(
        decode_request(&unknown),
        Err(VerificationWorkerProtocolError::MalformedRequest)
    );

    let mut wire = VerificationWorkerRequestWireV1::decode(encoded.as_slice()).unwrap();
    wire.deadline_seconds = 0;
    assert_eq!(
        decode_request(&wire.encode_to_vec()),
        Err(VerificationWorkerProtocolError::InvalidRequest)
    );
}

#[test]
fn debug_output_redacts_proof_and_public_values() {
    assert_eq!(
        format!("{:?}", request()),
        "IsolatedVerificationRequest { public_proof_input: \"<redacted>\" }"
    );
}

fn request() -> IsolatedVerificationRequest {
    IsolatedVerificationRequest::new(
        vec![0xaa, 0xbb],
        encode_public_values(&AuthorizationPublicValuesV1 {
            format_version: 1,
            policy_hash: [0x11; 32],
            relationship_snapshot_root: [0x22; 32],
            capability_request_commitment: [0x33; 32],
            capability_scope_hash: [0x44; 32],
            issued_at_unix_seconds: 10,
            expires_at_unix_seconds: 20,
            issuance_nullifier: [0x55; 32],
        })
        .unwrap(),
        TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap(),
        CORRELATION_ID.to_owned(),
    )
    .unwrap()
}
