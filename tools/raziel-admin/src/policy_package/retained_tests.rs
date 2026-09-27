use super::*;

#[test]
fn expired_and_revoked_history_is_retained_but_not_approved() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    let key = store.retain(a.input(), NOW).unwrap().key().clone();
    let original = fs::read(f.package_path(&key)).unwrap();
    let expired = store.read_retained(&key, NOW + 5_000).unwrap();
    assert_eq!(expired.source_schema(), a.source);
    assert_eq!(expired.compiler_config(), a.config.as_bytes());
    assert_eq!(expired.policy_pb(), a.policy_bytes);
    assert_eq!(expired.candidate_manifest(), a.manifest_bytes);
    assert_eq!(expired.identity(), &a.identity);
    assert_eq!(expired.key(), &key);
    assert_eq!(expired.policy_hash(), a.manifest.policy_hash);
    assert_eq!(expired.inventory().definitions().len(), 2);
    assert!(expired.inventory().caveats().is_empty());
    assert_eq!(expired.policy().program().definitions.len(), 2);
    assert!(!format!("{expired:?}").contains("definition"));
    assert_eq!(
        store.read_approved(&key, NOW + 5_000).unwrap_err(),
        PackageError::RegistryRejected
    );

    f.publish(&[&a], 2, true);
    let revoked = store.read_retained(&key, NOW + 5_001).unwrap();
    assert_eq!(
        revoked.registry_record().status,
        proto::PolicyStatus::Revoked as i32
    );
    assert_eq!(revoked.registry_record().registry_generation, 2);
    assert_eq!(revoked.registry_record().revoked_at, Some(timestamp(NOW)));
    assert_eq!(
        revoked.approval_record().status,
        proto::PolicyStatus::Approved as i32
    );
    assert_eq!(revoked.approval_record().registry_generation, 1);
    assert_eq!(store.list_retained(NOW + 5_001).unwrap().len(), 1);
    assert_eq!(
        store.read_approved(&key, NOW + 5_001).unwrap_err(),
        PackageError::RegistryRejected
    );
    assert_eq!(fs::read(f.package_path(&key)).unwrap(), original);
}

