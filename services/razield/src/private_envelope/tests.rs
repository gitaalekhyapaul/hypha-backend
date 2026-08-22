use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, capability_request_commitment,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CreateCapabilityRequest, authorization_context_entry,
};

use super::codec;
use super::wire::PrivateCapabilityEnvelopeWireV1;
use super::{
    DeleteStatus, MAX_PRIVATE_ENVELOPE_BYTES, PRIVATE_ENVELOPE_FORMAT_VERSION,
    PrivateEnvelopeError, PrivateEnvelopeRef, PrivateEnvelopeStore,
};
use crate::admission::{
    AdmittedCapabilityRequest, LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
    admit_create_capability_request,
};

const TRUSTED_NOW: i64 = 1_800_000_000;
const PRIVATE_CONTEXT_VALUE: &str = "private-merchant-4829";
const DISPLAY_AGENT_ID: &str = "reconciliation-agent";

static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn round_trip_keeps_only_validated_proof_worker_fields_and_owner_modes() {
    let root = TestRoot::new("round-trip");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let signing_key = signing_key();
    let transport = signed_request(valid_request(&signing_key, 0xa5), &signing_key);
    let public_key = transport.agent_public_key.clone();
    let signature = transport.request_signature.clone();
    let admitted = admit_create_capability_request(transport, TRUSTED_NOW).unwrap();

    let reference = store.stage(&admitted).unwrap();
    let loaded = store.read(&reference).unwrap();

    assert_eq!(loaded.request(), admitted.request());
    assert_eq!(
        loaded.capability_request_commitment(),
        admitted.capability_request_commitment()
    );
    assert_eq!(
        PrivateEnvelopeRef::from_queue_token(&reference.queue_token()).unwrap(),
        reference
    );
    assert_eq!(reference.queue_token().len(), 68);

    let final_path = store.final_path_for_test(&reference);
    let bytes = std::fs::read(&final_path).unwrap();
    assert!(contains_bytes(&bytes, PRIVATE_CONTEXT_VALUE.as_bytes()));
    assert!(!contains_bytes(&bytes, DISPLAY_AGENT_ID.as_bytes()));
    assert!(!contains_bytes(&bytes, &public_key));
    assert!(!contains_bytes(&bytes, &signature));
    assert_eq!(mode(root.path()), 0o700);
    assert_eq!(mode(&final_path), 0o600);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn queue_reference_parser_rejects_noncanonical_and_path_tokens() {
    let valid = format!("pe1_{}", "ab".repeat(32));
    assert!(PrivateEnvelopeRef::from_queue_token(&valid).is_ok());

    for invalid in [
        "",
        "pe1_00",
        &format!("pe1_{}", "AB".repeat(32)),
        &format!("../pe1_{}", "00".repeat(32)),
        &format!("pe1_{}", "gg".repeat(32)),
        &format!("pe2_{}", "00".repeat(32)),
    ] {
        assert_eq!(
            PrivateEnvelopeRef::from_queue_token(invalid),
            Err(PrivateEnvelopeError::InvalidReference)
        );
    }
}

#[test]
fn maximum_valid_request_fits_the_fixed_envelope_ceiling() {
    let root = TestRoot::new("maximum");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let signing_key = signing_key();
    let mut transport = valid_request(&signing_key, 0xa6);
    transport.agent_id = "d".repeat(256);
    transport.task_id = "t".repeat(256);
    transport.purpose = "p".repeat(256);
    transport.action = "a".repeat(256);
    transport.resource_type = "r".repeat(256);
    transport.resource_scope = "s".repeat(256);
    transport.snapshot_id = "n".repeat(256);
    transport.request_context = (0..16)
        .map(|index| AuthorizationContextEntry {
            name: format!("a{index:02}{}", "x".repeat(253)),
            value: Some(authorization_context_entry::Value::StringValue(
                "v".repeat(256),
            )),
        })
        .collect();
    let transport = signed_request(transport, &signing_key);
    let admitted = admit_create_capability_request(transport, TRUSTED_NOW).unwrap();
    let (_, encoded) = codec::encode(&admitted).unwrap();

    assert!(encoded.len() < MAX_PRIVATE_ENVELOPE_BYTES);
    let reference = store.stage(&admitted).unwrap();
    assert_eq!(
        store.read(&reference).unwrap().request(),
        admitted.request()
    );
}

#[test]
fn concurrent_duplicate_staging_publishes_exactly_one_envelope() {
    const WRITERS: usize = 8;

    let root = TestRoot::new("concurrent");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::new();

    for _ in 0..WRITERS {
        let store = store.clone();
        let barrier = Arc::clone(&barrier);
        let admitted = admitted_request(0xa7);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            store.stage(&admitted)
        }));
    }

    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter(|result| result.is_err())
            .all(|result| { matches!(result, Err(PrivateEnvelopeError::EnvelopeAlreadyExists)) })
    );

    let reference = results.into_iter().find_map(Result::ok).unwrap();
    store.read(&reference).unwrap();
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn stage_never_replaces_existing_files_links_or_directories() {
    let root = TestRoot::new("no-replace");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xa8);
    let (reference, _) = codec::encode(&admitted).unwrap();
    let final_path = store.final_path_for_test(&reference);

    write_private(&final_path, b"existing");
    assert_eq!(
        store.stage(&admitted),
        Err(PrivateEnvelopeError::EnvelopeAlreadyExists)
    );
    assert_eq!(std::fs::read(&final_path).unwrap(), b"existing");
    std::fs::remove_file(&final_path).unwrap();

    let outside = root.sibling("outside-private-envelope");
    write_private(&outside, b"outside");
    symlink(&outside, &final_path).unwrap();
    assert_eq!(
        store.stage(&admitted),
        Err(PrivateEnvelopeError::UnsafeEnvelopeFile)
    );
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
    std::fs::remove_file(&final_path).unwrap();
    std::fs::remove_file(&outside).unwrap();

    std::fs::create_dir(&final_path).unwrap();
    assert_eq!(
        store.stage(&admitted),
        Err(PrivateEnvelopeError::UnsafeEnvelopeFile)
    );
    assert!(final_path.is_dir());
    std::fs::remove_dir(&final_path).unwrap();
}

