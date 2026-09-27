#![cfg(unix)]

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, chown, symlink};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Barrier};

use crate::{AdminError, args::Arguments};

use super::{MAX_ALLOCATION_RECORDS, PolicyVersionStore, StoredAllocation, allocate_command};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let base = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        Self {
            root: base.join(format!(
                "raziel-policy-version-{}",
                super::random_id().unwrap()
            )),
        }
    }

    fn store(&self) -> PolicyVersionStore {
        PolicyVersionStore::new(self.root.clone()).unwrap()
    }

    fn path(&self, candidate: &str) -> PathBuf {
        self.root.join(format!("{candidate}.json"))
    }

    fn write(&self, candidate: &str, bytes: &[u8]) {
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(self.path(candidate))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn durable_json_has_exact_contract_and_private_modes() {
    let fixture = Fixture::new();
    let allocation = fixture
        .store()
        .allocate("candidate-001", DIGEST, None)
        .unwrap();
    assert_eq!(allocation.candidate_id(), "candidate-001");
    assert_eq!(allocation.request_digest(), DIGEST);
    assert_eq!(allocation.requested_family_id(), None);
    assert_eq!(allocation.identity().version(), 1);
    assert!(super::lower_hex(allocation.identity().family_id(), 32));
    assert!(super::lower_hex(allocation.identity().version_id(), 32));
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.path("candidate-001")).unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "format_version": 1,
            "candidate_id": "candidate-001",
            "request_digest": DIGEST,
            "requested_family_id": null,
            "family_id": allocation.identity().family_id(),
            "version_id": allocation.identity().version_id(),
            "version": 1
        })
    );
    assert_eq!(
        std::fs::metadata(&fixture.root)
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o700
    );
    let metadata = std::fs::metadata(fixture.path("candidate-001")).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
    assert_eq!(metadata.nlink(), 1);
}

#[test]
fn read_only_open_preserves_directory_and_rejects_allocations() {
    let fixture = Fixture::new();
    assert!(PolicyVersionStore::open_read_only(fixture.root.clone()).is_err());
    assert!(!fixture.root.exists());
    let expected = fixture.store().allocate("initial", DIGEST, None).unwrap();
    let before = std::fs::metadata(&fixture.root).unwrap();
    let reader = PolicyVersionStore::open_read_only(fixture.root.clone()).unwrap();
    assert_eq!(reader.read("initial").unwrap(), expected);
    assert!(reader.allocate("initial", DIGEST, None).is_err());
    assert!(reader.allocate("new", DIGEST, None).is_err());
    let after = std::fs::metadata(&fixture.root).unwrap();
    assert_eq!(
        (before.mtime(), before.mtime_nsec()),
        (after.mtime(), after.mtime_nsec())
    );
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 1);
    std::fs::set_permissions(&fixture.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(PolicyVersionStore::open_read_only(fixture.root.clone()).is_err());
    let linked = Fixture::new();
    symlink(&fixture.root, &linked.root).unwrap();
    assert!(PolicyVersionStore::open_read_only(linked.root.clone()).is_err());
}

#[test]
fn exact_retries_survive_restart_without_overwrite() {
    let fixture = Fixture::new();
    let original = fixture.store().allocate("initial", DIGEST, None).unwrap();
    let metadata = std::fs::metadata(fixture.path("initial")).unwrap();
    let bytes = std::fs::read(fixture.path("initial")).unwrap();
    let restarted = fixture.store();
    assert_eq!(restarted.read("initial").unwrap(), original);
    assert_eq!(
        restarted.allocate("initial", DIGEST, None).unwrap(),
        original
    );
    assert!(matches!(
        restarted.allocate("initial", &"f".repeat(64), None),
        Err(AdminError::InvalidCandidate)
    ));
    assert!(matches!(
        restarted.allocate("initial", DIGEST, Some(original.identity().family_id())),
        Err(AdminError::InvalidCandidate)
    ));
    assert_eq!(std::fs::read(fixture.path("initial")).unwrap(), bytes);
    assert_eq!(
        std::fs::metadata(fixture.path("initial")).unwrap().ino(),
        metadata.ino()
    );
}

