use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationPublicValuesV1, CapabilityScopeBindingV1,
    capability_scope_hash_from_binding, decode_public_values, encode_public_values,
    issuance_nullifier,
};
use raziel_canonical::{
    PROTECTED_REQUEST_FORMAT_VERSION, ProtectedRequestSignatureInputV1,
    protected_request_signature_message,
};
use raziel_proto::raziel::v1::{
    ProofCapability, ProofSystem, PublicErrorCode, RelationshipSnapshotDescriptor,
    VerifyCapabilityRequest,
};
use raziel_prover::{MAX_PROOF_ARTIFACT_BYTES, TrustedProgramKey};
use raziel_verifier::{LocalRegistry, RegistryError, default_registry_config};

use super::{
    LocalVerificationTrust, MAX_REQUEST_AGE_SECONDS, MAX_REQUEST_FUTURE_SKEW_SECONDS,
    MAX_VERIFICATION_SERVICE_MESSAGE_BYTES, TrustedCapabilityState, VerificationAdmissionError,
    VerificationTrust, admit_verify_capability_request, admit_with_trust,
};

const NOW: i64 = 1_800_000_000;
const ISSUED_AT: i64 = NOW - 1_000;
const EXPIRES_AT: i64 = NOW + 1_000;
const POLICY_HASH: [u8; 32] = [0x11; 32];
const RELATIONSHIP_ROOT: [u8; 32] = [0x22; 32];
const COMMITMENT: [u8; 32] = [0x33; 32];
const BODY_HASH: [u8; 32] = [0x44; 32];
const NONCE: [u8; 32] = [0x55; 32];
const KEY_ID: &str = "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

type RequestMutation = fn(&mut VerifyCapabilityRequest);

#[test]
fn admits_public_request_without_decoding_the_proof() {
    let mut fixture = Fixture::new();
    let admitted = admit_with_trust(fixture.request.clone(), NOW, &mut fixture.trust).unwrap();
    let capability = fixture.request.proof_capability.as_ref().unwrap();
    let claims = admitted.candidate_claims();

    assert_eq!(claims.capability_id, capability.capability_id);
    assert_eq!(claims.policy_hash, POLICY_HASH);
    assert_eq!(claims.relationship_snapshot_root, RELATIONSHIP_ROOT);
    assert_eq!(admitted.policy_record_id(), "policy_1");
    assert_eq!(admitted.snapshot_id(), "snapshot_1");
    assert_eq!(
        admitted.worker_request().proof_artifact(),
        b"not-decoded-by-admission"
    );
    assert_eq!(
        admitted
            .worker_request()
            .trusted_program_key()
            .verification_key_id(),
        KEY_ID
    );

    let public_values = decode_public_values(admitted.worker_request().expected_public_values())
        .expect("admission emits canonical ADR-0011 public values");
    assert_eq!(public_values, expected_public_values(capability));
    assert_ne!(admitted.proof_digest(), &[0; 32]);
    assert!(!format!("{admitted:?}").contains("not-decoded"));
    assert!(!format!("{:?}", admitted.worker_request()).contains("not-decoded"));
}

#[test]
fn request_signature_binds_every_adr_0002_field() {
    let fixture = Fixture::new();

    assert_request_mutation_rejected(&fixture, |request| request.request_method = "GET".into());
    assert_request_mutation_rejected(&fixture, |request| {
        request.canonical_request_target.push_str("&page=2");
    });
    assert_request_mutation_rejected(&fixture, |request| request.request_body_hash[0] ^= 1);
    assert_request_mutation_rejected(&fixture, |request| request.request_nonce[0] ^= 1);
    assert_request_mutation_rejected(&fixture, |request| {
        request.request_timestamp.as_mut().unwrap().seconds += 1;
    });
    assert_request_mutation_rejected(&fixture, |request| {
        request.agent_request_signature[0] ^= 1;
    });
    assert_request_mutation_rejected(&fixture, |request| request.agent_public_key[0] ^= 1);
}

