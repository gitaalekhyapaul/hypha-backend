use raziel_authorization_core::{
    CapabilityRequestBindingV1, capability_request_commitment, capability_scope_hash,
    issuance_nullifier,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};

const ISSUED_AT: i64 = 1_000;
const EXPIRES_AT: i64 = 1_800;

#[test]
fn request_commitment_binds_every_request_field() {
    let request = request();

    assert_commitment_changes(&request, |value| value.format_version += 1);
    assert_commitment_changes(&request, |value| value.agent_key_thumbprint[0] ^= 1);
    assert_commitment_changes(&request, |value| value.task_id.push('2'));
    assert_commitment_changes(&request, |value| value.purpose.push('2'));
    assert_commitment_changes(&request, |value| value.action.push('2'));
    assert_commitment_changes(&request, |value| value.resource_type.push('2'));
    assert_commitment_changes(&request, |value| value.resource_scope.push('2'));
    assert_commitment_changes(&request, |value| {
        value.requested_expires_at_unix_seconds += 1;
    });
    assert_commitment_changes(&request, |value| value.issuance_nonce[0] ^= 1);
    assert_commitment_changes(&request, |value| value.snapshot_id.push('2'));
    assert_commitment_changes(&request, |value| {
        value.request_context[0].name.push('2');
    });
    assert_commitment_changes(&request, |value| {
        value.request_context[0].value = CaveatValue::Int(81);
    });
    assert_commitment_changes(&request, |value| {
        value.request_context[0].value = CaveatValue::String("80".into());
    });
}

#[test]
fn scope_hash_binds_only_the_public_issued_scope() {
    let request = request();
    let expected = capability_scope_hash(&request, ISSUED_AT, EXPIRES_AT).unwrap();

    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.format_version += 1;
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.agent_key_thumbprint[0] ^= 1;
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.task_id.push('2');
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.purpose.push('2');
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.action.push('2');
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.resource_type.push('2');
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.resource_scope.push('2');
    });
    assert_scope_changes(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.snapshot_id.push('2');
    });
    assert_ne!(
        expected,
        capability_scope_hash(&request, ISSUED_AT + 1, EXPIRES_AT).unwrap()
    );
    assert_ne!(
        expected,
        capability_scope_hash(&request, ISSUED_AT, EXPIRES_AT + 1).unwrap()
    );

    assert_scope_unchanged(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.requested_expires_at_unix_seconds += 1;
    });
    assert_scope_unchanged(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.issuance_nonce[0] ^= 1;
    });
    assert_scope_unchanged(&request, ISSUED_AT, EXPIRES_AT, |value| {
        value.request_context[0].value = CaveatValue::Int(81);
    });
}

#[test]
fn nullifier_changes_with_the_request_commitment() {
    let request = request();
    let original = capability_request_commitment(&request).unwrap();
    let original_nullifier = issuance_nullifier(&original).unwrap();

    let mut changed = request;
    changed.issuance_nonce[0] ^= 1;
    let changed = capability_request_commitment(&changed).unwrap();

    assert_ne!(original, changed);
    assert_ne!(original_nullifier, issuance_nullifier(&changed).unwrap());
}

fn assert_commitment_changes(
    request: &CapabilityRequestBindingV1,
    mutate: impl FnOnce(&mut CapabilityRequestBindingV1),
) {
    let expected = capability_request_commitment(request).unwrap();
    let mut changed = request.clone();
    mutate(&mut changed);
    assert_ne!(expected, capability_request_commitment(&changed).unwrap());
}

fn assert_scope_changes(
    request: &CapabilityRequestBindingV1,
    issued_at: i64,
    expires_at: i64,
    mutate: impl FnOnce(&mut CapabilityRequestBindingV1),
) {
    let expected = capability_scope_hash(request, issued_at, expires_at).unwrap();
    let mut changed = request.clone();
    mutate(&mut changed);
    assert_ne!(
        expected,
        capability_scope_hash(&changed, issued_at, expires_at).unwrap()
    );
}

fn assert_scope_unchanged(
    request: &CapabilityRequestBindingV1,
    issued_at: i64,
    expires_at: i64,
    mutate: impl FnOnce(&mut CapabilityRequestBindingV1),
) {
    let expected = capability_scope_hash(request, issued_at, expires_at).unwrap();
    let mut changed = request.clone();
    mutate(&mut changed);
    assert_eq!(
        expected,
        capability_scope_hash(&changed, issued_at, expires_at).unwrap()
    );
}

fn request() -> CapabilityRequestBindingV1 {
    CapabilityRequestBindingV1 {
        format_version: 1,
        agent_key_thumbprint: [0x11; 32],
        task_id: "task_1".into(),
        purpose: "reconciliation".into(),
        action: "query".into(),
        resource_type: "dataset".into(),
        resource_scope: "payments".into(),
        requested_expires_at_unix_seconds: 1_900,
        issuance_nonce: [0x22; 32],
        snapshot_id: "snapshot_1".into(),
        request_context: vec![ContextEntry {
            name: "requested_amount".into(),
            value: CaveatValue::Int(80),
        }],
    }
}