#[test]
fn family_numbers_continue_after_abandoned_drafts_and_restart() {
    let fixture = Fixture::new();
    let original = fixture
        .store()
        .allocate("abandoned-first", DIGEST, None)
        .unwrap();
    let family = original.identity().family_id();
    let second = fixture
        .store()
        .allocate("abandoned-second", DIGEST, Some(family))
        .unwrap();
    let third = fixture
        .store()
        .allocate("compiled-third", DIGEST, Some(family))
        .unwrap();
    assert_eq!(second.identity().version(), 2);
    assert_eq!(third.identity().version(), 3);
    assert_ne!(
        second.identity().version_id(),
        third.identity().version_id()
    );
    assert_eq!(
        fixture
            .store()
            .allocate("abandoned-second", DIGEST, Some(family))
            .unwrap(),
        second
    );
    assert!(
        fixture
            .store()
            .allocate("abandoned-second", DIGEST, None)
            .is_err()
    );
    let another = fixture
        .store()
        .allocate("new-family", DIGEST, None)
        .unwrap();
    assert_eq!(another.identity().version(), 1);
    assert_ne!(another.identity().family_id(), family);
}

#[test]
fn unknown_family_and_invalid_requests_create_no_record() {
    let fixture = Fixture::new();
    let store = fixture.store();
    assert!(
        store
            .allocate("unknown", DIGEST, Some(&"f".repeat(32)))
            .is_err()
    );
    for candidate in ["", "../escape", ".hidden", "a/b", "a\\b", "a\0b", "a b"] {
        assert!(store.allocate(candidate, DIGEST, None).is_err());
        assert!(store.read(candidate).is_err());
    }
    assert!(store.allocate(&"a".repeat(129), DIGEST, None).is_err());
    for digest in ["f".repeat(63), "F".repeat(64), "g".repeat(64)] {
        assert!(store.allocate("invalid", &digest, None).is_err());
    }
    assert!(
        store
            .allocate("invalid", DIGEST, Some(&"F".repeat(32)))
            .is_err()
    );
    assert!(store.read("missing").is_err());
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[test]
fn concurrent_retries_return_one_identity_and_one_file() {
    let fixture = Fixture::new();
    fixture.store();
    let barrier = Arc::new(Barrier::new(12));
    let handles: Vec<_> = (0..12)
        .map(|_| {
            let root = fixture.root.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                PolicyVersionStore::new(root)
                    .unwrap()
                    .allocate("same", DIGEST, None)
                    .unwrap()
            })
        })
        .collect();
    let records: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(records.iter().all(|r| r == &records[0]));
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 1);
}

