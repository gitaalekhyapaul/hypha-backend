use super::dispatch_tests::{approve, prepared};
use super::*;
use crate::access_request::issuance_model::StoredAttempt;
use ed25519_dalek::Signer;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{CapabilityRequestBindingV1, capability_request_commitment};
use raziel_proto::raziel::v1::{
    AccessIssuanceSubmission, AccessRequest, CreateCapabilityRequest, SubmitAccessIssuanceRequest,
};

pub(super) fn proposal(owner: &mut AccessCoordinator) -> AccessRequest {
    let reference = approve(owner, REQUEST);
    owner.reserve_preparation(&reference, NOW + 2).unwrap();
    owner
        .publish_proposal(&reference, prepared(3), NOW + 3)
        .unwrap()
}

pub(super) fn submission(public: &AccessRequest, nonce: u8) -> SubmitAccessIssuanceRequest {
    let proposal = public.proposal.as_ref().unwrap();
    let binding = proposal.policy_binding.as_ref().unwrap();
    let key = SigningKey::from_bytes(&[7; 32]);
    let model = CapabilityRequestBindingV1 {
        format_version: 1,
        agent_key_thumbprint: derive_ed25519_key_thumbprint(&key.verifying_key().to_bytes()),
        task_id: proposal.task_id.clone(),
        purpose: proposal.purpose.clone(),
        action: proposal.action.clone(),
        resource_type: binding.resource_type.clone(),
        resource_scope: binding.resource_scope.clone(),
        requested_expires_at_unix_seconds: proposal.requested_expires_at_unix_seconds,
        issuance_nonce: [nonce; 32],
        snapshot_id: proposal.snapshot_id.clone(),
        request_context: vec![],
    };
    SubmitAccessIssuanceRequest {
        owner: input().owner,
        request_id: public.request_id.clone(),
        submission: Some(AccessIssuanceSubmission {
            format_version: 1,
            proposal_id: proposal.proposal_id.clone(),
            generation: proposal.generation,
            request: Some(CreateCapabilityRequest {
                format_version: 1,
                agent_id: AGENT.into(),
                agent_public_key: key.verifying_key().to_bytes().to_vec(),
                task_id: model.task_id.clone(),
                purpose: model.purpose.clone(),
                action: model.action.clone(),
                resource_type: model.resource_type.clone(),
                resource_scope: model.resource_scope.clone(),
                requested_expires_at: Some(prost_types::Timestamp {
                    seconds: model.requested_expires_at_unix_seconds,
                    nanos: 0,
                }),
                issuance_nonce: model.issuance_nonce.to_vec(),
                snapshot_id: model.snapshot_id.clone(),
                request_context: vec![],
                request_signature: key
                    .sign(&capability_request_commitment(&model).unwrap())
                    .to_bytes()
                    .to_vec(),
            }),
        }),
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_accepts_one_exact_signed_attempt_and_recovers_after_deadline() {
    let live = Live::new();
    let mut owner = live.owner();
    let public = proposal(&mut owner);
    let command = submission(&public, 4);
    let accepted = owner.submit_issuance(command.clone(), NOW + 4).unwrap();
    assert_eq!(accepted.state, State::Submitting as i32);
    assert_eq!(accepted.generation, public.generation + 1);
    let admitted = crate::admission::admit_create_capability_request(
        command
            .submission
            .as_ref()
            .unwrap()
            .request
            .clone()
            .unwrap(),
        NOW + 4,
    )
    .unwrap();
    assert_eq!(accepted.proof_job_id.as_deref(), Some(admitted.job_id()));
    assert!(accepted.capability.is_empty());
    drop(owner);
    let mut owner = live.owner();
    assert_eq!(
        owner.submit_issuance(command, NOW + 5000).unwrap(),
        accepted
    );
    assert_eq!(
        owner
            .submit_issuance(submission(&public, 5), NOW + 5000)
            .unwrap_err(),
        AccessError::Conflict
    );
    let raw = live.raw();
    assert!(!raw.contains("request_signature"));
    assert!(!raw.contains("request_context"));
    assert!(!raw.contains("issuance_nonce"));
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_rejects_changed_proposal_scope_identity_context_and_signature() {
    let live = Live::new();
    let mut owner = live.owner();
    let public = proposal(&mut owner);
    for mutation in 0..15 {
        let mut command = submission(&public, 4);
        let submitted = command.submission.as_mut().unwrap();
        let request = submitted.request.as_mut().unwrap();
        match mutation {
            0 => submitted.proposal_id = "a".repeat(32),
            1 => submitted.generation += 1,
            2 => request.task_id.push('x'),
            3 => request.purpose.push('x'),
            4 => request.action = "read".into(),
            5 => request.resource_type.push('x'),
            6 => request.resource_scope.push('x'),
            7 => request.snapshot_id.push('x'),
            8 => request.requested_expires_at.as_mut().unwrap().seconds += 1,
            9 => request.agent_id = "2".repeat(32),
            10 => request.agent_public_key[0] ^= 1,
            11 => request.request_signature[0] ^= 1,
            12 => {
                request
                    .request_context
                    .push(raziel_proto::raziel::v1::AuthorizationContextEntry {
                        name: "private_context".into(),
                        value: Some(
                            raziel_proto::raziel::v1::authorization_context_entry::Value::BoolValue(
                                true,
                            ),
                        ),
                    })
            }
            13 => command.owner.as_mut().unwrap().agent_public_key[0] ^= 1,
            _ => request.requested_expires_at.as_mut().unwrap().nanos = 1,
        }
        assert!(
            matches!(
                owner.submit_issuance(command, NOW + 4),
                Err(AccessError::Conflict | AccessError::InvalidRequest)
            ),
            "mutation {mutation}"
        );
        assert_eq!(owner.get(AGENT, REQUEST, NOW + 4).unwrap(), public);
    }
    assert_eq!(
        owner
            .submit_issuance(submission(&public, 4), NOW + 123)
            .unwrap_err(),
        AccessError::Conflict
    );
    assert_eq!(
        owner.get(AGENT, REQUEST, NOW + 123).unwrap().state,
        State::WaitingForAgent as i32
    );
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_private_acceptance_recovers_before_timeout_and_cancel() {
    let live = Live::new();
    let mut owner = live.owner();
    let public = proposal(&mut owner);
    let command = submission(&public, 4);
    let (record, _) = owner
        .redis
        .get(&mut live.connection(), &format!("{AGENT}:{REQUEST}"))
        .unwrap()
        .unwrap();
    let original = owner.private.read(&record.private_ref).unwrap();
    let attempt = StoredAttempt::accept(
        command.submission.as_ref().unwrap(),
        &record,
        &original,
        NOW + 4,
    )
    .unwrap();
    {
        let _lock = owner.private.lock().unwrap();
        owner
            .private
            .persist_attempt(&record.private_ref, &record, &original, &attempt)
            .unwrap();
    }
    let path = live
        .config
        .private_root()
        .join(format!("{}.attempt", record.private_ref));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    drop(owner);
    let mut owner = live.owner();
    let recovered = owner.get(AGENT, REQUEST, NOW + 123).unwrap();
    assert_eq!(recovered.state, State::Submitting as i32);
    assert_eq!(
        recovered.proof_job_id.as_deref(),
        Some(attempt.job_id.as_str())
    );
    let canceled = owner
        .cancel(AGENT, REQUEST, recovered.generation, NOW + 124)
        .unwrap();
    assert_eq!(canceled.state, State::Cancelling as i32);
    assert_eq!(
        owner
            .cancel(AGENT, REQUEST, recovered.generation, NOW + 125)
            .unwrap(),
        canceled
    );
    assert_eq!(owner.submit_issuance(command, NOW + 125).unwrap(), canceled);
    // Missing durable comparison bytes fail closed, never resend a replacement attempt.
    fs::remove_file(path).unwrap();
    assert_eq!(
        owner.get(AGENT, REQUEST, NOW + 126).unwrap_err(),
        AccessError::ServiceUnavailable
    );
}
