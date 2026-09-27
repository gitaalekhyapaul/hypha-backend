use std::future::ready;

use prost::Message;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, PolicyApprovalRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::decode_policy_program_strict;
use raziel_verifier::{PolicyRecordStatus, PolicyRegistryRecordV1, root_record_from_proto};

use super::*;

const NOW: i64 = 1_700_000_000;
const DOMAIN: &str = "raziel-snapshot-confirmation-test";
const POLICY_BYTES: &[u8] = include_bytes!(
    "../../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);

#[test]
fn append_confirmation_accepts_a_newer_unrelated_policy_publication() {
    let fixture = Fixture::new();
    let appended = fixture.appended(2);
    let mut confirmed = advance(appended.clone(), 3);
    confirmed.policies.push(policy_record("unrelated", 1, 3));
    let result = confirm_snapshot_mutation(
        &mut fixture.registry(),
        confirmed,
        NOW,
        &fixture.descriptor,
        proto::SnapshotStatus::Accepted,
        written(&appended),
    )
    .unwrap();
    assert_eq!(result.registry_generation, 3);
    assert_eq!(result.descriptor.as_ref(), Some(&fixture.descriptor));
}

#[test]
fn append_confirmation_rejects_older_missing_changed_or_revoked_roots() {
    let fixture = Fixture::new();
    let appended = fixture.appended(2);
    let mut cases = vec![advance(appended.clone(), 1)];
    let mut missing = advance(appended.clone(), 3);
    missing.accepted_roots.clear();
    cases.push(missing);
    let mut changed = advance(appended.clone(), 3);
    changed.accepted_roots[0].registered_at_unix_seconds += 1;
    cases.push(changed);
    let mut changed_descriptor = advance(appended.clone(), 3);
    let mut claims = changed_descriptor.accepted_roots[0].claims.claims().clone();
    claims.spicedb_revision = "different-revision".into();
    let claims = ValidatedSnapshotDescriptorClaimsV1::new(claims, snapshot_limits()).unwrap();
    changed_descriptor.accepted_roots[0].authority_signature =
        snapshot_signer().sign_snapshot_descriptor(&claims).unwrap();
    changed_descriptor.accepted_roots[0].claims = claims;
    cases.push(changed_descriptor);
    cases.push(revoke_root(advance(appended.clone(), 3)));

    for confirmed in cases {
        assert_eq!(
            confirm_snapshot_mutation(
                &mut fixture.registry(),
                confirmed,
                NOW,
                &fixture.descriptor,
                proto::SnapshotStatus::Accepted,
                written(&appended),
            ),
            Err(SnapshotOperationError::RegistryConfirmation)
        );
    }
}

#[test]
fn append_confirmation_rechecks_revoked_superseded_and_changed_bound_policies() {
    let fixture = Fixture::new();
    let appended = fixture.appended(2);
    let mut cases = vec![revoke_policy(advance(appended.clone(), 3))];
    let mut superseded = advance(appended.clone(), 3);
    superseded.policies.push(policy_record("bound", 2, 3));
    cases.push(superseded);
    let mut changed = advance(appended.clone(), 3);
    let mut claims = changed.policies[0].claims.claims().clone();
    claims.approved_by = "changed-approver".into();
    let claims = ValidatedPolicyApprovalClaimsV1::new(claims, policy_limits()).unwrap();
    changed.policies[0].authority_signature =
        policy_signer().sign_policy_approval(&claims).unwrap();
    changed.policies[0].claims = claims;
    cases.push(changed);

    for confirmed in cases {
        assert_eq!(
            confirm_snapshot_mutation(
                &mut fixture.registry(),
                confirmed,
                NOW,
                &fixture.descriptor,
                proto::SnapshotStatus::Accepted,
                written(&appended),
            ),
            Err(SnapshotOperationError::RegistryConfirmation)
        );
    }
}

#[test]
fn append_confirmation_uses_current_time_for_snapshot_and_authority_validity() {
    let fixture = Fixture::new();
    let appended = fixture.appended(2);
    assert_eq!(
        confirm_snapshot_mutation(
            &mut fixture.registry(),
            appended.clone(),
            NOW + 600,
            &fixture.descriptor,
            proto::SnapshotStatus::Accepted,
            written(&appended),
        ),
        Err(SnapshotOperationError::RegistryConfirmation)
    );
    let mut confirmed = advance(appended.clone(), 3);
    let mut key = confirmed.authority_keys[0].record().clone();
    key.status = AuthorityKeyStatus::Revoked;
    key.revoked_at_unix_seconds = Some(NOW);
    key.revocation_reason = Some("owner request".into());
    confirmed.authority_keys[0] = TrustedAuthorityKey::from_record(key, DOMAIN).unwrap();
    assert_eq!(
        confirm_snapshot_mutation(
            &mut fixture.registry(),
            confirmed,
            NOW,
            &fixture.descriptor,
            proto::SnapshotStatus::Accepted,
            written(&appended),
        ),
        Err(SnapshotOperationError::RegistryConfirmation)
    );
}

#[test]
fn revoke_confirmation_accepts_newer_publications_and_a_revoked_bound_policy() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    registry.install_generation(fixture.appended(2)).unwrap();
    let revoked = revoke_root(fixture.appended(3));
    let confirmed = revoke_policy(advance(revoked.clone(), 4));
    let result = confirm_snapshot_mutation(
        &mut registry,
        confirmed,
        NOW,
        &fixture.descriptor,
        proto::SnapshotStatus::Revoked,
        written(&revoked),
    )
    .unwrap();
    assert_eq!(result.registry_generation, 4);
    assert_eq!(result.status, proto::SnapshotStatus::Revoked as i32);
}

