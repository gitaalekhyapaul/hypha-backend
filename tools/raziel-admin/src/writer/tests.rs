use std::os::unix::fs::PermissionsExt;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    AuthoritySigner, PolicyApprovalRole, SnapshotAuthorityPublisher, SnapshotDescriptorDraftV1,
    SnapshotRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::decode_policy_program_strict;
use raziel_proto::raziel::v1 as proto;
use raziel_snapshot::{RELATIONSHIP_TREE_VERSION, SnapshotDescriptorLimits};
use raziel_verifier::{
    LocalRegistry, PolicyRecordStatus, PolicyRegistryRecordV1, RegistryLoader,
    authority_key_to_proto, policy_record_to_proto, root_record_from_proto,
};

use super::codec::{read_request, write_response};
use super::server::{bind_private_socket, handle_request};
use super::store::{RegistryMutationStore, StoreMutationError, StoreMutationResult};
use super::wire::{RegistryWriterRequestWire, RevokeSnapshotWire, registry_writer_request_wire};
use super::{PROTOCOL_VERSION, RegistryWriterClient};
use crate::args::Arguments;
use crate::config::{registry_config, validation_ceilings};
use crate::registry::publish;

const TEST_NOW: i64 = 1_700_000_000;
const POLICY_BYTES: &[u8] =
    include_bytes!("../../../spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb");
const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