#[test]
fn reads_reject_wrong_modes_symbolic_links_hard_links_and_oversized_files() {
    let root = TestRoot::new("unsafe-read");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xa9);
    let reference = store.stage(&admitted).unwrap();
    let final_path = store.final_path_for_test(&reference);

    std::fs::set_permissions(&final_path, std::fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::UnsafeEnvelopeFile)
    );
    std::fs::set_permissions(&final_path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let extra_link = root.path().join("extra-link");
    std::fs::hard_link(&final_path, &extra_link).unwrap();
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::UnsafeEnvelopeFile)
    );
    std::fs::remove_file(&extra_link).unwrap();

    OpenOptions::new()
        .write(true)
        .open(&final_path)
        .unwrap()
        .set_len((MAX_PRIVATE_ENVELOPE_BYTES + 1) as u64)
        .unwrap();
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::EnvelopeTooLarge)
    );
    assert_eq!(store.delete(&reference).unwrap(), DeleteStatus::Deleted);

    let outside = root.sibling("outside-read-target");
    write_private(&outside, b"outside");
    symlink(&outside, &final_path).unwrap();
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::UnsafeEnvelopeFile)
    );
    assert_eq!(
        store.delete(&reference),
        Err(PrivateEnvelopeError::UnsafeEnvelopeFile)
    );
    std::fs::remove_file(&final_path).unwrap();
    std::fs::remove_file(&outside).unwrap();
}

#[test]
fn strict_decode_rejects_unknown_fields_malformed_bytes_and_wrong_versions() {
    let root = TestRoot::new("strict-codec");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xaa);
    let (reference, baseline) = codec::encode(&admitted).unwrap();
    let final_path = store.final_path_for_test(&reference);

    let mut with_unknown_field = baseline.clone();
    with_unknown_field.extend_from_slice(&[0x78, 0x01]);
    write_private(&final_path, &with_unknown_field);
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::MalformedEnvelope)
    );

    write_private_replace(&final_path, &[0xff, 0xff]);
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::MalformedEnvelope)
    );

    let mut wire = PrivateCapabilityEnvelopeWireV1::decode(baseline.as_slice()).unwrap();
    assert!(!format!("{wire:?}").contains(PRIVATE_CONTEXT_VALUE));
    wire.envelope_format_version = PRIVATE_ENVELOPE_FORMAT_VERSION + 1;
    write_private_replace(&final_path, &wire.encode_to_vec());
    assert_eq!(
        store.read(&reference),
        Err(PrivateEnvelopeError::UnsupportedEnvelopeVersion)
    );
}

