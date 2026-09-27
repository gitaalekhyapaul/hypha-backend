use super::{BindingError, DatasetBinding, DatasetBindingStore};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "raziel-binding-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        Self(root)
    }
    fn store(&self) -> DatasetBindingStore {
        DatasetBindingStore::open(self.0.clone()).unwrap()
    }
    fn path(&self, binding: &DatasetBinding) -> PathBuf {
        self.0
            .join(&binding.policy_hash)
            .join(&binding.resource_type)
            .join(format!("{}.json", binding.resource_scope))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn binding() -> DatasetBinding {
    serde_json::from_str(include_str!(
        "../../../../tests/vectors/dataset-binding-v1.json"
    ))
    .unwrap()
}

#[test]
fn busy_mapping_store_returns_unavailable_without_waiting() {
    let fixture = Fixture::new();
    let lock = std::fs::File::open(&fixture.0).unwrap();
    lock.lock().unwrap();
    assert_eq!(
        fixture.store().retain(&binding()),
        Err(BindingError::Unavailable)
    );
}

#[test]
fn immutable_mapping_round_trips_and_exact_retries_do_not_rewrite() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let value = binding();
    store.retain(&value).unwrap();
    store.retain(&value).unwrap();
    assert_eq!(
        store
            .read(
                &value.policy_hash,
                &value.resource_type,
                &value.resource_scope
            )
            .unwrap(),
        value
    );
    assert_eq!(
        std::fs::metadata(fixture.path(&value))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    for altered in [
        DatasetBinding {
            dataset_id: "bank-ledger".into(),
            ..value.clone()
        },
        DatasetBinding {
            dataset_revision: "2".into(),
            ..value.clone()
        },
        DatasetBinding {
            dataset_checksum: "44".repeat(32),
            ..value.clone()
        },
    ] {
        assert_eq!(store.retain(&altered), Err(BindingError::Conflict));
    }
    let other_policy = DatasetBinding {
        policy_hash: "55".repeat(32),
        ..value.clone()
    };
    store.retain(&other_policy).unwrap();
}

#[test]
fn invalid_keys_revisions_and_unknown_fields_fail_closed() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let value = binding();
    for scope in ["../scope", "/scope", "x/y", ".", ""] {
        assert_eq!(
            store.retain(&DatasetBinding {
                resource_scope: scope.into(),
                ..value.clone()
            }),
            Err(BindingError::Invalid)
        );
    }
    for revision in ["0", "01", "-1", "9007199254740992"] {
        assert_eq!(
            store.retain(&DatasetBinding {
                dataset_revision: revision.into(),
                ..value.clone()
            }),
            Err(BindingError::Invalid)
        );
    }
    assert_eq!(
        store.retain(&DatasetBinding {
            action: "export".into(),
            ..value.clone()
        }),
        Err(BindingError::Invalid)
    );
    store.retain(&value).unwrap();
    let mut tampered = serde_json::to_value(&value).unwrap();
    tampered["witness"] = serde_json::json!({});
    std::fs::write(fixture.path(&value), serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert_eq!(
        store.read(
            &value.policy_hash,
            &value.resource_type,
            &value.resource_scope
        ),
        Err(BindingError::Invalid)
    );
}

#[test]
fn missing_symlink_and_permissive_records_are_rejected() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let value = binding();
    assert_eq!(
        store.read(
            &value.policy_hash,
            &value.resource_type,
            &value.resource_scope
        ),
        Err(BindingError::NotFound)
    );
    store.retain(&value).unwrap();
    let path = fixture.path(&value);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        store.read(
            &value.policy_hash,
            &value.resource_type,
            &value.resource_scope
        ),
        Err(BindingError::Invalid)
    );
    let real = path.with_extension("original");
    std::fs::rename(&path, &real).unwrap();
    symlink(&real, &path).unwrap();
    assert_eq!(
        store.read(
            &value.policy_hash,
            &value.resource_type,
            &value.resource_scope
        ),
        Err(BindingError::Invalid)
    );
}