#[test]
fn store_appends_and_revokes_with_monotonic_generations() {
    let fixture = registry_fixture("store");
    let store = RegistryMutationStore::new(fixture.root.clone(), "raziel-test").unwrap();
    let appended = store
        .apply(append_request(1, fixture.descriptor.clone()), TEST_NOW)
        .unwrap();
    let StoreMutationResult::Root(appended) = appended else {
        panic!("snapshot append returned a policy record");
    };
    assert_eq!(appended.registry_generation, 2);
    assert_eq!(
        store.apply(append_request(1, fixture.descriptor.clone()), TEST_NOW),
        Err(StoreMutationError::StaleGeneration)
    );

    let revoked = store
        .apply(
            revoke_request(2, &fixture.snapshot_id, "owner request"),
            TEST_NOW + 1,
        )
        .unwrap();
    let StoreMutationResult::Root(revoked) = revoked else {
        panic!("snapshot revoke returned a policy record");
    };
    assert_eq!(revoked.registry_generation, 3);
    assert_eq!(revoked.status, proto::SnapshotStatus::Revoked as i32);

    let config = registry_config("raziel-test").unwrap();
    let loader = RegistryLoader::new(config.clone());
    let current = loader.load_current(&fixture.root, TEST_NOW + 1).unwrap();
    assert_eq!(current.generation, 3);
    let mut registry = LocalRegistry::new(config);
    registry.install_generation(current).unwrap();
    assert!(
        registry
            .resolve_snapshot_exact(&fixture.snapshot_id, TEST_NOW + 1)
            .is_err()
    );
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn store_publishes_staged_policy_with_trusted_mutable_metadata() {
    let fixture = registry_fixture("policy-store");
    let store = RegistryMutationStore::new(fixture.root.clone(), "raziel-test").unwrap();
    let result = store
        .apply(policy_request(1, fixture.staged_policy.clone()), TEST_NOW)
        .unwrap();
    let StoreMutationResult::Policy(record) = result else {
        panic!("policy append returned a root record");
    };
    assert_eq!(record.status, proto::PolicyStatus::Approved as i32);
    assert_eq!(record.registered_at, Some(timestamp(TEST_NOW)));
    assert_eq!(record.registry_generation, 2);
    assert_eq!(record.record_id, "policy-2");

    let config = registry_config("raziel-test").unwrap();
    let current = RegistryLoader::new(config.clone())
        .load_current(&fixture.root, TEST_NOW)
        .unwrap();
    assert_eq!(current.generation, 2);
    let mut registry = LocalRegistry::new(config);
    registry.install_generation(current).unwrap();
    let hash: [u8; 32] = record.policy_hash.as_slice().try_into().unwrap();
    registry
        .resolve_policy_exact("policy-2", &hash, TEST_NOW)
        .unwrap();

    let appended = store
        .apply(append_request(2, fixture.descriptor), TEST_NOW)
        .unwrap();
    let StoreMutationResult::Root(appended) = appended else {
        panic!("snapshot append returned a policy record");
    };
    assert_eq!(appended.registry_generation, 3);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn store_rejects_mutated_or_precommitted_staged_policy() {
    let fixture = registry_fixture("policy-mutations");
    let store = RegistryMutationStore::new(fixture.root.clone(), "raziel-test").unwrap();
    assert_eq!(
        store.apply(policy_request(2, fixture.staged_policy.clone()), TEST_NOW),
        Err(StoreMutationError::StaleGeneration)
    );

    let mut mutable_records = Vec::new();
    let mut status = fixture.staged_policy.clone();
    status.status = proto::PolicyStatus::Approved as i32;
    mutable_records.push(status);
    let mut registered = fixture.staged_policy.clone();
    registered.registered_at = Some(timestamp(TEST_NOW));
    mutable_records.push(registered);
    let mut revoked = fixture.staged_policy.clone();
    revoked.revoked_at = Some(timestamp(TEST_NOW));
    mutable_records.push(revoked);
    let mut reason = fixture.staged_policy.clone();
    reason.revocation_reason = "caller value".into();
    mutable_records.push(reason);
    let mut generation = fixture.staged_policy.clone();
    generation.registry_generation = 2;
    mutable_records.push(generation);
    for mutable in mutable_records {
        assert_eq!(
            store.apply(policy_request(1, mutable), TEST_NOW),
            Err(StoreMutationError::InvalidRequest)
        );
    }

    let mut bad_signature = fixture.staged_policy.clone();
    bad_signature.authority_signature[0] ^= 1;
    assert_eq!(
        store.apply(policy_request(1, bad_signature), TEST_NOW),
        Err(StoreMutationError::RegistryRejected)
    );

    let mut bad_hash = fixture.staged_policy.clone();
    bad_hash.policy_hash[0] ^= 1;
    assert_eq!(
        store.apply(policy_request(1, bad_hash), TEST_NOW),
        Err(StoreMutationError::RegistryRejected)
    );

    let mut wrong_domain = fixture.staged_policy.clone();
    wrong_domain.trust_domain_id = "other-domain".into();
    assert_eq!(
        store.apply(policy_request(1, wrong_domain), TEST_NOW),
        Err(StoreMutationError::RegistryRejected)
    );

    let signer = AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x01)).unwrap();
    let stale_epoch = staged_policy(&signer, "policy-3", 4);
    assert_eq!(
        store.apply(policy_request(1, stale_epoch), TEST_NOW),
        Err(StoreMutationError::Conflict)
    );
    let duplicate = staged_policy(&signer, "policy-1", 2);
    assert_eq!(
        store.apply(policy_request(1, duplicate), TEST_NOW),
        Err(StoreMutationError::Conflict)
    );
    let expired =
        staged_policy_with_window(&signer, "policy-expired", 2, TEST_NOW - 1_000, TEST_NOW - 1);
    assert_eq!(
        store.apply(policy_request(1, expired), TEST_NOW),
        Err(StoreMutationError::RegistryRejected)
    );
    let untrusted_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x03)).unwrap();
    let wrong_key = staged_policy(&untrusted_signer, "policy-wrong-key", 2);
    assert_eq!(
        store.apply(policy_request(1, wrong_key), TEST_NOW),
        Err(StoreMutationError::RegistryRejected)
    );
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn unix_client_commits_one_public_record_over_owner_only_socket() {
    let fixture = registry_fixture("socket");
    let socket_dir = fixture.root.join("writer-ipc");
    std::fs::create_dir(&socket_dir).unwrap();
    std::fs::set_permissions(&socket_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket_path = socket_dir.join("writer.sock");
    let listener = bind_private_socket(&socket_path).unwrap();
    let socket_metadata = std::fs::symlink_metadata(&socket_path).unwrap();
    assert_eq!(socket_metadata.permissions().mode() & 0o7777, 0o600);

    let store = RegistryMutationStore::new(fixture.root.clone(), "raziel-test").unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream).unwrap();
        let response = handle_request(&store, request, TEST_NOW);
        write_response(&mut stream, &response).unwrap();
    });
    let client = RegistryWriterClient::new(socket_path).unwrap();
    let result = client.append_snapshot(1, fixture.descriptor).unwrap();
    assert_eq!(result.registry_generation, 2);
    assert_eq!(
        result.root_record.descriptor.as_ref().unwrap().snapshot_id,
        fixture.snapshot_id
    );
    server.join().unwrap();
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn unix_client_commits_one_staged_policy_over_owner_only_socket() {
    let fixture = registry_fixture("policy-socket");
    let socket_dir = fixture.root.join("policy-writer-ipc");
    std::fs::create_dir(&socket_dir).unwrap();
    std::fs::set_permissions(&socket_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket_path = socket_dir.join("writer.sock");
    let listener = bind_private_socket(&socket_path).unwrap();
    let store = RegistryMutationStore::new(fixture.root.clone(), "raziel-test").unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream).unwrap();
        let response = handle_request(&store, request, TEST_NOW);
        write_response(&mut stream, &response).unwrap();
    });
    let client = RegistryWriterClient::new(socket_path).unwrap();
    let result = client
        .append_approved_policy(1, fixture.staged_policy)
        .unwrap();
    assert_eq!(result.registry_generation, 2);
    assert_eq!(result.policy_record.record_id, "policy-2");
    assert!(format!("{result:?}").contains("<redacted>"));
    server.join().unwrap();
    std::fs::remove_dir_all(fixture.root).unwrap();
}