#[test]
fn file_swaps_and_request_mutations_fail_commitment_binding() {
    let root = TestRoot::new("commitment-swap");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let first = admitted_request(0xab);
    let second = admitted_request(0xac);
    let first_reference = store.stage(&first).unwrap();
    let second_reference = store.stage(&second).unwrap();
    let first_bytes = std::fs::read(store.final_path_for_test(&first_reference)).unwrap();
    write_private_replace(&store.final_path_for_test(&second_reference), &first_bytes);

    assert_eq!(
        store.read(&second_reference),
        Err(PrivateEnvelopeError::CommitmentMismatch)
    );

    let mut wire = PrivateCapabilityEnvelopeWireV1::decode(first_bytes.as_slice()).unwrap();
    wire.task_id.push_str("-changed");
    write_private_replace(
        &store.final_path_for_test(&first_reference),
        &wire.encode_to_vec(),
    );
    assert_eq!(
        store.read(&first_reference),
        Err(PrivateEnvelopeError::CommitmentMismatch)
    );
}

#[test]
fn deletion_is_idempotent_and_can_remove_malformed_private_files() {
    let root = TestRoot::new("delete");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xad);
    let reference = store.stage(&admitted).unwrap();
    let final_path = store.final_path_for_test(&reference);
    write_private_replace(&final_path, b"malformed-private-data");

    assert_eq!(store.delete(&reference).unwrap(), DeleteStatus::Deleted);
    assert_eq!(store.delete(&reference).unwrap(), DeleteStatus::NotFound);
    assert!(!final_path.exists());
}

#[test]
fn orphan_cleanup_honors_live_set_grace_and_stale_temporary_files() {
    let root = TestRoot::new("cleanup");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let live = admitted_request(0xae);
    let orphan = admitted_request(0xaf);
    let fresh = admitted_request(0xb0);
    let live_reference = store.stage(&live).unwrap();
    let orphan_reference = store.stage(&orphan).unwrap();
    let fresh_reference = store.stage(&fresh).unwrap();

    let temporary_path = root
        .path()
        .join(format!(".{}.tmp-999-999", orphan_reference.file_name()));
    write_private(&temporary_path, b"staged");
    let live_set = HashSet::from([live_reference]);
    let before_fresh = SystemTime::now();
    std::thread::sleep(Duration::from_millis(5));
    touch(&store.final_path_for_test(&fresh_reference));

    let summary = store.cleanup_orphans(&live_set, before_fresh).unwrap();
    assert_eq!(summary.envelopes_deleted, 1);
    assert_eq!(summary.temporary_files_deleted, 1);
    store.read(&live_reference).unwrap();
    store.read(&fresh_reference).unwrap();
    assert_eq!(
        store.read(&orphan_reference),
        Err(PrivateEnvelopeError::EnvelopeNotFound)
    );
    assert!(!temporary_path.exists());
}

#[test]
fn cleanup_validates_the_complete_directory_before_deleting_any_file() {
    let root = TestRoot::new("cleanup-fail-closed");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let orphan = admitted_request(0xb1);
    let reference = store.stage(&orphan).unwrap();
    let unknown = root.path().join("unexpected-private-file");
    write_private(&unknown, b"unknown");

    assert_eq!(
        store.cleanup_orphans(&HashSet::new(), SystemTime::now() + Duration::from_secs(1)),
        Err(PrivateEnvelopeError::CleanupFailed)
    );
    store.read(&reference).unwrap();
    assert!(unknown.exists());
}

#[test]
fn reference_scan_is_sorted_and_fails_closed_on_unknown_entries() {
    let root = TestRoot::new("reference-scan");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let first = store.stage(&admitted_request(0xc1)).unwrap();
    let second = store.stage(&admitted_request(0xc0)).unwrap();
    let mut expected = vec![first, second];
    expected.sort_unstable();

    assert_eq!(store.scan_references().unwrap(), expected);

    let unknown = root.path().join("unexpected-private-file");
    write_private(&unknown, b"unknown");
    assert_eq!(
        store.scan_references(),
        Err(PrivateEnvelopeError::CleanupFailed)
    );
    store.read(&first).unwrap();
    store.read(&second).unwrap();
}

