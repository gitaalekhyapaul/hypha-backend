use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ed25519_dalek::{Signer, SigningKey};
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::issuance_nullifier;
use raziel_canonical::{
    PROTECTED_REQUEST_FORMAT_VERSION, ProtectedRequestSignatureInputV1,
    protected_request_signature_message,
};
use raziel_proto::raziel::v1::{
    ProofCapability, ProofSystem, RelationshipSnapshotDescriptor, VerifyCapabilityRequest,
};
use raziel_prover::TrustedProgramKey;

use super::{
    TrustedCapabilityState, VerificationAdmissionError, VerificationTrust, admit_with_trust,
};
use crate::retained_fixture::RetainedPlonkFixture;
use crate::worker::{
    IsolatedVerificationRequest, UnixSocketVerificationExecutor, VERIFIER_LAUNCHER_SOCKET_PATH_ENV,
    VerificationExecutionError, VerificationExecutor,
};

const NOW: i64 = 1_500;
const BODY_HASH: [u8; 32] = [0x81; 32];
const REQUEST_NONCE: [u8; 32] = [0x82; 32];
const RFC_8032_SIGNING_KEY: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const RFC_8032_PUBLIC_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];
const WRONG_SP1_KEY_HASH: &str =
    "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[tokio::test]
#[ignore = "requires the retained B-0031 Plonk fixture and isolated verifier sandbox"]
async fn retained_plonk_fixture_passes_isolated_verifier_and_mutations_fail_closed() {
    let retained = RetainedPlonkFixture::load(&retained_fixture_path()).unwrap();
    let executor = isolated_executor();
    let positive = RequestFixture::new(&retained);
    let started = Instant::now();
    let success = verify(positive, &executor).await.unwrap();
    let elapsed = started.elapsed();
    assert_eq!(
        success.verification_key_id(),
        retained.trusted_program_key().verification_key_id()
    );
    eprintln!(
        "b0031_retained_verify: verify_ms={}, artifact_sha256={}, verification_key_id={}",
        elapsed.as_millis(),
        lowercase_hex(retained.proof_sha256()),
        success.verification_key_id(),
    );

    let mut changed_proof = RequestFixture::new(&retained);
    *changed_proof.capability_mut().zk_proof.last_mut().unwrap() ^= 1;
    expect_worker_rejection(changed_proof, &executor).await;

    let mut changed_journal = RequestFixture::new(&retained);
    let mut commitment = fixed_digest(&changed_journal.capability().capability_request_commitment);
    commitment[0] ^= 1;
    let capability = changed_journal.capability_mut();
    capability.capability_request_commitment = commitment.to_vec();
    capability.capability_id = lowercase_hex(&commitment);
    capability.issuance_nullifier = issuance_nullifier(&commitment).unwrap().to_vec();
    changed_journal.resign();
    expect_worker_rejection(changed_journal, &executor).await;

    let mut changed_key = RequestFixture::new(&retained);
    assert_ne!(
        retained.trusted_program_key().sp1_vkey_hash(),
        WRONG_SP1_KEY_HASH
    );
    changed_key.trust.program_key = TrustedProgramKey::from_registry_record(
        retained.trusted_program_key().verification_key_id(),
        WRONG_SP1_KEY_HASH,
    )
    .unwrap();
    expect_worker_rejection(changed_key, &executor).await;

    let mut changed_policy = RequestFixture::new(&retained);
    changed_policy.capability_mut().policy_hash[0] ^= 1;
    changed_policy.sync_trusted_descriptor();
    expect_worker_rejection(changed_policy, &executor).await;

    let mut changed_root = RequestFixture::new(&retained);
    changed_root.capability_mut().relationship_snapshot_root[0] ^= 1;
    changed_root.sync_trusted_descriptor();
    expect_worker_rejection(changed_root, &executor).await;

    let mut changed_scope = RequestFixture::new(&retained);
    changed_scope.capability_mut().action = "union_access".into();
    expect_worker_rejection(changed_scope, &executor).await;

    let mut changed_expiry = RequestFixture::new(&retained);
    changed_expiry
        .capability_mut()
        .expires_at
        .as_mut()
        .unwrap()
        .seconds -= 1;
    expect_worker_rejection(changed_expiry, &executor).await;

    let mut changed_signature = RequestFixture::new(&retained);
    changed_signature.request.agent_request_signature[0] ^= 1;
    assert!(matches!(
        admit_with_trust(changed_signature.request, NOW, &mut changed_signature.trust),
        Err(VerificationAdmissionError::InvalidRequestSignature)
    ));

    eprintln!(
        "b0031_retained_mutations: proof=reject, public_journal=reject, key_identity=reject, policy=reject, root=reject, scope=reject, expiry=reject, request_signature=reject"
    );
}