#[test]
fn freshness_boundaries_match_adr_0002() {
    for timestamp in [
        NOW - MAX_REQUEST_AGE_SECONDS,
        NOW + MAX_REQUEST_FUTURE_SKEW_SECONDS,
    ] {
        let mut fixture = Fixture::new();
        fixture.set_request_timestamp_and_resign(timestamp);
        assert!(admit_with_trust(fixture.request, NOW, &mut fixture.trust).is_ok());
    }

    let mut old = Fixture::new();
    old.set_request_timestamp_and_resign(NOW - MAX_REQUEST_AGE_SECONDS - 1);
    assert_eq!(
        admit_with_trust(old.request, NOW, &mut old.trust),
        Err(VerificationAdmissionError::RequestTooOld)
    );

    let mut future = Fixture::new();
    future.set_request_timestamp_and_resign(NOW + MAX_REQUEST_FUTURE_SKEW_SECONDS + 1);
    assert_eq!(
        admit_with_trust(future.request, NOW, &mut future.trust),
        Err(VerificationAdmissionError::RequestTooFarInFuture)
    );
}

#[test]
fn noncanonical_method_and_target_fail_before_signature_use() {
    let mut lower_method = Fixture::new();
    lower_method.request.request_method = "post".into();
    assert_eq!(
        admit_with_trust(lower_method.request, NOW, &mut lower_method.trust),
        Err(VerificationAdmissionError::InvalidRequestMethod)
    );

    for target in [
        "/payments/%72econcile",
        "//payments",
        "/payments/",
        "/payments?a=1&%61=2",
    ] {
        let mut fixture = Fixture::new();
        fixture.request.canonical_request_target = target.into();
        assert_eq!(
            admit_with_trust(fixture.request, NOW, &mut fixture.trust),
            Err(VerificationAdmissionError::InvalidRequestTarget)
        );
    }
}

#[test]
fn malformed_capability_and_request_fields_fail_closed() {
    let mut missing_capability = Fixture::new();
    missing_capability.request.proof_capability = None;
    assert_eq!(
        admit_with_trust(
            missing_capability.request,
            NOW,
            &mut missing_capability.trust
        ),
        Err(VerificationAdmissionError::MissingCapability)
    );

    let cases: Vec<(RequestMutation, VerificationAdmissionError)> = vec![
        (
            |request| request.proof_capability.as_mut().unwrap().format_version = 2,
            VerificationAdmissionError::UnsupportedCapabilityVersion,
        ),
        (
            |request| request.proof_capability.as_mut().unwrap().proof_system = 99,
            VerificationAdmissionError::UnsupportedProofSystem,
        ),
        (
            |request| request.proof_capability.as_mut().unwrap().capability_id = "AA".repeat(32),
            VerificationAdmissionError::InvalidCapabilityId,
        ),
        (
            |request| request.proof_capability.as_mut().unwrap().zk_proof.clear(),
            VerificationAdmissionError::EmptyProofArtifact,
        ),
        (
            |request| {
                request
                    .proof_capability
                    .as_mut()
                    .unwrap()
                    .issuance_nullifier[0] ^= 1
            },
            VerificationAdmissionError::InvalidCapabilityClaim,
        ),
        (
            |request| {
                request
                    .proof_capability
                    .as_mut()
                    .unwrap()
                    .verification_key_id
                    .push('0')
            },
            VerificationAdmissionError::InvalidVerificationKey,
        ),
        (
            |request| {
                request
                    .proof_capability
                    .as_mut()
                    .unwrap()
                    .snapshot_attestation = None
            },
            VerificationAdmissionError::MissingSnapshotAttestation,
        ),
        (
            |request| request.agent_public_key.pop().map(|_| ()).unwrap(),
            VerificationAdmissionError::InvalidAgentPublicKeyLength,
        ),
        (
            |request| request.request_body_hash.pop().map(|_| ()).unwrap(),
            VerificationAdmissionError::InvalidBodyHash,
        ),
        (
            |request| request.request_nonce.pop().map(|_| ()).unwrap(),
            VerificationAdmissionError::InvalidRequestNonce,
        ),
        (
            |request| request.request_timestamp = None,
            VerificationAdmissionError::MissingRequestTimestamp,
        ),
        (
            |request| request.agent_request_signature.pop().map(|_| ()).unwrap(),
            VerificationAdmissionError::InvalidRequestSignatureLength,
        ),
    ];

    for (mutate, expected) in cases {
        let mut fixture = Fixture::new();
        mutate(&mut fixture.request);
        assert_eq!(
            admit_with_trust(fixture.request, NOW, &mut fixture.trust),
            Err(expected)
        );
    }
}