#[test]
fn store_rejects_insecure_roots_and_detects_mode_changes() {
    let insecure = TestRoot::new("insecure-root");
    std::fs::create_dir(insecure.path()).unwrap();
    std::fs::set_permissions(insecure.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        PrivateEnvelopeStore::open(insecure.path()),
        Err(PrivateEnvelopeError::StoreRootUnsafe)
    ));
    std::fs::set_permissions(insecure.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

    let linked = TestRoot::new("linked-root");
    let target = linked.sibling("private-envelope-root-target");
    std::fs::create_dir(&target).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&target, linked.path()).unwrap();
    assert!(matches!(
        PrivateEnvelopeStore::open(linked.path()),
        Err(PrivateEnvelopeError::StoreRootUnsafe)
    ));
    std::fs::remove_file(linked.path()).unwrap();
    std::fs::remove_dir(target).unwrap();

    let changed = TestRoot::new("changed-root");
    let store = PrivateEnvelopeStore::open(changed.path()).unwrap();
    std::fs::set_permissions(changed.path(), std::fs::Permissions::from_mode(0o750)).unwrap();
    assert_eq!(
        store.stage(&admitted_request(0xb2)),
        Err(PrivateEnvelopeError::StoreRootUnsafe)
    );
    std::fs::set_permissions(changed.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

    let replaced = TestRoot::new("replaced-root");
    let store = PrivateEnvelopeStore::open(replaced.path()).unwrap();
    let original = replaced.sibling("original-root");
    std::fs::rename(replaced.path(), &original).unwrap();
    std::fs::create_dir(replaced.path()).unwrap();
    std::fs::set_permissions(replaced.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        store.stage(&admitted_request(0xb4)),
        Err(PrivateEnvelopeError::StoreRootUnsafe)
    );
    std::fs::remove_dir(replaced.path()).unwrap();
    std::fs::rename(original, replaced.path()).unwrap();
}

#[test]
fn debug_and_error_output_redact_paths_tokens_and_private_values() {
    let root = TestRoot::new("redaction-private-path");
    let store = PrivateEnvelopeStore::open(root.path()).unwrap();
    let admitted = admitted_request(0xb3);
    let reference = store.stage(&admitted).unwrap();
    let envelope = store.read(&reference).unwrap();
    let token = reference.queue_token();
    let outputs = [
        format!("{store:?}"),
        format!("{reference:?}"),
        format!("{envelope:?}"),
        format!(
            "{:?} {}",
            PrivateEnvelopeError::CommitmentMismatch,
            PrivateEnvelopeError::CommitmentMismatch
        ),
    ];

    for output in outputs {
        assert!(!output.contains(root.path().to_str().unwrap()));
        assert!(!output.contains(&token));
        assert!(!output.contains(PRIVATE_CONTEXT_VALUE));
    }
}

fn admitted_request(nonce_byte: u8) -> AdmittedCapabilityRequest {
    let signing_key = signing_key();
    let transport = signed_request(valid_request(&signing_key, nonce_byte), &signing_key);
    admit_create_capability_request(transport, TRUSTED_NOW).unwrap()
}

fn valid_request(signing_key: &SigningKey, nonce_byte: u8) -> CreateCapabilityRequest {
    CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: DISPLAY_AGENT_ID.into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: "task-2026-08-21".into(),
        purpose: "reconcile merchant settlement records".into(),
        action: "reconcile".into(),
        resource_type: "payment_batch".into(),
        resource_scope: "batch:synthetic-001".into(),
        requested_expires_at: Some(Timestamp {
            seconds: TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS,
            nanos: 0,
        }),
        issuance_nonce: vec![nonce_byte; 32],
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
                name: "reviewed".into(),
                value: Some(authorization_context_entry::Value::BoolValue(true)),
            },
            AuthorizationContextEntry {
                name: "value_limit".into(),
                value: Some(authorization_context_entry::Value::IntValue(10_000)),
            },
        ],
    }
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

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn write_private_replace(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .truncate(true)
        .write(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn touch(path: &Path) {
    let file = OpenOptions::new().write(true).open(path).unwrap();
    let times = std::fs::FileTimes::new().set_modified(SystemTime::now());
    file.set_times(times).unwrap();
}

fn mode(path: &Path) -> u32 {
    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o7777
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> Self {
        let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "razield-private-envelope-{label}-{}-{sequence}-{timestamp}",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn sibling(&self, label: &str) -> PathBuf {
        let file_name = self.path.file_name().unwrap().to_string_lossy();
        self.path
            .parent()
            .unwrap()
            .join(format!("{file_name}-{label}"))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let _ = std::fs::remove_file(&self.path);
            }
            Ok(_) => {
                let _ =
                    std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o700));
                let _ = std::fs::remove_dir_all(&self.path);
            }
            Err(_) => {}
        }
    }
}
