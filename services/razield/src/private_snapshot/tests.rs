use std::fs::OpenOptions;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValue, CaveatValueType, ObjectDefinition, PermissionDefinition, PermissionExpr,
    PolicyProgram, RelationDefinition, ValidatedPolicyProgram,
};
use raziel_snapshot::{RelationshipRecord, SnapshotTree, StoredCaveat, StoredContextEntry};

use super::codec;
use super::wire::PrivateRelationshipSnapshotWireV1;
use super::{
    MAX_PRIVATE_SNAPSHOT_BYTES, PRIVATE_SNAPSHOT_FORMAT_VERSION, PrivateRelationshipSnapshot,
    PrivateSnapshotError, PrivateSnapshotStore,
};
use raziel_authorization_core::{TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS};

const SNAPSHOT_ID: &str = "snapshot-private-test";
const PRIVATE_VALUE: &str = "synthetic-private-region";
const SALT: [u8; 32] = [0x5a; 32];

static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn round_trip_rebuilds_the_root_and_preserves_typed_private_context() {
    let root = TestRoot::new("round-trip");
    let policy = policy();
    let relationships = relationships();
    let relationship_root = SnapshotTree::build(
        &policy,
        relationships.clone(),
        &SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .unwrap()
    .root();
    let snapshot = PrivateRelationshipSnapshot::new(
        &policy,
        SNAPSHOT_ID.into(),
        relationship_root,
        SALT,
        relationships,
    )
    .unwrap();
    let store = PrivateSnapshotStore::open(root.path()).unwrap();

    store.publish(&snapshot).unwrap();
    let loaded = store
        .load(&policy, SNAPSHOT_ID, &relationship_root)
        .unwrap();

    assert_eq!(loaded, snapshot);
    let path = store.final_path_for_test(&relationship_root);
    let bytes = std::fs::read(&path).unwrap();
    assert!(contains_bytes(&bytes, PRIVATE_VALUE.as_bytes()));
    assert_eq!(mode(root.path()), 0o700);
    assert_eq!(mode(&path), 0o600);
    assert_eq!(
        path.file_name().unwrap().to_string_lossy().len(),
        64 + ".snapshot.pb".len()
    );
    assert!(!format!("{snapshot:?}{store:?}").contains(PRIVATE_VALUE));
}

#[test]
fn strict_decode_rejects_unknown_fields_versions_and_registry_binding_changes() {
    let policy = policy();
    let snapshot = snapshot(&policy);
    let baseline = codec::encode(&snapshot).unwrap();

    let mut unknown = baseline.clone();
    unknown.extend_from_slice(&[0x78, 0x01]);
    assert_eq!(
        codec::decode(&policy, SNAPSHOT_ID, snapshot.relationship_root(), &unknown),
        Err(PrivateSnapshotError::MalformedSnapshot)
    );

    let mut wire = PrivateRelationshipSnapshotWireV1::decode(baseline.as_slice()).unwrap();
    assert!(!format!("{wire:?}").contains(PRIVATE_VALUE));
    wire.format_version = PRIVATE_SNAPSHOT_FORMAT_VERSION + 1;
    assert_eq!(
        codec::decode(
            &policy,
            SNAPSHOT_ID,
            snapshot.relationship_root(),
            &wire.encode_to_vec()
        ),
        Err(PrivateSnapshotError::UnsupportedSnapshotVersion)
    );
    assert_eq!(
        codec::decode(
            &policy,
            "another-snapshot",
            snapshot.relationship_root(),
            &baseline
        ),
        Err(PrivateSnapshotError::SnapshotIdMismatch)
    );
    assert_eq!(
        codec::decode(&policy, SNAPSHOT_ID, &[0x33; 32], &baseline),
        Err(PrivateSnapshotError::RootMismatch)
    );
}

#[test]
fn constructor_rejects_root_changes_duplicates_and_unordered_relationships() {
    let policy = policy();
    let relationships = relationships();
    let root = SnapshotTree::build(
        &policy,
        relationships.clone(),
        &SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .unwrap()
    .root();
    assert_eq!(
        PrivateRelationshipSnapshot::new(
            &policy,
            SNAPSHOT_ID.into(),
            [0x44; 32],
            SALT,
            relationships.clone()
        ),
        Err(PrivateSnapshotError::RootMismatch)
    );

    let mut duplicate = relationships.clone();
    duplicate.push(relationships[0].clone());
    assert_eq!(
        PrivateRelationshipSnapshot::new(&policy, SNAPSHOT_ID.into(), root, SALT, duplicate),
        Err(PrivateSnapshotError::InvalidSnapshot)
    );

    let mut unordered = relationships;
    unordered.swap(0, 1);
    assert_eq!(
        PrivateRelationshipSnapshot::new(&policy, SNAPSHOT_ID.into(), root, SALT, unordered),
        Err(PrivateSnapshotError::InvalidSnapshot)
    );
}

#[test]
fn loader_rejects_unsafe_files_and_the_transport_ceiling() {
    let root = TestRoot::new("unsafe-files");
    let policy = policy();
    let snapshot = snapshot(&policy);
    let store = PrivateSnapshotStore::open(root.path()).unwrap();
    store.publish(&snapshot).unwrap();
    let path = store.final_path_for_test(snapshot.relationship_root());

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(
        store.load(&policy, SNAPSHOT_ID, snapshot.relationship_root()),
        Err(PrivateSnapshotError::UnsafeSnapshotFile)
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let hard_link = root.path().join("extra-link");
    std::fs::hard_link(&path, &hard_link).unwrap();
    assert_eq!(
        store.load(&policy, SNAPSHOT_ID, snapshot.relationship_root()),
        Err(PrivateSnapshotError::UnsafeSnapshotFile)
    );
    std::fs::remove_file(hard_link).unwrap();

    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len((MAX_PRIVATE_SNAPSHOT_BYTES + 1) as u64)
        .unwrap();
    assert_eq!(
        store.load(&policy, SNAPSHOT_ID, snapshot.relationship_root()),
        Err(PrivateSnapshotError::SnapshotTooLarge)
    );

    std::fs::remove_file(&path).unwrap();
    let outside = root.sibling("outside-private-snapshot");
    std::fs::write(&outside, b"outside").unwrap();
    symlink(&outside, &path).unwrap();
    assert_eq!(
        store.load(&policy, SNAPSHOT_ID, snapshot.relationship_root()),
        Err(PrivateSnapshotError::UnsafeSnapshotFile)
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(outside).unwrap();
}

#[test]
fn immutable_publication_never_replaces_an_existing_root() {
    let root = TestRoot::new("immutable");
    let policy = policy();
    let snapshot = snapshot(&policy);
    let store = PrivateSnapshotStore::open(root.path()).unwrap();

    store.publish(&snapshot).unwrap();
    assert_eq!(
        store.publish(&snapshot),
        Err(PrivateSnapshotError::SnapshotAlreadyExists)
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

fn snapshot(policy: &ValidatedPolicyProgram) -> PrivateRelationshipSnapshot {
    let relationships = relationships();
    let root = SnapshotTree::build(
        policy,
        relationships.clone(),
        &SALT,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .unwrap()
    .root();
    PrivateRelationshipSnapshot::new(policy, SNAPSHOT_ID.into(), root, SALT, relationships).unwrap()
}

fn relationships() -> Vec<RelationshipRecord> {
    vec![
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "private-payments-a".into(),
            relation: "caveated".into(),
            subject_type: "agent".into(),
            subject_id: "agent-a".into(),
            subject_relation: None,
            stored_caveat: Some(StoredCaveat {
                name: "access_rule".into(),
                context: vec![
                    StoredContextEntry {
                        name: "approved".into(),
                        value: CaveatValue::Bool(true),
                    },
                    StoredContextEntry {
                        name: "limit".into(),
                        value: CaveatValue::Int(500),
                    },
                    StoredContextEntry {
                        name: "region".into(),
                        value: CaveatValue::String(PRIVATE_VALUE.into()),
                    },
                ],
            }),
        },
        RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "private-payments-b".into(),
            relation: "direct".into(),
            subject_type: "agent".into(),
            subject_id: "agent-b".into(),
            subject_relation: None,
            stored_caveat: None,
        },
    ]
}

fn policy() -> ValidatedPolicyProgram {
    let limits = TRUSTED_POLICY_CEILINGS.policy_limits;
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "private-snapshot-test".into(),
            compiler_version: "1".into(),
            source_schema_digest: [0x42; 32],
            definitions: vec![
                ObjectDefinition {
                    name: "agent".into(),
                    relations: vec![],
                },
                ObjectDefinition {
                    name: "dataset".into(),
                    relations: vec![
                        RelationDefinition {
                            name: "caveated".into(),
                            allowed_subject_types: vec![AllowedSubjectType {
                                object_type: "agent".into(),
                                subject_relation: None,
                                caveat_name: Some("access_rule".into()),
                            }],
                        },
                        RelationDefinition {
                            name: "direct".into(),
                            allowed_subject_types: vec![AllowedSubjectType {
                                object_type: "agent".into(),
                                subject_relation: None,
                                caveat_name: None,
                            }],
                        },
                    ],
                },
            ],
            permissions: vec![PermissionDefinition {
                object_type: "dataset".into(),
                name: "access".into(),
                expression: PermissionExpr::RelationRef {
                    name: "direct".into(),
                },
            }],
            caveats: vec![CaveatDefinition {
                name: "access_rule".into(),
                parameters: vec![
                    CaveatParameter {
                        source: CaveatInputSource::RelationshipContext,
                        name: "approved".into(),
                        value_type: CaveatValueType::Bool,
                    },
                    CaveatParameter {
                        source: CaveatInputSource::RelationshipContext,
                        name: "limit".into(),
                        value_type: CaveatValueType::Int,
                    },
                    CaveatParameter {
                        source: CaveatInputSource::RelationshipContext,
                        name: "region".into(),
                        value_type: CaveatValueType::String,
                    },
                ],
                expression: CaveatExpr::ConstBool(true),
            }],
            limits,
        },
        &TRUSTED_POLICY_CEILINGS,
    )
    .unwrap()
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
        .any(|window| window == needle)
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
        Self {
            path: std::env::temp_dir().join(format!(
                "razield-private-snapshot-{label}-{}-{timestamp}-{sequence}",
                std::process::id()
            )),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn sibling(&self, label: &str) -> PathBuf {
        self.path.with_file_name(format!(
            "razield-private-snapshot-{label}-{}",
            std::process::id()
        ))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