#[test]
fn enumeration_keeps_superseded_approvals_in_numeric_epoch_order() {
    let f = Fixture::new();
    let approvals: Vec<_> = (1..=10)
        .map(|epoch| Artifacts::new('b', 1, epoch))
        .collect();
    let later = Artifacts::new('c', 2, 1);
    let mut store = f.store();
    let mut keys = Vec::new();
    for epoch in 1..=10 {
        let mut records: Vec<_> = approvals[..epoch].iter().collect();
        records.push(&later);
        f.publish(&records, epoch as u64, false);
        if epoch == 1 {
            store.retain(later.input(), NOW).unwrap();
        }
        keys.push(
            store
                .retain(approvals[epoch - 1].input(), NOW)
                .unwrap()
                .key()
                .clone(),
        );
    }
    let listed = store.list_retained(NOW).unwrap();
    assert_eq!(listed.len(), 11);
    for (index, item) in listed.iter().take(10).enumerate() {
        assert_eq!(item.key(), &keys[index]);
        assert_eq!(item.registry_record().registry_generation, 10);
    }
    assert_eq!(listed[10].identity(), &later.identity);
    assert_eq!(
        store.read_approved(&keys[0], NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
    store.read_approved(&keys[9], NOW).unwrap();
    assert_eq!(
        store.read_retained(&keys[0], NOW).unwrap().source_schema(),
        approvals[0].source
    );
}

#[test]
fn retained_artifact_and_approval_mutations_are_rejected() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    let key = store.retain(a.input(), NOW).unwrap().key().clone();
    let path = f.package_path(&key);
    let original = fs::read(&path).unwrap();
    let mutations: [fn(&mut StoredPackage); 7] = [
        |p| p.source_schema.push(b'\n'),
        |p| p.compiler_config = b"{}".to_vec(),
        |p| p.policy_pb.push(0),
        |p| p.candidate_manifest.push(0),
        |p| p.definitions.push("foreign".into()),
        |p| p.caveats.push("foreign".into()),
        |p| p.version = 2,
    ];
    for mutate in mutations {
        let mut changed = StoredPackage::decode(original.as_slice()).unwrap();
        mutate(&mut changed);
        atomic_write(&path, &changed.encode_to_vec()).unwrap();
        assert!(store.read_retained(&key, NOW + 5_000).is_err());
        assert!(store.list_retained(NOW + 5_000).is_err());
    }
    let record_mutations: [fn(&mut proto::PolicyRecord); 6] = [
        |r| r.authority_signature[0] ^= 1,
        |r| r.approved_by = "another-owner".into(),
        |r| r.registered_at = Some(timestamp(NOW + 1)),
        |r| r.registry_generation = 2,
        |r| r.revocation_reason = "untrusted".into(),
        |r| r.status = proto::PolicyStatus::Revoked as i32,
    ];
    for mutate in record_mutations {
        let mut changed = StoredPackage::decode(original.as_slice()).unwrap();
        let mut record = proto::PolicyRecord::decode(changed.approval_record.as_slice()).unwrap();
        mutate(&mut record);
        changed.approval_record = record.encode_to_vec();
        atomic_write(&path, &changed.encode_to_vec()).unwrap();
        assert!(store.read_retained(&key, NOW + 5_000).is_err());
    }
}

#[test]
fn validly_resigned_different_registry_approval_does_not_replace_retained_history() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let key = f.store().retain(a.input(), NOW).unwrap().key().clone();
    let mut changed = a.clone();
    changed.manifest.valid_until_unix_seconds += 1;
    changed.refresh();
    f.publish(&[&changed], 2, false);
    // A fresh store cannot rely on an in-memory transition history for this check.
    assert_eq!(
        f.store().read_retained(&key, NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
}

#[test]
fn registry_signature_and_missing_history_fail_retained_reads() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let key = f.store().retain(a.input(), NOW).unwrap().key().clone();
    let mut record = proto::PolicyRecord::decode(
        fs::read(f.registry.join("policy-000.pb"))
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    record.authority_signature[0] ^= 1;
    let entry = f.write("policy-000.pb", &record.encode_to_vec());
    let mut manifest = proto::RegistryGenerationManifest::decode(
        fs::read(f.registry.join("current.pb")).unwrap().as_slice(),
    )
    .unwrap();
    manifest.policies = vec![entry];
    atomic_write(&f.registry.join("current.pb"), &manifest.encode_to_vec()).unwrap();
    assert_eq!(
        f.store().read_retained(&key, NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
    f.publish(&[], 2, false);
    assert_eq!(
        f.store().read_retained(&key, NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
}

#[test]
fn retained_listing_rejects_filename_aliases_and_bounds_aggregate_bytes() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    assert!(store.list_retained(NOW).unwrap().is_empty());
    let key = store.retain(a.input(), NOW).unwrap().key().clone();
    let path = f.package_path(&key);
    let alias = f.packages.join(format!("{}.01.pb", key.version_id()));
    fs::rename(&path, &alias).unwrap();
    assert_eq!(
        store.list_retained(NOW).unwrap_err(),
        PackageError::InvalidPackage
    );
    fs::rename(&alias, &path).unwrap();

    let byte_limit = crate::config::registry_config("package-test")
        .unwrap()
        .limits
        .max_manifest_total_bytes;
    // Sparse files test the pre-read aggregate bound without allocating large buffers.
    for index in 0..=byte_limit / MAX_PACKAGE_BYTES {
        let path = f.packages.join(format!("{index:032x}.1.pb"));
        atomic_write(&path, &[]).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_len(MAX_PACKAGE_BYTES)
            .unwrap();
    }
    assert_eq!(
        store.list_retained(NOW).unwrap_err(),
        PackageError::InvalidPackage
    );
}

#[test]
fn retained_listing_bounds_number_of_entries_before_reading_payloads() {
    let f = Fixture::new();
    f.publish(&[], 1, false);
    let limit = crate::config::registry_config("package-test")
        .unwrap()
        .limits
        .max_manifest_files_per_kind;
    for index in 0..=limit {
        // Empty entries would fail decoding. Enumeration must reject the count first.
        let path = f.packages.join(format!("{index:032x}.1.pb"));
        fs::File::create(&path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(
        f.store().list_retained(NOW).unwrap_err(),
        PackageError::InvalidPackage
    );
}