struct RegistryFixture {
    root: std::path::PathBuf,
    snapshot_id: String,
    descriptor: proto::RelationshipSnapshotDescriptor,
    staged_policy: proto::PolicyRecord,
}

fn registry_fixture(label: &str) -> RegistryFixture {
    let root = temp_root(label);
    std::fs::create_dir_all(root.join("drafts")).unwrap();
    let policy_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x01)).unwrap();
    let snapshot_signer =
        AuthoritySigner::<SnapshotRole>::from_pkcs8_der(&test_pkcs8(0x02)).unwrap();
    let policy_key = trusted_key(
        policy_signer.key_id(),
        policy_signer.public_key_bytes(),
        AuthorityRole::PolicyApproval,
    );
    let snapshot_key = trusted_key(
        snapshot_signer.key_id(),
        snapshot_signer.public_key_bytes(),
        AuthorityRole::Snapshot,
    );
    let policy = decode_policy_program_strict(
        POLICY_BYTES,
        POLICY_BYTES.len() as u64,
        &validation_ceilings(),
    )
    .unwrap();
    let policy_hash = policy_hash(&policy).unwrap();
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            record_id: "policy-1".into(),
            policy_name: "reconciliation".into(),
            policy_version: "v1".into(),
            approval_epoch: 1,
            policy_hash,
            valid_from_unix_seconds: TEST_NOW - 100,
            valid_until_unix_seconds: TEST_NOW + 10_000,
            approved_by: "maintainer".into(),
            signing_key_id: policy_signer.key_id().into(),
        },
        PolicyApprovalLimits {
            max_string_bytes: 512,
            max_lifetime_seconds: 31 * 24 * 60 * 60,
        },
    )
    .unwrap();
    let policy_record = PolicyRegistryRecordV1 {
        authority_signature: policy_signer.sign_policy_approval(&claims).unwrap(),
        claims,
        policy,
        canonical_policy_ir_bytes: POLICY_BYTES.to_vec(),
        status: PolicyRecordStatus::Approved,
        registered_at_unix_seconds: TEST_NOW,
        revoked_at_unix_seconds: None,
        revocation_reason: None,
        registry_generation: 1,
    };
    let staged_policy = staged_policy(&policy_signer, "policy-2", 2);
    std::fs::write(
        root.join("drafts/policy-key.pb"),
        authority_key_to_proto(&policy_key).encode_to_vec(),
    )
    .unwrap();
    std::fs::write(
        root.join("drafts/snapshot-key.pb"),
        authority_key_to_proto(&snapshot_key).encode_to_vec(),
    )
    .unwrap();
    std::fs::write(
        root.join("drafts/policy.pb"),
        policy_record_to_proto(&policy_record)
            .unwrap()
            .encode_to_vec(),
    )
    .unwrap();
    publish(&arguments([
        "publish-registry",
        "--root",
        root.to_str().unwrap(),
        "--trust-domain",
        "raziel-test",
        "--generation",
        "1",
        "--published-at",
        "1700000000",
        "--observed-now",
        "1700000000",
        "--output-directory",
        "generations/000001",
        "--output-manifest",
        root.join("current.pb").to_str().unwrap(),
        "--output-json",
        root.join("current.json").to_str().unwrap(),
        "--authority-key",
        "drafts/policy-key.pb",
        "--authority-key",
        "drafts/snapshot-key.pb",
        "--policy",
        "drafts/policy.pb",
    ]))
    .unwrap();

    let relationship_root = [0x22; 32];
    let snapshot_id = format!("snapshot-v1:{}", lowercase_hex(&relationship_root));
    let mut publisher =
        SnapshotAuthorityPublisher::new(snapshot_signer, "raziel-test".into(), std::iter::empty())
            .unwrap();
    let signed = publisher
        .sign_next(
            0,
            SnapshotDescriptorDraftV1 {
                snapshot_id: snapshot_id.clone(),
                relationship_snapshot_root: relationship_root,
                policy_record_id: "policy-1".into(),
                policy_hash,
                spicedb_revision: "zed-1".into(),
                issued_at_unix_seconds: TEST_NOW,
                expires_at_unix_seconds: TEST_NOW + 1_000,
                canonicalization_version: RELATIONSHIP_TREE_VERSION,
            },
            SnapshotDescriptorLimits {
                max_string_bytes: 512,
                max_lifetime_seconds: 31 * 24 * 60 * 60,
            },
        )
        .unwrap();
    let descriptor = proto::RelationshipSnapshotDescriptor {
        format_version: signed.claims.claims().format_version,
        snapshot_id: snapshot_id.clone(),
        relationship_snapshot_root: relationship_root.to_vec(),
        policy_hash: policy_hash.to_vec(),
        spicedb_revision: "zed-1".into(),
        epoch: 1,
        issued_at: Some(timestamp(TEST_NOW)),
        expires_at: Some(timestamp(TEST_NOW + 1_000)),
        canonicalization_version: RELATIONSHIP_TREE_VERSION,
        signing_key_id: signed.claims.claims().signing_key_id.clone(),
        authority_signature: signed.authority_signature.to_vec(),
        trust_domain_id: "raziel-test".into(),
        policy_record_id: "policy-1".into(),
    };
    let _ = root_record_from_proto(
        proto::AcceptedRootRecord {
            format_version: 1,
            descriptor: Some(descriptor.clone()),
            status: proto::SnapshotStatus::Accepted as i32,
            registered_at: Some(timestamp(TEST_NOW)),
            revoked_at: None,
            revocation_reason: String::new(),
            registry_generation: 2,
        },
        &registry_config("raziel-test").unwrap(),
        TEST_NOW,
    )
    .unwrap();
    RegistryFixture {
        root,
        snapshot_id,
        descriptor,
        staged_policy,
    }
}