#[test]
fn revoke_confirmation_rejects_changed_revocation_metadata() {
    let fixture = Fixture::new();
    let revoked = revoke_root(fixture.appended(3));
    let mut confirmed = advance(revoked.clone(), 4);
    confirmed.accepted_roots[0].revocation_reason = Some("different reason".into());
    assert_eq!(
        confirm_snapshot_mutation(
            &mut fixture.registry(),
            confirmed,
            NOW,
            &fixture.descriptor,
            proto::SnapshotStatus::Revoked,
            written(&revoked),
        ),
        Err(SnapshotOperationError::RegistryConfirmation)
    );
}

#[tokio::test]
async fn stale_append_retries_with_the_original_descriptor_and_private_snapshot() {
    let fixture = Fixture::new();
    let private_root = TestDirectory::new();
    let store = PrivateSnapshotStore::open(&private_root.0).unwrap();
    store.publish(&fixture.private_snapshot).unwrap();
    let before = std::fs::read_dir(&private_root.0)
        .unwrap()
        .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    let mut refreshed = advance(fixture.initial.clone(), 2);
    refreshed.policies.push(policy_record("unrelated", 1, 2));
    let mut appended = advance(refreshed.clone(), 3);
    appended.accepted_roots = fixture.appended(3).accepted_roots;
    let mut loads = vec![refreshed, advance(appended.clone(), 4)].into_iter();
    let mut calls = Vec::new();
    let result = mutate_snapshot_registry(
        fixture.registry(),
        &fixture.descriptor,
        proto::SnapshotStatus::Accepted,
        |generation| {
            calls.push((generation, fixture.descriptor.clone()));
            ready(if calls.len() == 1 {
                Err(SnapshotOperationError::WriterStaleGeneration)
            } else {
                Ok(written(&appended))
            })
        },
        || ready(Ok((loads.next().unwrap(), NOW))),
    )
    .await
    .unwrap();
    assert_eq!(
        calls,
        vec![
            (1, fixture.descriptor.clone()),
            (2, fixture.descriptor.clone())
        ]
    );
    assert_eq!(result.registry_generation, 4);
    assert!(loads.next().is_none());
    let after = std::fs::read_dir(&private_root.0)
        .unwrap()
        .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    assert_eq!(after.len(), 1);
    assert_eq!(
        store
            .load(
                &fixture.initial.policies[0].policy,
                &fixture.descriptor.snapshot_id,
                fixture.private_snapshot.relationship_root(),
            )
            .unwrap(),
        fixture.private_snapshot
    );
}

#[tokio::test]
async fn stale_append_does_not_retry_a_revoked_policy() {
    let fixture = Fixture::new();
    let mut calls = 0;
    let result = mutate_snapshot_registry(
        fixture.registry(),
        &fixture.descriptor,
        proto::SnapshotStatus::Accepted,
        |_| {
            calls += 1;
            ready(Err(SnapshotOperationError::WriterStaleGeneration))
        },
        || {
            ready(Ok((
                revoke_policy(advance(fixture.initial.clone(), 2)),
                NOW,
            )))
        },
    )
    .await;
    assert_eq!(result, Err(SnapshotOperationError::PolicyResolution));
    assert_eq!(calls, 1);
}