#[test]
fn service_and_artifact_byte_limits_are_distinct() {
    let mut oversized_service = Fixture::new();
    oversized_service
        .request
        .proof_capability
        .as_mut()
        .unwrap()
        .zk_proof = vec![0; MAX_VERIFICATION_SERVICE_MESSAGE_BYTES];
    assert_eq!(
        admit_with_trust(oversized_service.request, NOW, &mut oversized_service.trust),
        Err(VerificationAdmissionError::RequestTooLarge)
    );

    let mut oversized_artifact = Fixture::new();
    oversized_artifact
        .request
        .proof_capability
        .as_mut()
        .unwrap()
        .zk_proof = vec![0; MAX_PROOF_ARTIFACT_BYTES + 1];
    assert_eq!(
        admit_with_trust(
            oversized_artifact.request,
            NOW,
            &mut oversized_artifact.trust
        ),
        Err(VerificationAdmissionError::ProofArtifactTooLarge)
    );
}

#[test]
fn trusted_state_and_capability_time_are_checked_independently() {
    let mut wrong_policy = Fixture::new();
    wrong_policy
        .request
        .proof_capability
        .as_mut()
        .unwrap()
        .policy_hash[0] ^= 1;
    assert_eq!(
        admit_with_trust(wrong_policy.request, NOW, &mut wrong_policy.trust),
        Err(VerificationAdmissionError::SnapshotAttestationMismatch)
    );

    let mut wrong_attestation = Fixture::new();
    wrong_attestation
        .request
        .proof_capability
        .as_mut()
        .unwrap()
        .snapshot_attestation
        .as_mut()
        .unwrap()
        .epoch += 1;
    assert_eq!(
        admit_with_trust(wrong_attestation.request, NOW, &mut wrong_attestation.trust),
        Err(VerificationAdmissionError::SnapshotAttestationMismatch)
    );

    let mut expired = Fixture::new();
    expired
        .request
        .proof_capability
        .as_mut()
        .unwrap()
        .expires_at = Some(timestamp(NOW));
    expired.resign();
    assert_eq!(
        admit_with_trust(expired.request, NOW, &mut expired.trust),
        Err(VerificationAdmissionError::CapabilityInactive)
    );
}

#[test]
fn production_trust_adapter_fails_closed_without_an_installed_registry() {
    let fixture = Fixture::new();
    let registry = LocalRegistry::new(default_registry_config("raziel-test").unwrap());
    let program_key = TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap();
    let mut trust = LocalVerificationTrust::new(registry, program_key);

    assert_eq!(
        admit_verify_capability_request(fixture.request, NOW, &mut trust),
        Err(VerificationAdmissionError::Registry(
            RegistryError::UnknownRoot
        ))
    );
}

#[test]
fn registry_time_floor_prevents_request_revival_after_clock_rollback() {
    let mut fixture = Fixture::new();
    fixture.trust.observed_now_override = Some(NOW + MAX_REQUEST_AGE_SECONDS + 1);

    assert_eq!(
        admit_with_trust(fixture.request, NOW, &mut fixture.trust),
        Err(VerificationAdmissionError::RequestTooOld)
    );
}