fn policy_request(
    generation: u64,
    staged_policy: proto::PolicyRecord,
) -> RegistryWriterRequestWire {
    RegistryWriterRequestWire {
        format_version: PROTOCOL_VERSION,
        expected_registry_generation: generation,
        operation: Some(
            registry_writer_request_wire::Operation::AppendApprovedPolicy(Box::new(staged_policy)),
        ),
    }
}

fn staged_policy(
    signer: &AuthoritySigner<PolicyApprovalRole>,
    record_id: &str,
    approval_epoch: u64,
) -> proto::PolicyRecord {
    staged_policy_with_window(
        signer,
        record_id,
        approval_epoch,
        TEST_NOW - 100,
        TEST_NOW + 10_000,
    )
}

fn staged_policy_with_window(
    signer: &AuthoritySigner<PolicyApprovalRole>,
    record_id: &str,
    approval_epoch: u64,
    valid_from: i64,
    valid_until: i64,
) -> proto::PolicyRecord {
    let policy = decode_policy_program_strict(
        POLICY_BYTES,
        POLICY_BYTES.len() as u64,
        &validation_ceilings(),
    )
    .unwrap();
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            record_id: record_id.into(),
            policy_name: "reconciliation".into(),
            policy_version: format!("v{approval_epoch}"),
            approval_epoch,
            policy_hash: policy_hash(&policy).unwrap(),
            valid_from_unix_seconds: valid_from,
            valid_until_unix_seconds: valid_until,
            approved_by: "maintainer".into(),
            signing_key_id: signer.key_id().into(),
        },
        PolicyApprovalLimits {
            max_string_bytes: 512,
            max_lifetime_seconds: 31 * 24 * 60 * 60,
        },
    )
    .unwrap();
    proto::PolicyRecord {
        format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
        record_id: record_id.into(),
        policy_name: "reconciliation".into(),
        policy_version: format!("v{approval_epoch}"),
        canonical_policy_ir: Some(proto::PolicyProgram::decode(POLICY_BYTES).unwrap()),
        policy_hash: claims.claims().policy_hash.to_vec(),
        status: proto::PolicyStatus::Unspecified as i32,
        valid_from: Some(timestamp(valid_from)),
        valid_until: Some(timestamp(valid_until)),
        approved_by: "maintainer".into(),
        signing_key_id: signer.key_id().into(),
        authority_signature: signer.sign_policy_approval(&claims).unwrap().to_vec(),
        registered_at: None,
        trust_domain_id: "raziel-test".into(),
        approval_epoch,
        revoked_at: None,
        revocation_reason: String::new(),
        registry_generation: 0,
    }
}