#[tokio::test]
async fn writer_retries_are_bounded_and_do_not_retry_other_failures() {
    let fixture = Fixture::new();
    for (error, expected_calls) in [
        (
            SnapshotOperationError::WriterStaleGeneration,
            MAX_REGISTRY_WRITE_ATTEMPTS,
        ),
        (SnapshotOperationError::WriterConflict, 1),
        (SnapshotOperationError::WriterUnavailable, 1),
    ] {
        let mut calls = 0;
        let mut generation = 1;
        let result = mutate_snapshot_registry(
            fixture.registry(),
            &fixture.descriptor,
            proto::SnapshotStatus::Accepted,
            |_| {
                calls += 1;
                ready(Err(error))
            },
            || {
                generation += 1;
                ready(Ok((advance(fixture.initial.clone(), generation), NOW)))
            },
        )
        .await;
        assert_eq!(result, Err(error));
        assert_eq!(calls, expected_calls);
        assert_eq!(generation, expected_calls as u64);
    }
}

#[tokio::test]
async fn stale_revoke_retries_the_same_target_in_the_newer_generation() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    registry.install_generation(fixture.appended(2)).unwrap();
    let revoked = revoke_root(fixture.appended(4));
    let mut loads = vec![fixture.appended(3), advance(revoked.clone(), 5)].into_iter();
    let mut calls = Vec::new();
    let result = mutate_snapshot_registry(
        registry,
        &fixture.descriptor,
        proto::SnapshotStatus::Revoked,
        |generation| {
            calls.push(generation);
            ready(if calls.len() == 1 {
                Err(SnapshotOperationError::WriterStaleGeneration)
            } else {
                Ok(written(&revoked))
            })
        },
        || ready(Ok((loads.next().unwrap(), NOW))),
    )
    .await
    .unwrap();
    assert_eq!(calls, vec![2, 3]);
    assert_eq!(result.descriptor.as_ref(), Some(&fixture.descriptor));
    assert_eq!(result.status, proto::SnapshotStatus::Revoked as i32);
}

struct Fixture {
    initial: RegistryGenerationV1,
    descriptor: proto::RelationshipSnapshotDescriptor,
    private_snapshot: PrivateRelationshipSnapshot,
}

impl Fixture {
    fn new() -> Self {
        let policy_record = policy_record("bound", 1, 1);
        let policy_hash = policy_record.claims.claims().policy_hash;
        let salt = [0x5a; 32];
        let root = SnapshotTree::build(
            &policy_record.policy,
            vec![],
            &salt,
            TRUSTED_SNAPSHOT_LIMITS,
        )
        .unwrap()
        .root();
        let signer = snapshot_signer();
        let claims = ValidatedSnapshotDescriptorClaimsV1::new(
            SnapshotDescriptorClaimsV1 {
                format_version: raziel_snapshot::SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
                trust_domain_id: DOMAIN.into(),
                snapshot_id: format!("snapshot-v1:{}", lowercase_hex(&root)),
                relationship_snapshot_root: root,
                policy_record_id: "bound-1".into(),
                policy_hash,
                spicedb_revision: "revision-1".into(),
                epoch: 1,
                issued_at_unix_seconds: NOW,
                expires_at_unix_seconds: NOW + 600,
                canonicalization_version: RELATIONSHIP_TREE_VERSION,
                signing_key_id: signer.key_id().into(),
            },
            snapshot_limits(),
        )
        .unwrap();
        let descriptor =
            descriptor_to_proto(&claims, signer.sign_snapshot_descriptor(&claims).unwrap());
        let private_snapshot = PrivateRelationshipSnapshot::new(
            &policy_record.policy,
            descriptor.snapshot_id.clone(),
            root,
            salt,
            vec![],
        )
        .unwrap();
        Self {
            initial: RegistryGenerationV1 {
                generation: 1,
                publication_digest: [1; 32],
                authority_keys: vec![
                    key(
                        policy_signer().key_id(),
                        policy_signer().public_key_bytes(),
                        AuthorityRole::PolicyApproval,
                    ),
                    key(
                        signer.key_id(),
                        signer.public_key_bytes(),
                        AuthorityRole::Snapshot,
                    ),
                ],
                policies: vec![policy_record],
                accepted_roots: vec![],
            },
            descriptor,
            private_snapshot,
        }
    }

    fn registry(&self) -> LocalRegistry {
        let mut registry = LocalRegistry::new(default_registry_config(DOMAIN).unwrap());
        registry.install_generation(self.initial.clone()).unwrap();
        registry
    }

