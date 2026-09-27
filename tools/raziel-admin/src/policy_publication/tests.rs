// Run these ignored tests with the pinned toolchain and `--cap-drop ALL`.
// The real authority rejects a key file that the process can open for writing.
use std::fs;

use crate::policy_activation::store::{StoreActivationError, trusted_now};
use crate::policy_package::PolicyPackageKey;

#[path = "fixtures.rs"]
mod fixtures;

use fixtures::{Fixture, request};

#[test]
#[ignore = "requires RAZIEL_POLICY_PUBLICATION_TEST_COMPILER and built authority/writer binaries"]
fn production_publication_preserves_versions_and_retries() {
    let fixture = Fixture::new();
    let first = fixture.prepare("candidate-one", None, false);
    let second = fixture.prepare("candidate-two", Some(first.identity().family_id()), true);
    assert_eq!(first.identity().version(), 1);
    assert_eq!(second.identity().version(), 2);
    assert_ne!(
        first.identity().version_id(),
        second.identity().version_id()
    );
    let publisher = fixture.publisher();

    let first_result = publisher.publish(request(first.candidate_id())).unwrap();
    assert_eq!(first_result.registry_generation, 2);
    let first_schema = fixture.schema_bytes();
    fixture.assert_matches_candidates(&[&first]);
    let first_key = PolicyPackageKey::new(first.identity().version_id().into(), 1).unwrap();
    let first_package_bytes = fixture.package_bytes(&first_key);
    let installed = fixture.installed_bytes();

    let retry = publisher.publish(request(first.candidate_id())).unwrap();
    assert_eq!(retry, first_result);
    assert!(
        fixture
            .controlled_write_count()
            .is_none_or(|count| count == 1)
    );
    assert_eq!(fixture.installed_bytes(), installed);

    let second_result = publisher.publish(request(second.candidate_id())).unwrap();
    assert_eq!(second_result.registry_generation, 3);
    assert_ne!(first_result.policy_hash, second_result.policy_hash);
    assert!(
        fixture
            .controlled_write_count()
            .is_none_or(|count| count == 2)
    );
    assert_ne!(fixture.schema_bytes(), first_schema);
    fixture.assert_matches_candidates(&[&first, &second]);
    assert_eq!(fixture.package_bytes(&first_key), first_package_bytes);
    let mut packages = fixture.packages();
    let retained_first = packages
        .read_approved(&first_key, trusted_now().unwrap())
        .unwrap();
    assert_eq!(retained_first.identity(), first.identity());
    assert_eq!(retained_first.approval_record().registry_generation, 2);
    assert_eq!(fixture.registry().policies.len(), 2);
    assert!(fixture.registry().accepted_roots.is_empty());

    // A new publisher reloads installed state. Retry cannot publish another generation.
    let installed = fixture.installed_bytes();
    let registry_bytes = fixture.registry_bytes();
    assert_eq!(
        fixture
            .publisher()
            .publish(request(second.candidate_id()))
            .unwrap(),
        second_result,
    );
    assert_eq!(fixture.installed_bytes(), installed);
    assert_eq!(fixture.registry_bytes(), registry_bytes);
    assert!(
        fixture
            .controlled_write_count()
            .is_none_or(|count| count == 2)
    );
    assert!(!fixture.pending_path().exists());
}

#[test]
#[ignore = "requires RAZIEL_POLICY_PUBLICATION_TEST_COMPILER and built authority/writer binaries"]
fn production_publication_rejects_schema_drift_without_overwrite() {
    let fixture = Fixture::new_controlled();
    let candidate = fixture.prepare("candidate-drift", None, false);
    let publisher = fixture.publisher();
    publisher
        .publish(request(candidate.candidate_id()))
        .unwrap();
    let installed = fixture.installed_bytes();
    let registry_bytes = fixture.registry_bytes();

    let mut drift = fixture.schema().bytes();
    drift.extend_from_slice(b"\ndefinition unexpected_owner_change {}\n");
    fixture.schema().replace(drift.clone());
    assert_eq!(
        publisher.publish(request(candidate.candidate_id())),
        Err(StoreActivationError::Conflict),
    );
    assert_eq!(fixture.schema().bytes(), drift);
    assert_eq!(fixture.schema().writes(), 1);
    assert_eq!(fixture.registry_bytes(), registry_bytes);
    assert_eq!(fixture.installed_bytes(), installed);
    assert!(!fixture.pending_path().exists());
    assert_eq!(fixture.schema().unexpected_calls(), 0);
}

#[test]
#[ignore = "requires RAZIEL_POLICY_PUBLICATION_TEST_COMPILER and built authority/writer binaries"]
fn production_publication_reconciles_applied_write_after_lost_response() {
    let fixture = Fixture::new_controlled();
    let candidate = fixture.prepare("candidate-lost-response", None, false);
    let publisher = fixture.publisher();
    fixture.schema().lose_next_write_response();

    assert_eq!(
        publisher.publish(request(candidate.candidate_id())),
        Err(StoreActivationError::Unavailable),
    );
    assert_eq!(fixture.schema().writes(), 1);
    assert!(fixture.pending_path().is_file());
    assert!(!fixture.installed_path().exists());
    fixture.assert_matches_candidates(&[&candidate]);
    let registry_before_retry = fixture.registry_bytes();
    let key = PolicyPackageKey::new(candidate.identity().version_id().into(), 1).unwrap();
    let retained_before_retry = fixture.package_bytes(&key);

    let result = publisher
        .publish(request(candidate.candidate_id()))
        .unwrap();
    assert_eq!(result.registry_generation, 2);
    assert_eq!(fixture.schema().writes(), 1);
    assert!(!fixture.pending_path().exists());
    assert!(fixture.installed_path().is_file());
    assert_eq!(fixture.registry_bytes(), registry_before_retry);
    assert_eq!(fixture.package_bytes(&key), retained_before_retry);
    fixture
        .packages()
        .read_approved(&key, trusted_now().unwrap())
        .unwrap();
    assert!(fixture.registry().accepted_roots.is_empty());
    assert_eq!(fixture.schema().unexpected_calls(), 0);

    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.installed_path()).unwrap()).unwrap();
    assert_eq!(installed["generation"], 1);
}