#[test]
fn private_errors_map_to_coarse_public_codes() {
    assert_eq!(
        VerificationAdmissionError::InvalidRequestSignature.public_error_code(),
        PublicErrorCode::InvalidRequest
    );
    assert_eq!(
        VerificationAdmissionError::CapabilityInactive.public_error_code(),
        PublicErrorCode::CapabilityExpired
    );
    assert_eq!(
        VerificationAdmissionError::SnapshotAttestationMismatch.public_error_code(),
        PublicErrorCode::CapabilityInvalid
    );
    assert_eq!(
        VerificationAdmissionError::InvalidRequestSignature.to_string(),
        "capability verification admission failed"
    );
}

fn assert_request_mutation_rejected(
    fixture: &Fixture,
    mutate: impl FnOnce(&mut VerifyCapabilityRequest),
) {
    let mut request = fixture.request.clone();
    mutate(&mut request);
    let mut trust = fixture.trust.clone();
    assert!(admit_with_trust(request, NOW, &mut trust).is_err());
}

struct Fixture {
    signing_key: SigningKey,
    request: VerifyCapabilityRequest,
    trust: FakeTrust,
}

impl Fixture {
    fn new() -> Self {
        let signing_key = SigningKey::from_bytes(&[0x07; 32]);
        let public_key = signing_key.verifying_key().to_bytes();
        let agent_key_thumbprint = derive_ed25519_key_thumbprint(&public_key);
        let descriptor = descriptor();
        let capability = ProofCapability {
            format_version: 1,
            capability_id: lowercase_hex(&COMMITMENT),
            agent_key_thumbprint: agent_key_thumbprint.to_vec(),
            task_id: "task_1".into(),
            purpose: "reconciliation".into(),
            action: "query".into(),
            resource_type: "dataset".into(),
            resource_scope: "payments".into(),
            policy_hash: POLICY_HASH.to_vec(),
            relationship_snapshot_root: RELATIONSHIP_ROOT.to_vec(),
            snapshot_id: "snapshot_1".into(),
            capability_request_commitment: COMMITMENT.to_vec(),
            issuance_nullifier: issuance_nullifier(&COMMITMENT).unwrap().to_vec(),
            issued_at: Some(timestamp(ISSUED_AT)),
            expires_at: Some(timestamp(EXPIRES_AT)),
            proof_system: ProofSystem::Sp1 as i32,
            verification_key_id: KEY_ID.into(),
            zk_proof: b"not-decoded-by-admission".to_vec(),
            snapshot_attestation: Some(descriptor.clone()),
        };
        let request = VerifyCapabilityRequest {
            proof_capability: Some(capability),
            agent_public_key: public_key.to_vec(),
            request_method: "POST".into(),
            canonical_request_target: "/payments/reconcile?account=a-1&status=pending".into(),
            request_body_hash: BODY_HASH.to_vec(),
            request_nonce: NONCE.to_vec(),
            request_timestamp: Some(timestamp(NOW)),
            agent_request_signature: Vec::new(),
        };
        let trust = FakeTrust {
            state: TrustedCapabilityState {
                observed_now_unix_seconds: NOW,
                policy_record_id: "policy_1".into(),
                policy_valid_from_unix_seconds: ISSUED_AT - 100,
                policy_valid_until_unix_seconds: EXPIRES_AT + 100,
                snapshot_issued_at_unix_seconds: ISSUED_AT - 100,
                snapshot_expires_at_unix_seconds: EXPIRES_AT + 100,
                policy_hash: POLICY_HASH,
                relationship_snapshot_root: RELATIONSHIP_ROOT,
                descriptor,
            },
            program_key: TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap(),
            observed_now_override: None,
        };
        let mut fixture = Self {
            signing_key,
            request,
            trust,
        };
        fixture.resign();
        fixture
    }

    fn set_request_timestamp_and_resign(&mut self, timestamp: i64) {
        self.request.request_timestamp = Some(super::timestamp(timestamp));
        self.resign();
    }