async fn verify(
    mut fixture: RequestFixture,
    executor: &UnixSocketVerificationExecutor,
) -> Result<crate::worker::VerificationWorkerSuccess, VerificationExecutionError> {
    let admitted = admit_with_trust(fixture.request, NOW, &mut fixture.trust)
        .expect("retained fixture request must pass public admission");
    let request = IsolatedVerificationRequest::from_admission(
        admitted.worker_request(),
        admitted.proof_digest(),
    )
    .expect("admitted request must fit the isolated verifier protocol");
    executor.verify(request).await
}

async fn expect_worker_rejection(
    fixture: RequestFixture,
    executor: &UnixSocketVerificationExecutor,
) {
    assert_eq!(
        verify(fixture, executor).await,
        Err(VerificationExecutionError::Rejected)
    );
}

fn retained_fixture_path() -> PathBuf {
    std::env::var_os("RAZIEL_RETAINED_PLONK_FIXTURE_PATH")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/retained-plonk-v1")
        })
}

fn isolated_executor() -> UnixSocketVerificationExecutor {
    let socket_path = std::env::var_os(VERIFIER_LAUNCHER_SOCKET_PATH_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .expect("isolated verifier launcher socket path must be configured");
    let owner_uid = std::fs::symlink_metadata(socket_path.parent().unwrap())
        .unwrap()
        .uid();
    UnixSocketVerificationExecutor::new(socket_path, owner_uid).unwrap()
}

struct RequestFixture {
    signing_key: SigningKey,
    request: VerifyCapabilityRequest,
    trust: FixtureTrust,
}

impl RequestFixture {
    fn new(retained: &RetainedPlonkFixture) -> Self {
        let signing_key = SigningKey::from_bytes(&RFC_8032_SIGNING_KEY);
        let public_key = signing_key.verifying_key().to_bytes();
        assert_eq!(public_key, RFC_8032_PUBLIC_KEY);
        let public_values = retained.public_values();
        let agent_key_thumbprint = derive_ed25519_key_thumbprint(&public_key);
        let descriptor = RelationshipSnapshotDescriptor {
            format_version: 1,
            snapshot_id: "snapshot_1".into(),
            relationship_snapshot_root: public_values.relationship_snapshot_root.to_vec(),
            policy_hash: public_values.policy_hash.to_vec(),
            spicedb_revision: "zed_b0031".into(),
            epoch: 1,
            issued_at: Some(timestamp(public_values.issued_at_unix_seconds - 100)),
            expires_at: Some(timestamp(public_values.expires_at_unix_seconds + 100)),
            canonicalization_version: 1,
            signing_key_id: "ed25519:b0031-public-test-key".into(),
            authority_signature: vec![0x66; 64],
            trust_domain_id: "raziel-b0031-test".into(),
            policy_record_id: "policy_b0031".into(),
        };
        let capability = ProofCapability {
            format_version: 1,
            capability_id: lowercase_hex(&public_values.capability_request_commitment),
            agent_key_thumbprint: agent_key_thumbprint.to_vec(),
            task_id: "task_1".into(),
            purpose: "reconciliation".into(),
            action: "direct_access".into(),
            resource_type: "dataset".into(),
            resource_scope: "payments".into(),
            policy_hash: public_values.policy_hash.to_vec(),
            relationship_snapshot_root: public_values.relationship_snapshot_root.to_vec(),
            snapshot_id: "snapshot_1".into(),
            capability_request_commitment: public_values.capability_request_commitment.to_vec(),
            issuance_nullifier: public_values.issuance_nullifier.to_vec(),
            issued_at: Some(timestamp(public_values.issued_at_unix_seconds)),
            expires_at: Some(timestamp(public_values.expires_at_unix_seconds)),
            proof_system: ProofSystem::Sp1 as i32,
            verification_key_id: retained
                .trusted_program_key()
                .verification_key_id()
                .to_owned(),
            zk_proof: retained.proof_artifact().to_vec(),
            snapshot_attestation: Some(descriptor.clone()),
        };
        let request = VerifyCapabilityRequest {
            proof_capability: Some(capability),
            agent_public_key: public_key.to_vec(),
            request_method: "POST".into(),
            canonical_request_target: "/payments/reconcile?account=a-1&status=pending".into(),
            request_body_hash: BODY_HASH.to_vec(),
            request_nonce: REQUEST_NONCE.to_vec(),
            request_timestamp: Some(timestamp(NOW)),
            agent_request_signature: Vec::new(),
        };
        let trust = FixtureTrust {
            state: TrustedCapabilityState {
                observed_now_unix_seconds: NOW,
                policy_record_id: "policy_b0031".into(),
                policy_valid_from_unix_seconds: public_values.issued_at_unix_seconds - 100,
                policy_valid_until_unix_seconds: public_values.expires_at_unix_seconds + 100,
                snapshot_issued_at_unix_seconds: public_values.issued_at_unix_seconds - 100,
                snapshot_expires_at_unix_seconds: public_values.expires_at_unix_seconds + 100,
                policy_hash: public_values.policy_hash,
                relationship_snapshot_root: public_values.relationship_snapshot_root,
                descriptor,
            },
            program_key: retained.trusted_program_key().clone(),
        };
        let mut fixture = Self {
            signing_key,
            request,
            trust,
        };
        fixture.resign();
        fixture
    }

    fn capability(&self) -> &ProofCapability {
        self.request.proof_capability.as_ref().unwrap()
    }

    fn capability_mut(&mut self) -> &mut ProofCapability {
        self.request.proof_capability.as_mut().unwrap()
    }

    fn sync_trusted_descriptor(&mut self) {
        let policy_hash = fixed_digest(&self.capability().policy_hash);
        let root = fixed_digest(&self.capability().relationship_snapshot_root);
        let descriptor = {
            let descriptor = self.capability_mut().snapshot_attestation.as_mut().unwrap();
            descriptor.policy_hash = policy_hash.to_vec();
            descriptor.relationship_snapshot_root = root.to_vec();
            descriptor.clone()
        };
        self.trust.state.policy_hash = policy_hash;
        self.trust.state.relationship_snapshot_root = root;
        self.trust.state.descriptor = descriptor;
    }

    fn resign(&mut self) {
        let capability_id = self.capability().capability_id.clone();
        let message = protected_request_signature_message(&ProtectedRequestSignatureInputV1 {
            format_version: PROTECTED_REQUEST_FORMAT_VERSION,
            capability_id: &capability_id,
            request_method: &self.request.request_method,
            canonical_request_target: &self.request.canonical_request_target,
            request_body_hash: &BODY_HASH,
            request_nonce: &REQUEST_NONCE,
            request_timestamp_unix_seconds: NOW,
        })
        .unwrap();
        self.request.agent_request_signature = self.signing_key.sign(&message).to_bytes().to_vec();
    }
}

#[derive(Clone)]
struct FixtureTrust {
    state: TrustedCapabilityState,
    program_key: TrustedProgramKey,
}

impl VerificationTrust for FixtureTrust {
    fn resolve_snapshot(
        &mut self,
        snapshot_id: &str,
        _trusted_now_unix_seconds: i64,
    ) -> Result<TrustedCapabilityState, VerificationAdmissionError> {
        if snapshot_id != self.state.descriptor.snapshot_id {
            return Err(VerificationAdmissionError::SnapshotAttestationMismatch);
        }
        Ok(self.state.clone())
    }

    fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.program_key
    }
}

fn fixed_digest(bytes: &[u8]) -> [u8; 32] {
    bytes.try_into().unwrap()
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