#[test]
fn concurrent_processes_allocate_monotonic_distinct_versions() {
    let fixture = Fixture::new();
    let first = fixture.store().allocate("first", DIGEST, None).unwrap();
    let executable = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for index in 0..8 {
        children.push(
            Command::new(&executable)
                .args([
                    "--exact",
                    "policy_version::tests::allocation_child_process",
                    "--nocapture",
                ])
                .env("RAZIEL_ALLOCATION_TEST_ROOT", &fixture.root)
                .env(
                    "RAZIEL_ALLOCATION_TEST_FAMILY",
                    first.identity().family_id(),
                )
                .env("RAZIEL_ALLOCATION_TEST_CANDIDATE", format!("child-{index}"))
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let store = fixture.store();
    let allocations: Vec<_> = (0..8)
        .map(|i| store.read(&format!("child-{i}")).unwrap())
        .collect();
    assert_eq!(
        allocations
            .iter()
            .map(|a| a.identity().version())
            .collect::<BTreeSet<_>>(),
        (2..=9).collect()
    );
    assert_eq!(
        allocations
            .iter()
            .map(|a| a.identity().version_id())
            .collect::<BTreeSet<_>>()
            .len(),
        8
    );
}

#[test]
fn allocation_child_process() {
    let Some(root) = std::env::var_os("RAZIEL_ALLOCATION_TEST_ROOT") else {
        return;
    };
    let family = std::env::var("RAZIEL_ALLOCATION_TEST_FAMILY").unwrap();
    let candidate = std::env::var("RAZIEL_ALLOCATION_TEST_CANDIDATE").unwrap();
    PolicyVersionStore::new(root.into())
        .unwrap()
        .allocate(&candidate, DIGEST, Some(&family))
        .unwrap();
}

#[test]
fn malformed_or_mutated_records_are_never_overwritten() {
    for mutation in [
        "missing",
        "extra",
        "version-zero",
        "version-overflow",
        "format",
        "candidate",
        "family",
        "digest",
        "duplicate",
    ] {
        let fixture = Fixture::new();
        let store = fixture.store();
        store.allocate("candidate", DIGEST, None).unwrap();
        let path = fixture.path("candidate");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        match mutation {
            "missing" => {
                json.as_object_mut().unwrap().remove("requested_family_id");
            }
            "extra" => json["unapproved_field"] = true.into(),
            "version-zero" => json["version"] = 0.into(),
            "version-overflow" => json["version"] = (i32::MAX as u64 + 1).into(),
            "format" => json["format_version"] = 2.into(),
            "candidate" => json["candidate_id"] = "different".into(),
            "family" => json["family_id"] = "bad".into(),
            "digest" => json["request_digest"] = "bad".into(),
            _ => {}
        }
        let bytes = if mutation == "duplicate" {
            format!(
                "{{\"version\":1,{}",
                &serde_json::to_string(&json).unwrap()[1..]
            )
            .into_bytes()
        } else {
            serde_json::to_vec(&json).unwrap()
        };
        std::fs::write(&path, &bytes).unwrap();
        assert!(store.read("candidate").is_err(), "{mutation}");
        assert!(
            store.allocate("candidate", DIGEST, None).is_err(),
            "{mutation}"
        );
        assert!(
            store.allocate("another", DIGEST, None).is_err(),
            "{mutation}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn count_limit_allows_retry_but_not_a_new_allocation() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let first = store.allocate("first", DIGEST, None).unwrap();
    for index in 1..MAX_ALLOCATION_RECORDS {
        let candidate = format!("seed-{index}");
        fixture.write(
            &candidate,
            &serde_json::to_vec(&StoredAllocation {
                format_version: 1,
                candidate_id: candidate.clone(),
                request_digest: DIGEST.into(),
                requested_family_id: Some(first.identity().family_id().into()),
                family_id: first.identity().family_id().into(),
                version_id: format!("{index:032x}"),
                version: index as u32 + 1,
            })
            .unwrap(),
        );
    }
    assert_eq!(store.allocate("first", DIGEST, None).unwrap(), first);
    assert!(
        store
            .allocate("overflow", DIGEST, Some(first.identity().family_id()))
            .is_err()
    );
    assert!(!fixture.path("overflow").exists());
}

#[test]
fn version_overflow_and_duplicate_version_identity_fail_closed() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let first = store.allocate("first", DIGEST, None).unwrap();
    let family = first.identity().family_id();
    fixture.write(
        "last",
        &serde_json::to_vec(&StoredAllocation {
            format_version: 1,
            candidate_id: "last".into(),
            request_digest: DIGEST.into(),
            requested_family_id: Some(family.into()),
            family_id: family.into(),
            version_id: "f".repeat(32),
            version: i32::MAX as u32,
        })
        .unwrap(),
    );
    assert!(store.allocate("too-late", DIGEST, Some(family)).is_err());
    assert_eq!(
        store.read("last").unwrap().identity().version(),
        i32::MAX as u32
    );
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.path("last")).unwrap()).unwrap();
    json["version_id"] = first.identity().version_id().into();
    std::fs::write(fixture.path("last"), serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(store.allocate("new-family", DIGEST, None).is_err());
}

#[test]
fn private_paths_reject_symlinks_hardlinks_modes_and_directory_replacement() {
    assert!(PolicyVersionStore::new(PathBuf::from("relative")).is_err());
    assert!(PolicyVersionStore::new(PathBuf::from("/")).is_err());
    let fixture = Fixture::new();
    let store = fixture.store();
    store.allocate("first", DIGEST, None).unwrap();
    let path = fixture.path("first");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.read("first").is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::hard_link(&path, fixture.path("hardlink")).unwrap();
    assert!(store.read("first").is_err());
    std::fs::remove_file(fixture.path("hardlink")).unwrap();
    symlink(&path, fixture.path("link")).unwrap();
    assert!(store.read("link").is_err());
    assert!(store.allocate("new", DIGEST, None).is_err());
    std::fs::remove_file(fixture.path("link")).unwrap();
    std::fs::set_permissions(&fixture.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(store.read("first").is_err());
    assert!(PolicyVersionStore::new(fixture.root.clone()).is_err());
    std::fs::set_permissions(&fixture.root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let alias = Fixture::new();
    symlink(&fixture.root, &alias.root).unwrap();
    assert!(PolicyVersionStore::new(alias.root.clone()).is_err());
    assert!(PolicyVersionStore::new(alias.root.join("child")).is_err());
    std::fs::remove_file(&alias.root).unwrap();
    let replaced = Fixture::new();
    std::fs::rename(&fixture.root, &replaced.root).unwrap();
    fixture.store();
    assert!(store.allocate("wrong-root", DIGEST, None).is_err());
}

#[test]
fn wrong_owner_root_and_records_are_rejected() {
    let fixture = Fixture::new();
    let store = fixture.store();
    store.allocate("first", DIGEST, None).unwrap();
    // The pinned Docker toolchain runs these owner-mutation checks as root.
    if std::fs::metadata(&fixture.root).unwrap().uid() != 0 {
        return;
    }
    chown(fixture.path("first"), Some(1), None).unwrap();
    assert!(store.read("first").is_err());
    assert!(store.allocate("next", DIGEST, None).is_err());
    chown(fixture.path("first"), Some(0), None).unwrap();
    chown(&fixture.root, Some(1), None).unwrap();
    assert!(store.read("first").is_err());
    assert!(PolicyVersionStore::new(fixture.root.clone()).is_err());
    chown(&fixture.root, Some(0), None).unwrap();
}

#[test]
fn oversized_and_nonregular_records_are_rejected() {
    let fixture = Fixture::new();
    let store = fixture.store();
    fixture.write(
        "large",
        &vec![b' '; super::MAX_ALLOCATION_BYTES as usize + 1],
    );
    assert!(store.read("large").is_err());
    std::fs::create_dir(fixture.path("directory")).unwrap();
    assert!(store.read("directory").is_err());
    assert!(store.allocate("new", DIGEST, None).is_err());
}

#[test]
fn cli_rejects_unknown_duplicate_or_missing_options() {
    let fixture = Fixture::new();
    let args = vec![
        "allocate-policy-version".into(),
        "--root".into(),
        fixture.root.to_string_lossy().into_owned(),
        "--candidate-id".into(),
        "cli-first".into(),
        "--request-digest".into(),
        DIGEST.into(),
    ];
    allocate_command(&Arguments::parse(args.clone()).unwrap()).unwrap();
    let first = fixture.store().read("cli-first").unwrap();
    let mut family_args = args.clone();
    family_args[4] = "cli-second".into();
    family_args.extend(["--family-id".into(), first.identity().family_id().into()]);
    allocate_command(&Arguments::parse(family_args).unwrap()).unwrap();
    assert_eq!(
        fixture
            .store()
            .read("cli-second")
            .unwrap()
            .identity()
            .version(),
        2
    );
    for flag in ["--version", "--family-id", "--candidate-id"] {
        let mut invalid = args.clone();
        invalid.extend([flag.into(), "1".into()]);
        assert!(allocate_command(&Arguments::parse(invalid).unwrap()).is_err());
    }
    assert!(
        allocate_command(&Arguments::parse(["allocate-policy-version".into()]).unwrap()).is_err()
    );
    let mut repeated_family = args;
    repeated_family.extend([
        "--family-id".into(),
        first.identity().family_id().into(),
        "--family-id".into(),
        first.identity().family_id().into(),
    ]);
    assert!(allocate_command(&Arguments::parse(repeated_family).unwrap()).is_err());
    File::open(fixture.path("cli-first"))
        .unwrap()
        .sync_all()
        .unwrap();
}