    fn appended(&self, generation: u64) -> RegistryGenerationV1 {
        let mut result = advance(self.initial.clone(), generation);
        result.accepted_roots.push(
            root_record_from_proto(
                proto::AcceptedRootRecord {
                    format_version: 1,
                    descriptor: Some(self.descriptor.clone()),
                    status: proto::SnapshotStatus::Accepted as i32,
                    registered_at: Some(timestamp(NOW)),
                    revoked_at: None,
                    revocation_reason: String::new(),
                    registry_generation: generation,
                },
                &default_registry_config(DOMAIN).unwrap(),
                NOW,
            )
            .unwrap(),
        );
        result
    }
}

fn policy_record(name: &str, epoch: u64, generation: u64) -> PolicyRegistryRecordV1 {
    let mut wire = proto::PolicyProgram::decode(POLICY_BYTES).unwrap();
    if name == "unrelated" {
        wire.source_schema_digest[0] ^= 1;
    }
    let bytes = wire.encode_to_vec();
    let policy =
        decode_policy_program_strict(&bytes, bytes.len() as u64, &TRUSTED_POLICY_CEILINGS).unwrap();
    let signer = policy_signer();
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: DOMAIN.into(),
            record_id: format!("{name}-{epoch}"),
            policy_name: name.into(),
            policy_version: "v1".into(),
            approval_epoch: epoch,
            policy_hash: policy_hash(&policy).unwrap(),
            valid_from_unix_seconds: NOW - 100,
            valid_until_unix_seconds: NOW + 3_600,
            approved_by: "operator".into(),
            signing_key_id: signer.key_id().into(),
        },
        policy_limits(),
    )
    .unwrap();
    PolicyRegistryRecordV1 {
        authority_signature: signer.sign_policy_approval(&claims).unwrap(),
        claims,
        policy,
        canonical_policy_ir_bytes: bytes,
        status: PolicyRecordStatus::Approved,
        registered_at_unix_seconds: NOW,
        revoked_at_unix_seconds: None,
        revocation_reason: None,
        registry_generation: generation,
    }
}

fn advance(mut value: RegistryGenerationV1, generation: u64) -> RegistryGenerationV1 {
    value.generation = generation;
    value.publication_digest = [generation as u8; 32];
    for key in &mut value.authority_keys {
        let mut record = key.record().clone();
        record.registry_generation = generation;
        *key = TrustedAuthorityKey::from_record(record, DOMAIN).unwrap();
    }
    for policy in &mut value.policies {
        policy.registry_generation = generation;
    }
    for root in &mut value.accepted_roots {
        root.registry_generation = generation;
    }
    value
}

fn revoke_policy(mut value: RegistryGenerationV1) -> RegistryGenerationV1 {
    value.policies[0].status = PolicyRecordStatus::Revoked;
    value.policies[0].revoked_at_unix_seconds = Some(NOW);
    value.policies[0].revocation_reason = Some("owner request".into());
    value
}

fn revoke_root(mut value: RegistryGenerationV1) -> RegistryGenerationV1 {
    value.accepted_roots[0].status = RootRecordStatus::Revoked;
    value.accepted_roots[0].revoked_at_unix_seconds = Some(NOW);
    value.accepted_roots[0].revocation_reason = Some("owner request".into());
    value
}

fn written(value: &RegistryGenerationV1) -> RegistryWriterResult {
    RegistryWriterResult {
        registry_generation: value.generation,
        root_record: root_record_to_proto(&value.accepted_roots[0]),
    }
}

fn key(id: &str, public_key: [u8; 32], role: AuthorityRole) -> TrustedAuthorityKey {
    TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: DOMAIN.into(),
            key_id: id.into(),
            role,
            public_key,
            status: AuthorityKeyStatus::Active,
            valid_from_unix_seconds: NOW - 1_000,
            valid_until_unix_seconds: NOW + 10_000,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        },
        DOMAIN,
    )
    .unwrap()
}

fn policy_signer() -> AuthoritySigner<PolicyApprovalRole> {
    AuthoritySigner::from_pkcs8_der(&test_key(1)).unwrap()
}

fn snapshot_signer() -> AuthoritySigner<SnapshotRole> {
    AuthoritySigner::from_pkcs8_der(&test_key(2)).unwrap()
}

fn test_key(seed: u8) -> Vec<u8> {
    [
        &[
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20,
        ],
        &[seed; 32][..],
    ]
    .concat()
}

fn policy_limits() -> PolicyApprovalLimits {
    PolicyApprovalLimits {
        max_string_bytes: 512,
        max_lifetime_seconds: 86_400,
    }
}

fn snapshot_limits() -> SnapshotDescriptorLimits {
    SnapshotDescriptorLimits {
        max_string_bytes: 512,
        max_lifetime_seconds: 86_400,
    }
}

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "snapshot-confirmation-{}-{unique}",
            std::process::id()
        )))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
