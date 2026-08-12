use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, capability_request_commitment,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CreateCapabilityRequest, authorization_context_entry,
};
use razield::admission::{
    AdmissionError, LOCAL_PLONK_ADMISSION_WINDOW_SECONDS, admit_create_capability_request,
};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PRIVATE_CONTEXT_VALUE: &str = "private-merchant-4829";

#[test]
fn admits_exact_signed_commitment_and_derives_lowercase_job_id() {
    let signing_key = test_signing_key();
    let request = signed_request(valid_request(&signing_key), &signing_key);
    let expected_binding = expected_binding(&request);
    let expected_commitment = capability_request_commitment(&expected_binding).unwrap();

    let admitted = admit_create_capability_request(request, TRUSTED_NOW).unwrap();

    assert_eq!(admitted.agent_id(), "reconciliation-agent");
    assert_eq!(admitted.request(), &expected_binding);
    assert_eq!(
        admitted.capability_request_commitment(),
        &expected_commitment
    );
    assert_eq!(admitted.job_id(), lowercase_hex(&expected_commitment));
    assert_eq!(admitted.job_id().len(), 64);
    assert!(
        admitted
            .job_id()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

#[test]
fn every_authorization_bound_mutation_changes_commitment_and_breaks_signature() {
    assert_bound_mutation("task_id", |request| request.task_id.push_str("-changed"));
    assert_bound_mutation("purpose", |request| request.purpose.push_str("-changed"));
    assert_bound_mutation("action", |request| request.action.push_str("_changed"));
    assert_bound_mutation("resource_type", |request| {
        request.resource_type.push_str("_changed");
    });
    assert_bound_mutation("resource_scope", |request| {
        request.resource_scope.push_str(":changed");
    });
    assert_bound_mutation("requested_expires_at", |request| {
        request.requested_expires_at.as_mut().unwrap().seconds += 1;
    });
    assert_bound_mutation("issuance_nonce", |request| request.issuance_nonce[0] ^= 1);
    assert_bound_mutation("snapshot_id", |request| {
        request.snapshot_id.push_str("-changed");
    });
    assert_bound_mutation("context_name", |request| {
        request.request_context[0].name = "department_changed".into();
    });
    assert_bound_mutation("context_string", |request| {
        request.request_context[0].value = Some(authorization_context_entry::Value::StringValue(
            "another-merchant".into(),
        ));
    });
    assert_bound_mutation("context_bool", |request| {
        request.request_context[1].value =
            Some(authorization_context_entry::Value::BoolValue(false));
    });
    assert_bound_mutation("context_int", |request| {
        request.request_context[2].value =
            Some(authorization_context_entry::Value::IntValue(10_001));
    });
    assert_bound_mutation("agent_public_key", |request| {
        request.agent_public_key = another_signing_key().verifying_key().to_bytes().to_vec();
    });
}

#[test]
fn format_version_is_bound_and_rejected_before_signature_verification() {
    let signing_key = test_signing_key();
    let baseline = signed_request(valid_request(&signing_key), &signing_key);
    let baseline_commitment = capability_request_commitment(&expected_binding(&baseline)).unwrap();
    let mut changed = baseline;
    changed.format_version += 1;
    let changed_commitment = capability_request_commitment(&expected_binding(&changed)).unwrap();

    assert_ne!(baseline_commitment, changed_commitment);
    assert_eq!(
        reject(changed, TRUSTED_NOW),
        AdmissionError::UnsupportedFormatVersion
    );
}

#[test]
fn agent_id_is_unsigned_display_metadata() {
    let signing_key = test_signing_key();
    let original = signed_request(valid_request(&signing_key), &signing_key);
    let mut renamed = original.clone();
    renamed.agent_id = "renamed-display-label".into();

    let first = admit_create_capability_request(original, TRUSTED_NOW).unwrap();
    let second = admit_create_capability_request(renamed, TRUSTED_NOW).unwrap();

    assert_eq!(
        first.capability_request_commitment(),
        second.capability_request_commitment()
    );
    assert_eq!(first.job_id(), second.job_id());
    assert_ne!(first.agent_id(), second.agent_id());
}

#[test]
fn rejects_invalid_key_and_signature_forms() {
    let signing_key = test_signing_key();

    let mut short_key = signed_request(valid_request(&signing_key), &signing_key);
    short_key.agent_public_key.pop();
    assert_eq!(
        reject(short_key, TRUSTED_NOW),
        AdmissionError::InvalidPublicKeyLength
    );

    let mut malformed_key = signed_request(valid_request(&signing_key), &signing_key);
    malformed_key.agent_public_key = [
        2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]
    .to_vec();
    assert_eq!(
        reject(malformed_key, TRUSTED_NOW),
        AdmissionError::InvalidPublicKey
    );

    let mut weak_key = signed_request(valid_request(&signing_key), &signing_key);
    weak_key.agent_public_key = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]
    .to_vec();
    assert_eq!(reject(weak_key, TRUSTED_NOW), AdmissionError::WeakPublicKey);

    let mut short_signature = signed_request(valid_request(&signing_key), &signing_key);
    short_signature.request_signature.pop();
    assert_eq!(
        reject(short_signature, TRUSTED_NOW),
        AdmissionError::InvalidSignatureLength
    );

    let mut changed_signature = signed_request(valid_request(&signing_key), &signing_key);
    changed_signature.request_signature[0] ^= 1;
    assert_eq!(
        reject(changed_signature, TRUSTED_NOW),
        AdmissionError::InvalidSignature
    );

    let mut wrong_message = valid_request(&signing_key);
    wrong_message.request_signature = signing_key
        .sign(b"not-the-request-commitment")
        .to_bytes()
        .to_vec();
    assert_eq!(
        reject(wrong_message, TRUSTED_NOW),
        AdmissionError::InvalidSignature
    );
}

#[test]
fn enforces_whole_seconds_and_the_exact_seventy_five_minute_floor() {
    let signing_key = test_signing_key();
    let exact = signed_request(valid_request(&signing_key), &signing_key);
    admit_create_capability_request(exact, TRUSTED_NOW).unwrap();

    let mut too_short = valid_request(&signing_key);
    too_short.requested_expires_at.as_mut().unwrap().seconds -= 1;
    assert_eq!(
        reject(too_short, TRUSTED_NOW),
        AdmissionError::InsufficientValidityWindow
    );

    let mut missing = valid_request(&signing_key);
    missing.requested_expires_at = None;
    assert_eq!(
        reject(missing, TRUSTED_NOW),
        AdmissionError::MissingRequestedExpiry
    );

    let mut fractional = valid_request(&signing_key);
    fractional.requested_expires_at.as_mut().unwrap().nanos = 1;
    assert_eq!(
        reject(fractional, TRUSTED_NOW),
        AdmissionError::InvalidRequestedExpiry
    );

    let mut negative = valid_request(&signing_key);
    negative.requested_expires_at.as_mut().unwrap().seconds = -1;
    assert_eq!(
        reject(negative, TRUSTED_NOW),
        AdmissionError::InvalidRequestedExpiry
    );

    let mut beyond_protobuf_range = valid_request(&signing_key);
    beyond_protobuf_range
        .requested_expires_at
        .as_mut()
        .unwrap()
        .seconds = 253_402_300_800;
    assert_eq!(
        reject(beyond_protobuf_range, TRUSTED_NOW),
        AdmissionError::InvalidRequestedExpiry
    );

    let overflow = signed_request(valid_request(&signing_key), &signing_key);
    assert_eq!(
        reject(overflow, i64::MAX),
        AdmissionError::InvalidRequestedExpiry
    );

    let negative_clock = signed_request(valid_request(&signing_key), &signing_key);
    assert_eq!(
        reject(negative_clock, -1),
        AdmissionError::InvalidRequestedExpiry
    );
}

#[test]
fn rejects_invalid_metadata_nonce_fields_and_context() {
    let signing_key = test_signing_key();

    let mut empty_agent = valid_request(&signing_key);
    empty_agent.agent_id = " \t".into();
    assert_eq!(
        reject(empty_agent, TRUSTED_NOW),
        AdmissionError::InvalidAgentMetadata
    );

    let mut control_agent = valid_request(&signing_key);
    control_agent.agent_id = "agent\nforged-log".into();
    assert_eq!(
        reject(control_agent, TRUSTED_NOW),
        AdmissionError::InvalidAgentMetadata
    );

    let mut long_agent = valid_request(&signing_key);
    long_agent.agent_id = "a".repeat(257);
    assert_eq!(
        reject(long_agent, TRUSTED_NOW),
        AdmissionError::InvalidAgentMetadata
    );

    let mut short_nonce = valid_request(&signing_key);
    short_nonce.issuance_nonce.pop();
    assert_eq!(
        reject(short_nonce, TRUSTED_NOW),
        AdmissionError::InvalidIssuanceNonce
    );

    let mut zero_nonce = valid_request(&signing_key);
    zero_nonce.issuance_nonce = vec![0; 32];
    assert_eq!(
        reject(zero_nonce, TRUSTED_NOW),
        AdmissionError::InvalidRequest
    );

    let mut invalid_action = valid_request(&signing_key);
    invalid_action.action = "Can_Reconcile".into();
    assert_eq!(
        reject(invalid_action, TRUSTED_NOW),
        AdmissionError::InvalidRequest
    );

    let mut missing_value = valid_request(&signing_key);
    missing_value.request_context[0].value = None;
    assert_eq!(
        reject(missing_value, TRUSTED_NOW),
        AdmissionError::InvalidContext
    );

    let mut unsorted = valid_request(&signing_key);
    unsorted.request_context.swap(0, 1);
    assert_eq!(
        reject(unsorted, TRUSTED_NOW),
        AdmissionError::InvalidContext
    );

    let mut duplicate = valid_request(&signing_key);
    duplicate.request_context[1].name = duplicate.request_context[0].name.clone();
    assert_eq!(
        reject(duplicate, TRUSTED_NOW),
        AdmissionError::InvalidContext
    );

    let mut too_many = valid_request(&signing_key);
    too_many.request_context = (0..17)
        .map(|index| AuthorizationContextEntry {
            name: format!("field_{index:02}"),
            value: Some(authorization_context_entry::Value::BoolValue(true)),
        })
        .collect();
    assert_eq!(
        reject(too_many, TRUSTED_NOW),
        AdmissionError::InvalidContext
    );

    let mut long_context = valid_request(&signing_key);
    long_context.request_context[0].value = Some(authorization_context_entry::Value::StringValue(
        "s".repeat(257),
    ));
    assert_eq!(
        reject(long_context, TRUSTED_NOW),
        AdmissionError::InvalidContext
    );
}

#[test]
fn debug_and_error_output_redact_request_material() {
    let signing_key = test_signing_key();
    let request = signed_request(valid_request(&signing_key), &signing_key);
    let public_key_text = lowercase_hex(&request.agent_public_key);
    let signature_text = lowercase_hex(&request.request_signature);
    let admitted = admit_create_capability_request(request, TRUSTED_NOW).unwrap();

    let admitted_debug = format!("{admitted:?}");
    let request_debug = format!("{:?}", admitted.request());
    for output in [&admitted_debug, &request_debug] {
        assert!(!output.contains(PRIVATE_CONTEXT_VALUE));
        assert!(!output.contains(&public_key_text));
        assert!(!output.contains(&signature_text));
        assert!(!output.contains(admitted.job_id()));
        assert!(output.contains("<redacted>"));
    }

    let mut invalid = valid_request(&signing_key);
    invalid.request_context[0].value = None;
    let error = reject(invalid, TRUSTED_NOW);
    let error_output = format!("{error:?} {error}");
    assert!(!error_output.contains(PRIVATE_CONTEXT_VALUE));
    assert!(!error_output.contains(&public_key_text));
    assert!(!error_output.contains(&signature_text));
}

fn assert_bound_mutation(label: &str, mutate: impl FnOnce(&mut CreateCapabilityRequest)) {
    let signing_key = test_signing_key();
    let baseline = signed_request(valid_request(&signing_key), &signing_key);
    let baseline_commitment = capability_request_commitment(&expected_binding(&baseline)).unwrap();
    let mut changed = baseline;
    mutate(&mut changed);
    let changed_commitment = capability_request_commitment(&expected_binding(&changed)).unwrap();

    assert_ne!(baseline_commitment, changed_commitment, "{label}");
    assert_eq!(
        reject(changed, TRUSTED_NOW),
        AdmissionError::InvalidSignature,
        "{label}"
    );
}

fn reject(request: CreateCapabilityRequest, trusted_now: i64) -> AdmissionError {
    admit_create_capability_request(request, trusted_now).unwrap_err()
}

fn signed_request(
    mut request: CreateCapabilityRequest,
    signing_key: &SigningKey,
) -> CreateCapabilityRequest {
    let commitment = capability_request_commitment(&expected_binding(&request)).unwrap();
    request.request_signature = signing_key.sign(&commitment).to_bytes().to_vec();
    request
}

fn expected_binding(request: &CreateCapabilityRequest) -> CapabilityRequestBindingV1 {
    let public_key: [u8; 32] = request.agent_public_key.as_slice().try_into().unwrap();
    CapabilityRequestBindingV1 {
        format_version: request.format_version,
        agent_key_thumbprint: derive_ed25519_key_thumbprint(&public_key),
        task_id: request.task_id.clone(),
        purpose: request.purpose.clone(),
        action: request.action.clone(),
        resource_type: request.resource_type.clone(),
        resource_scope: request.resource_scope.clone(),
        requested_expires_at_unix_seconds: request.requested_expires_at.unwrap().seconds,
        issuance_nonce: request.issuance_nonce.as_slice().try_into().unwrap(),
        snapshot_id: request.snapshot_id.clone(),
        request_context: request
            .request_context
            .iter()
            .map(|entry| ContextEntry {
                name: entry.name.clone(),
                value: match entry.value.as_ref().unwrap() {
                    authorization_context_entry::Value::BoolValue(value) => {
                        CaveatValue::Bool(*value)
                    }
                    authorization_context_entry::Value::IntValue(value) => CaveatValue::Int(*value),
                    authorization_context_entry::Value::StringValue(value) => {
                        CaveatValue::String(value.clone())
                    }
                },
            })
            .collect(),
    }
}

fn valid_request(signing_key: &SigningKey) -> CreateCapabilityRequest {
    CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: "reconciliation-agent".into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: "task-2026-08-12".into(),
        purpose: "reconcile merchant settlement records".into(),
        action: "reconcile".into(),
        resource_type: "payment_batch".into(),
        resource_scope: "batch:synthetic-001".into(),
        requested_expires_at: Some(Timestamp {
            seconds: TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
            nanos: 0,
        }),
        issuance_nonce: vec![0xa5; 32],
        snapshot_id: "snapshot-synthetic-001".into(),
        request_signature: Vec::new(),
        request_context: vec![
            AuthorizationContextEntry {
                name: "department".into(),
                value: Some(authorization_context_entry::Value::StringValue(
                    PRIVATE_CONTEXT_VALUE.into(),
                )),
            },
            AuthorizationContextEntry {
                name: "is_reconciler".into(),
                value: Some(authorization_context_entry::Value::BoolValue(true)),
            },
            AuthorizationContextEntry {
                name: "max_amount".into(),
                value: Some(authorization_context_entry::Value::IntValue(10_000)),
            },
        ],
    }
}

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn another_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[9; 32])
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
