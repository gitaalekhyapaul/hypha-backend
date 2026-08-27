use std::io::Write;
use std::net::Shutdown;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    AuthoritySigner, PolicyApprovalRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::decode_policy_program_strict;
use raziel_proto::raziel::v1 as proto;
use raziel_verifier::{
    PolicyRecordStatus, PolicyRegistryRecordV1, authority_key_to_proto, policy_record_to_proto,
};

use super::codec::{write_message, write_response};
use super::server::{bind_private_socket, handle_connection};
use super::store::{PolicyApprovalStore, StoreApprovalError};
use super::wire::{
    PolicyAuthorityFailureWire, PolicyAuthorityRequestWire, PolicyCandidateManifestWire,
};
use super::{PROTOCOL_VERSION, PolicyAuthorityApprovalRequest, PolicyAuthorityClient};
use crate::args::Arguments;
use crate::config::{MAX_RECORD_BYTES, MAX_STRING_BYTES, validation_ceilings};
use crate::registry::publish;

const POLICY_BYTES: &[u8] =
    include_bytes!("../../../spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb");
const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

#[test]
fn authority_signs_one_staged_record_and_retries_idempotently() {
    let fixture = fixture(false, 0x01);
    let store = fixture.store(0x01);
    let public_request = PolicyAuthorityApprovalRequest {
        candidate_id: fixture.request.candidate_id.clone(),
        expected_policy_hash: fixture
            .request
            .expected_policy_hash
            .as_slice()
            .try_into()
            .unwrap(),
        expected_approval_epoch: fixture.request.expected_approval_epoch,
        valid_from_unix_seconds: fixture.request.valid_from_unix_seconds,
        valid_until_unix_seconds: fixture.request.valid_until_unix_seconds,
        approved_by: fixture.request.approved_by.clone(),
        approval_nonce: [0x33; 32],
    };
    let debug = format!("{public_request:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("[51, 51"));

    let first = store.apply(fixture.request.clone(), fixture.now).unwrap();
    assert_staged_record(&fixture, &first);
    assert_eq!(
        std::fs::metadata(fixture.candidate_dir.join("approval.pb"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    let stored_bytes = std::fs::read(fixture.candidate_dir.join("approval.pb")).unwrap();
    assert!(!stored_bytes.windows(32).any(|window| window == [0x33; 32]));
    assert!(!stored_bytes.windows(32).any(|window| window == [0x01; 32]));

    let second = store
        .apply(fixture.request.clone(), fixture.now + 4_000)
        .unwrap();
    assert_eq!(first, second);

    let mut conflicting = fixture.request.clone();
    conflicting.approval_nonce = vec![0x44; 32];
    assert_eq!(
        store.apply(conflicting, fixture.now),
        Err(StoreApprovalError::Conflict)
    );
    fixture.remove();
}

#[test]
fn authority_rejects_hash_epoch_expiry_duplicate_record_and_wrong_key() {
    let hash_fixture = fixture(false, 0x01);
    let store = hash_fixture.store(0x01);
    let mut bad_hash = hash_fixture.request.clone();
    bad_hash.expected_policy_hash = vec![0x99; 32];
    assert_eq!(
        store.apply(bad_hash, hash_fixture.now),
        Err(StoreApprovalError::CandidateRejected)
    );
    let mut bad_epoch = hash_fixture.request.clone();
    bad_epoch.expected_approval_epoch = 2;
    assert_eq!(
        store.apply(bad_epoch, hash_fixture.now),
        Err(StoreApprovalError::StaleEpoch)
    );
    let mut bad_window = hash_fixture.request.clone();
    bad_window.valid_until_unix_seconds -= 1;
    assert_eq!(
        store.apply(bad_window, hash_fixture.now),
        Err(StoreApprovalError::CandidateRejected)
    );
    let mut bad_path = hash_fixture.request.clone();
    bad_path.candidate_id = "../candidate-001".into();
    assert_eq!(
        store.apply(bad_path, hash_fixture.now),
        Err(StoreApprovalError::InvalidRequest)
    );
    hash_fixture.remove();

    let expired_fixture = fixture(false, 0x01);
    assert_eq!(
        expired_fixture
            .store(0x01)
            .apply(expired_fixture.request.clone(), expired_fixture.now + 4_000),
        Err(StoreApprovalError::CandidateRejected)
    );
    expired_fixture.remove();

    let duplicate_fixture = fixture(true, 0x01);
    assert_eq!(
        duplicate_fixture
            .store(0x01)
            .apply(duplicate_fixture.request.clone(), duplicate_fixture.now),
        Err(StoreApprovalError::Conflict)
    );
    duplicate_fixture.remove();

    let wrong_key_fixture = fixture(false, 0x02);
    assert_eq!(
        wrong_key_fixture
            .store(0x01)
            .apply(wrong_key_fixture.request.clone(), wrong_key_fixture.now),
        Err(StoreApprovalError::CandidateRejected)
    );
    wrong_key_fixture.remove();
}

#[test]
fn authority_rejects_non_private_candidate_and_altered_manifest() {
    let permission_fixture = fixture(false, 0x01);
    std::fs::set_permissions(
        permission_fixture.candidate_dir.join("policy.pb"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(
        permission_fixture
            .store(0x01)
            .apply(permission_fixture.request.clone(), permission_fixture.now),
        Err(StoreApprovalError::CandidateRejected)
    );
    permission_fixture.remove();

    let altered_fixture = fixture(false, 0x01);
    let manifest_path = altered_fixture.candidate_dir.join("candidate.pb");
    let mut bytes = std::fs::read(&manifest_path).unwrap();
    bytes.extend_from_slice(&[0x98, 0x06, 0x01]);
    write_private(&manifest_path, &bytes);
    assert_eq!(
        altered_fixture
            .store(0x01)
            .apply(altered_fixture.request.clone(), altered_fixture.now),
        Err(StoreApprovalError::CandidateRejected)
    );
    altered_fixture.remove();
}

#[test]
fn unix_client_receives_only_the_staged_record() {
    let fixture = fixture(false, 0x01);
    let socket_root = std::path::PathBuf::from(format!(
        "/tmp/raziel-pa-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let socket_path = socket_root.join("authority.sock");
    let listener = bind_private_socket(&socket_path).unwrap();
    let store = fixture.store(0x01);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let response = handle_connection(&store, &mut stream);
        write_response(&mut stream, &response).unwrap();
    });
    let client = PolicyAuthorityClient::new(socket_path).unwrap();
    let result = client
        .approve(PolicyAuthorityApprovalRequest {
            candidate_id: fixture.request.candidate_id.clone(),
            expected_policy_hash: fixture
                .request
                .expected_policy_hash
                .as_slice()
                .try_into()
                .unwrap(),
            expected_approval_epoch: fixture.request.expected_approval_epoch,
            valid_from_unix_seconds: fixture.request.valid_from_unix_seconds,
            valid_until_unix_seconds: fixture.request.valid_until_unix_seconds,
            approved_by: fixture.request.approved_by.clone(),
            approval_nonce: fixture
                .request
                .approval_nonce
                .as_slice()
                .try_into()
                .unwrap(),
        })
        .unwrap();
    assert_staged_record(&fixture, &result.staged_policy_record);
    server.join().unwrap();
    std::fs::remove_dir_all(socket_root).unwrap();
    fixture.remove();
}

#[test]
fn protocol_rejects_malformed_oversized_and_trailing_frames() {
    let fixture = fixture(false, 0x01);
    for payload in [
        vec![0, 0, 0, 1, 0xff],
        [u32::MAX.to_be_bytes().as_slice(), &[0]].concat(),
    ] {
        let store = fixture.store(0x01);
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_all(&payload).unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let response = handle_connection(&store, &mut server);
        assert_eq!(
            response.failure,
            PolicyAuthorityFailureWire::InvalidRequest as i32
        );
    }

    let store = fixture.store(0x01);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    write_message(&mut client, &fixture.request).unwrap();
    client.write_all(&[0]).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let response = handle_connection(&store, &mut server);
    assert_eq!(
        response.failure,
        PolicyAuthorityFailureWire::InvalidRequest as i32
    );
    fixture.remove();
}

struct Fixture {
    root: std::path::PathBuf,
    candidate_root: std::path::PathBuf,
    candidate_dir: std::path::PathBuf,
    registry_root: std::path::PathBuf,
    request: PolicyAuthorityRequestWire,
    now: i64,
    trusted_key: TrustedAuthorityKey,
}

impl Fixture {
    fn store(&self, signer_seed: u8) -> PolicyApprovalStore {
        PolicyApprovalStore::new(
            self.candidate_root.clone(),
            self.registry_root.clone(),
            "raziel-test",
            AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(signer_seed))
                .unwrap(),
        )
        .unwrap()
    }

    fn remove(self) {
        std::fs::remove_dir_all(self.root).unwrap();
    }
}

fn fixture(existing_policy: bool, registry_key_seed: u8) -> Fixture {
    let now = current_time();
    let root = temp_root();
    set_mode(&root, 0o700);
    let candidate_root = root.join("candidates");
    let candidate_dir = candidate_root.join("candidate-001");
    std::fs::create_dir(&candidate_root).unwrap();
    std::fs::create_dir(&candidate_dir).unwrap();
    set_mode(&candidate_root, 0o700);
    set_mode(&candidate_dir, 0o700);

    let policy =
        decode_policy_program_strict(POLICY_BYTES, MAX_RECORD_BYTES, &validation_ceilings())
            .unwrap();
    let digest = policy_hash(&policy).unwrap();
    let expected_epoch = if existing_policy { 2 } else { 1 };
    let manifest = PolicyCandidateManifestWire {
        format_version: 1,
        candidate_id: "candidate-001".into(),
        record_id: "policy-reconciliation-001".into(),
        policy_name: "reconciliation-query".into(),
        policy_version: "1".into(),
        resource_type: "payment_batch".into(),
        permission: "view".into(),
        rule_summary: "ALL(viewer, owner)".into(),
        source_schema_digest: policy.program().source_schema_digest.to_vec(),
        policy_hash: digest.to_vec(),
        expected_approval_epoch: expected_epoch,
        valid_from_unix_seconds: now - 60,
        valid_until_unix_seconds: now + 86_400,
        created_at_unix_seconds: now - 60,
        expires_at_unix_seconds: now + 3_600,
    };
    write_private(&candidate_dir.join("policy.pb"), POLICY_BYTES);
    write_private(
        &candidate_dir.join("candidate.pb"),
        &manifest.encode_to_vec(),
    );

    let registry_root = root.join("registry");
    std::fs::create_dir(&registry_root).unwrap();
    let registry_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(registry_key_seed))
            .unwrap();
    let trusted_key = TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            key_id: registry_signer.key_id().into(),
            role: AuthorityRole::PolicyApproval,
            public_key: registry_signer.public_key_bytes(),
            status: AuthorityKeyStatus::Active,
            valid_from_unix_seconds: now - 3_600,
            valid_until_unix_seconds: now + 172_800,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        },
        "raziel-test",
    )
    .unwrap();
    publish_registry(
        &registry_root,
        now,
        &registry_signer,
        &trusted_key,
        existing_policy,
        policy,
        digest,
    );

    Fixture {
        root,
        candidate_root,
        candidate_dir,
        registry_root,
        request: PolicyAuthorityRequestWire {
            format_version: PROTOCOL_VERSION,
            candidate_id: "candidate-001".into(),
            expected_policy_hash: digest.to_vec(),
            expected_approval_epoch: expected_epoch,
            valid_from_unix_seconds: now - 60,
            valid_until_unix_seconds: now + 86_400,
            approved_by: "demo-owner".into(),
            approval_nonce: vec![0x33; 32],
        },
        now,
        trusted_key,
    }
}

fn publish_registry(
    root: &std::path::Path,
    now: i64,
    signer: &AuthoritySigner<PolicyApprovalRole>,
    trusted_key: &TrustedAuthorityKey,
    existing_policy: bool,
    policy: raziel_policy_ir::ValidatedPolicyProgram,
    digest: [u8; 32],
) {
    std::fs::create_dir(root.join("drafts")).unwrap();
    std::fs::write(
        root.join("drafts/policy-key.pb"),
        authority_key_to_proto(trusted_key).encode_to_vec(),
    )
    .unwrap();
    let mut arguments = vec![
        "publish-registry".into(),
        "--root".into(),
        root.to_str().unwrap().into(),
        "--trust-domain".into(),
        "raziel-test".into(),
        "--generation".into(),
        "1".into(),
        "--published-at".into(),
        now.to_string(),
        "--observed-now".into(),
        now.to_string(),
        "--output-directory".into(),
        "generations/000001".into(),
        "--output-manifest".into(),
        root.join("current.pb").to_str().unwrap().into(),
        "--output-json".into(),
        root.join("current.json").to_str().unwrap().into(),
        "--authority-key".into(),
        "drafts/policy-key.pb".into(),
    ];
    if existing_policy {
        let claims = ValidatedPolicyApprovalClaimsV1::new(
            PolicyApprovalClaimsV1 {
                format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                record_id: "policy-reconciliation-001".into(),
                policy_name: "reconciliation-query".into(),
                policy_version: "1".into(),
                approval_epoch: 1,
                policy_hash: digest,
                valid_from_unix_seconds: now - 60,
                valid_until_unix_seconds: now + 86_400,
                approved_by: "prior-owner".into(),
                signing_key_id: signer.key_id().into(),
            },
            PolicyApprovalLimits {
                max_string_bytes: MAX_STRING_BYTES,
                max_lifetime_seconds: 31 * 24 * 60 * 60,
            },
        )
        .unwrap();
        let record = PolicyRegistryRecordV1 {
            authority_signature: signer.sign_policy_approval(&claims).unwrap(),
            claims,
            policy,
            canonical_policy_ir_bytes: POLICY_BYTES.to_vec(),
            status: PolicyRecordStatus::Approved,
            registered_at_unix_seconds: now,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        };
        std::fs::write(
            root.join("drafts/policy.pb"),
            policy_record_to_proto(&record).unwrap().encode_to_vec(),
        )
        .unwrap();
        arguments.extend(["--policy".into(), "drafts/policy.pb".into()]);
    }
    publish(&Arguments::parse(arguments).unwrap()).unwrap();
}

fn assert_staged_record(fixture: &Fixture, record: &proto::PolicyRecord) {
    assert_eq!(record.status, proto::PolicyStatus::Unspecified as i32);
    assert!(record.registered_at.is_none());
    assert_eq!(record.registry_generation, 0);
    assert!(record.revoked_at.is_none());
    assert!(record.revocation_reason.is_empty());
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: record.format_version,
            trust_domain_id: record.trust_domain_id.clone(),
            record_id: record.record_id.clone(),
            policy_name: record.policy_name.clone(),
            policy_version: record.policy_version.clone(),
            approval_epoch: record.approval_epoch,
            policy_hash: record.policy_hash.as_slice().try_into().unwrap(),
            valid_from_unix_seconds: record.valid_from.as_ref().unwrap().seconds,
            valid_until_unix_seconds: record.valid_until.as_ref().unwrap().seconds,
            approved_by: record.approved_by.clone(),
            signing_key_id: record.signing_key_id.clone(),
        },
        PolicyApprovalLimits {
            max_string_bytes: MAX_STRING_BYTES,
            max_lifetime_seconds: 31 * 24 * 60 * 60,
        },
    )
    .unwrap();
    fixture
        .trusted_key
        .verify_policy_approval(fixture.now, &claims, &record.authority_signature)
        .unwrap();
}

fn write_private(path: &std::path::Path, bytes: &[u8]) {
    let _ = std::fs::remove_file(path);
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

fn set_mode(path: &std::path::Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
}

fn current_time() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap()
}

fn temp_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "raziel-policy-authority-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