    fn resign(&mut self) {
        let capability_id = self
            .request
            .proof_capability
            .as_ref()
            .unwrap()
            .capability_id
            .clone();
        let request_timestamp = self.request.request_timestamp.as_ref().unwrap().seconds;
        let body_hash: [u8; 32] = self
            .request
            .request_body_hash
            .as_slice()
            .try_into()
            .unwrap();
        let nonce: [u8; 32] = self.request.request_nonce.as_slice().try_into().unwrap();
        let message = protected_request_signature_message(&ProtectedRequestSignatureInputV1 {
            format_version: PROTECTED_REQUEST_FORMAT_VERSION,
            capability_id: &capability_id,
            request_method: &self.request.request_method,
            canonical_request_target: &self.request.canonical_request_target,
            request_body_hash: &body_hash,
            request_nonce: &nonce,
            request_timestamp_unix_seconds: request_timestamp,
        })
        .unwrap();
        self.request.agent_request_signature = self.signing_key.sign(&message).to_bytes().to_vec();
    }
}

#[derive(Clone)]
struct FakeTrust {
    state: TrustedCapabilityState,
    program_key: TrustedProgramKey,
    observed_now_override: Option<i64>,
}

impl VerificationTrust for FakeTrust {
    fn resolve_snapshot(
        &mut self,
        snapshot_id: &str,
        trusted_now_unix_seconds: i64,
    ) -> Result<TrustedCapabilityState, VerificationAdmissionError> {
        if snapshot_id != self.state.descriptor.snapshot_id {
            return Err(VerificationAdmissionError::SnapshotAttestationMismatch);
        }
        let mut state = self.state.clone();
        state.observed_now_unix_seconds = self
            .observed_now_override
            .unwrap_or(trusted_now_unix_seconds);
        Ok(state)
    }

    fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.program_key
    }
}

fn expected_public_values(capability: &ProofCapability) -> AuthorizationPublicValuesV1 {
    let agent_key_thumbprint = capability
        .agent_key_thumbprint
        .as_slice()
        .try_into()
        .unwrap();
    let issued_at_unix_seconds = capability.issued_at.as_ref().unwrap().seconds;
    let expires_at_unix_seconds = capability.expires_at.as_ref().unwrap().seconds;
    let scope_hash = capability_scope_hash_from_binding(&CapabilityScopeBindingV1 {
        format_version: capability.format_version,
        agent_key_thumbprint,
        task_id: capability.task_id.clone(),
        purpose: capability.purpose.clone(),
        action: capability.action.clone(),
        resource_type: capability.resource_type.clone(),
        resource_scope: capability.resource_scope.clone(),
        snapshot_id: capability.snapshot_id.clone(),
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    })
    .unwrap();
    let values = AuthorizationPublicValuesV1 {
        format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
        policy_hash: POLICY_HASH,
        relationship_snapshot_root: RELATIONSHIP_ROOT,
        capability_request_commitment: COMMITMENT,
        capability_scope_hash: scope_hash,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
        issuance_nullifier: issuance_nullifier(&COMMITMENT).unwrap(),
    };
    assert_eq!(
        decode_public_values(&encode_public_values(&values).unwrap()).unwrap(),
        values
    );
    values
}

fn descriptor() -> RelationshipSnapshotDescriptor {
    RelationshipSnapshotDescriptor {
        format_version: 1,
        snapshot_id: "snapshot_1".into(),
        relationship_snapshot_root: RELATIONSHIP_ROOT.to_vec(),
        policy_hash: POLICY_HASH.to_vec(),
        spicedb_revision: "zed_1".into(),
        epoch: 1,
        issued_at: Some(timestamp(ISSUED_AT - 100)),
        expires_at: Some(timestamp(EXPIRES_AT + 100)),
        canonicalization_version: 1,
        signing_key_id: "ed25519:test".into(),
        authority_signature: vec![0x66; 64],
        trust_domain_id: "raziel-test".into(),
        policy_record_id: "policy_1".into(),
    }
}

const fn timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
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