fn append_request(
    generation: u64,
    descriptor: proto::RelationshipSnapshotDescriptor,
) -> RegistryWriterRequestWire {
    RegistryWriterRequestWire {
        format_version: PROTOCOL_VERSION,
        expected_registry_generation: generation,
        operation: Some(registry_writer_request_wire::Operation::AppendSnapshot(
            Box::new(descriptor),
        )),
    }
}

fn revoke_request(generation: u64, snapshot_id: &str, reason: &str) -> RegistryWriterRequestWire {
    RegistryWriterRequestWire {
        format_version: PROTOCOL_VERSION,
        expected_registry_generation: generation,
        operation: Some(registry_writer_request_wire::Operation::RevokeSnapshot(
            RevokeSnapshotWire {
                snapshot_id: snapshot_id.into(),
                reason: reason.into(),
            },
        )),
    }
}

fn trusted_key(key_id: &str, public_key: [u8; 32], role: AuthorityRole) -> TrustedAuthorityKey {
    TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            key_id: key_id.into(),
            role,
            public_key,
            status: AuthorityKeyStatus::Active,
            valid_from_unix_seconds: TEST_NOW - 1_000,
            valid_until_unix_seconds: TEST_NOW + 10_000,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        },
        "raziel-test",
    )
    .unwrap()
}

fn arguments<const N: usize>(values: [&str; N]) -> Arguments {
    Arguments::parse(values.into_iter().map(str::to_owned)).unwrap()
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::path::Path::new("/tmp").join(format!("rzw-{label}-{}-{unique}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    root
}

fn lowercase_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
}
