use std::sync::atomic::{AtomicU64, Ordering};

use prost::Message;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    AuthoritySigner, PolicyApprovalRole, SnapshotRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::{
    PolicyLimits, ValidatedPolicyProgram, ValidationCeilings, decode_policy_program_strict,
};
use raziel_proto::raziel::v1 as proto;
use raziel_snapshot::{
    RELATIONSHIP_TREE_VERSION, SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SnapshotDescriptorClaimsV1,
    SnapshotDescriptorLimits, ValidatedSnapshotDescriptorClaimsV1,
};
use raziel_verifier::{
    AcceptedRootRecordV1, InstallOutcome, LocalRegistry, PolicyRecordStatus,
    PolicyRegistryRecordV1, RegistryConfig, RegistryError, RegistryGenerationV1, RegistryLimits,
    RegistryLoader, RootRecordStatus, authority_key_from_proto, authority_key_to_proto,
    policy_record_from_proto, policy_record_to_proto, root_record_from_proto, root_record_to_proto,
};
use sha2::{Digest, Sha256};

const TEST_NOW: i64 = 1_700_000_000;
const POLICY_BYTES: &[u8] = include_bytes!(
    "../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);
const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

#[test]
fn installs_and_resolves_an_exact_policy_and_root() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    assert_eq!(
        registry.install_generation(fixture.generation(1)),
        Ok(InstallOutcome::Installed)
    );
    assert_eq!(
        registry.install_generation(fixture.generation(1)),
        Ok(InstallOutcome::AlreadyCurrent)
    );

    let policy = registry
        .resolve_policy_exact("policy-1", &fixture.policy_hash, TEST_NOW)
        .unwrap();
    assert_eq!(policy.claims.claims().approval_epoch, 1);

    let resolved = registry.resolve_root(&[0x22; 32], TEST_NOW).unwrap();
    assert_eq!(
        resolved.root_record.claims.claims().snapshot_id,
        "snapshot-1"
    );
    assert_eq!(resolved.policy_record.claims.claims().record_id, "policy-1");

    let resolved_by_id = registry
        .resolve_snapshot_exact("snapshot-1", TEST_NOW)
        .unwrap();
    assert_eq!(resolved_by_id, resolved);
    assert_eq!(registry.observed_time_floor(), Some(TEST_NOW));
}

#[test]
fn unknown_expired_and_clock_rollback_paths_fail_closed() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(fixture.generation(1)).unwrap();

    assert_eq!(
        registry.resolve_policy_exact("missing", &fixture.policy_hash, TEST_NOW),
        Err(RegistryError::UnknownPolicy)
    );
    assert_eq!(
        registry.resolve_root(&[0x99; 32], TEST_NOW),
        Err(RegistryError::UnknownRoot)
    );
    assert_eq!(
        registry.resolve_snapshot_exact("missing", TEST_NOW),
        Err(RegistryError::UnknownRoot)
    );
    assert_eq!(
        registry.resolve_snapshot_exact("snapshot-1", TEST_NOW + 101),
        Err(RegistryError::ExpiredRoot)
    );
    assert_eq!(
        registry.resolve_root(&[0x22; 32], TEST_NOW),
        Err(RegistryError::ExpiredRoot),
        "a backward clock step must not revive an expired root"
    );
}

#[test]
fn invalid_signatures_unknown_keys_and_epoch_gaps_do_not_install() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());

    let mut tampered = fixture.generation(1);
    tampered.accepted_roots[0].authority_signature[0] ^= 1;
    assert!(matches!(
        registry.install_generation(tampered),
        Err(RegistryError::Authority(_))
    ));

    let mut missing_key = fixture.generation(1);
    missing_key.authority_keys.pop();
    assert_eq!(
        registry.install_generation(missing_key),
        Err(RegistryError::UnknownAuthorityKey)
    );

    let mut policy_gap = fixture.generation(1);
    policy_gap.policies[0] = fixture.policy_record(2, "policy-2", 1, PolicyRecordStatus::Approved);
    policy_gap.accepted_roots.clear();
    assert_eq!(
        registry.install_generation(policy_gap),
        Err(RegistryError::EpochGap)
    );

    let mut root_gap = fixture.generation(1);
    root_gap.accepted_roots[0] =
        fixture.root_record(2, "snapshot-2", "policy-1", 1, RootRecordStatus::Accepted);
    assert_eq!(
        registry.install_generation(root_gap),
        Err(RegistryError::EpochGap)
    );

    let mut detached_policy = fixture.generation(1);
    let mut changed_program = detached_policy.policies[0].policy.program().clone();
    changed_program.source_schema_digest[0] ^= 1;
    detached_policy.policies[0].policy =
        ValidatedPolicyProgram::new(changed_program, &validation_ceilings()).unwrap();
    assert_eq!(
        registry.install_generation(detached_policy),
        Err(RegistryError::PolicyHashMismatch)
    );

    let mut active_policy_with_revocation_metadata = fixture.generation(1);
    active_policy_with_revocation_metadata.policies[0].revocation_reason =
        Some("not revoked".into());
    assert_eq!(
        registry.install_generation(active_policy_with_revocation_metadata),
        Err(RegistryError::InvalidValidityWindow)
    );

    let mut revoked_policy_without_time = fixture.generation(1);
    revoked_policy_without_time.policies[0].status = PolicyRecordStatus::Revoked;
    revoked_policy_without_time.policies[0].revocation_reason = Some("revoked".into());
    assert_eq!(
        registry.install_generation(revoked_policy_without_time),
        Err(RegistryError::InvalidValidityWindow)
    );

    let mut active_root_with_revocation_metadata = fixture.generation(1);
    active_root_with_revocation_metadata.accepted_roots[0].revocation_reason =
        Some("not revoked".into());
    assert_eq!(
        registry.install_generation(active_root_with_revocation_metadata),
        Err(RegistryError::InvalidValidityWindow)
    );

    let mut same_key_for_both_roles = fixture.generation(1);
    same_key_for_both_roles.authority_keys[1] = trusted_key(
        &fixture.policy_signer,
        AuthorityRole::Snapshot,
        1,
        AuthorityKeyStatus::Active,
    );
    assert_eq!(
        registry.install_generation(same_key_for_both_roles),
        Err(RegistryError::WrongAuthorityRole)
    );

    let mut duplicate_root = fixture.generation(1);
    duplicate_root.accepted_roots.push(fixture.root_record(
        2,
        "snapshot-2",
        "policy-1",
        1,
        RootRecordStatus::Accepted,
    ));
    assert_eq!(
        registry.install_generation(duplicate_root),
        Err(RegistryError::DuplicateRecord)
    );

    let claims = ValidatedSnapshotDescriptorClaimsV1::new(
        SnapshotDescriptorClaimsV1 {
            format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            snapshot_id: "snapshot-1".into(),
            relationship_snapshot_root: [0x22; 32],
            policy_record_id: "policy-1".into(),
            policy_hash: fixture.policy_hash,
            spicedb_revision: "zed-1".into(),
            epoch: 1,
            issued_at_unix_seconds: TEST_NOW - 10,
            expires_at_unix_seconds: TEST_NOW + 901,
            canonicalization_version: RELATIONSHIP_TREE_VERSION,
            signing_key_id: fixture.snapshot_signer.key_id().into(),
        },
        SnapshotDescriptorLimits {
            max_string_bytes: 256,
            max_lifetime_seconds: 2_000,
        },
    )
    .unwrap();
    let mut outside_policy_window = fixture.generation(1);
    outside_policy_window.accepted_roots[0].authority_signature = fixture
        .snapshot_signer
        .sign_snapshot_descriptor(&claims)
        .unwrap();
    outside_policy_window.accepted_roots[0].claims = claims;
    assert_eq!(
        registry.install_generation(outside_policy_window),
        Err(RegistryError::InvalidValidityWindow)
    );
}

#[test]
fn policy_rotation_never_falls_back_to_an_older_approval() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(fixture.generation(1)).unwrap();

    let mut generation_two = fixture.generation(2);
    generation_two.policies.push(fixture.policy_record(
        2,
        "policy-2",
        2,
        PolicyRecordStatus::Approved,
    ));
    registry.install_generation(generation_two).unwrap();
    assert_eq!(
        registry.resolve_policy_exact("policy-1", &fixture.policy_hash, TEST_NOW),
        Err(RegistryError::UnapprovedPolicy)
    );
    assert_eq!(
        registry
            .resolve_policy_hash(&fixture.policy_hash, TEST_NOW)
            .unwrap()
            .claims
            .claims()
            .record_id,
        "policy-2"
    );

    let mut generation_three = fixture.generation(3);
    generation_three.policies.push(fixture.policy_record(
        2,
        "policy-2",
        3,
        PolicyRecordStatus::Revoked,
    ));
    registry.install_generation(generation_three).unwrap();
    assert_eq!(
        registry.resolve_policy_hash(&fixture.policy_hash, TEST_NOW),
        Err(RegistryError::RevokedPolicy)
    );

    let mut attempted_revival = fixture.generation(4);
    attempted_revival.policies.push(fixture.policy_record(
        2,
        "policy-2",
        4,
        PolicyRecordStatus::Approved,
    ));
    assert_eq!(
        registry.install_generation(attempted_revival),
        Err(RegistryError::RevocationRollback)
    );
}

#[test]
fn policy_key_rotation_keeps_old_trust_and_uses_the_new_signature() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(fixture.generation(1)).unwrap();

    let rotated_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x03)).unwrap();
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            record_id: "policy-2".into(),
            policy_name: "reconciliation".into(),
            policy_version: "v2".into(),
            approval_epoch: 2,
            policy_hash: fixture.policy_hash,
            valid_from_unix_seconds: TEST_NOW - 500,
            valid_until_unix_seconds: TEST_NOW + 900,
            approved_by: "test-maintainer".into(),
            signing_key_id: rotated_signer.key_id().into(),
        },
        PolicyApprovalLimits {
            max_string_bytes: 256,
            max_lifetime_seconds: 2_000,
        },
    )
    .unwrap();
    let signature = rotated_signer.sign_policy_approval(&claims).unwrap();

    let mut generation_two = fixture.generation(2);
    generation_two.authority_keys.push(trusted_key(
        &rotated_signer,
        AuthorityRole::PolicyApproval,
        2,
        AuthorityKeyStatus::Active,
    ));
    generation_two.policies.push(PolicyRegistryRecordV1 {
        claims,
        policy: fixture.policy.clone(),
        canonical_policy_ir_bytes: POLICY_BYTES.to_vec(),
        authority_signature: signature,
        status: PolicyRecordStatus::Approved,
        registered_at_unix_seconds: TEST_NOW - 100,
        revoked_at_unix_seconds: None,
        revocation_reason: None,
        registry_generation: 2,
    });
    registry.install_generation(generation_two).unwrap();
    assert_eq!(
        registry
            .resolve_policy_hash(&fixture.policy_hash, TEST_NOW)
            .unwrap()
            .claims
            .claims()
            .signing_key_id,
        rotated_signer.key_id()
    );
}

#[test]
fn root_and_key_revocation_are_one_way_and_take_effect_immediately() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(fixture.generation(1)).unwrap();

    let mut root_revoked = fixture.generation(2);
    root_revoked.accepted_roots[0] =
        fixture.root_record(1, "snapshot-1", "policy-1", 2, RootRecordStatus::Revoked);
    registry.install_generation(root_revoked).unwrap();
    assert_eq!(
        registry.resolve_root(&[0x22; 32], TEST_NOW),
        Err(RegistryError::RevokedRoot)
    );

    let attempted_root_revival = fixture.generation(3);
    assert_eq!(
        registry.install_generation(attempted_root_revival),
        Err(RegistryError::RevocationRollback)
    );

    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(fixture.generation(1)).unwrap();
    let mut key_revoked = fixture.generation(2);
    key_revoked.authority_keys[1] = fixture.snapshot_key(2, AuthorityKeyStatus::Revoked);
    registry.install_generation(key_revoked).unwrap();
    assert!(matches!(
        registry.resolve_root(&[0x22; 32], TEST_NOW),
        Err(RegistryError::Authority(_))
    ));
}

#[test]
fn generation_and_immutable_record_rollbacks_fail() {
    let fixture = Fixture::new();
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(fixture.generation(1)).unwrap();
    registry.install_generation(fixture.generation(2)).unwrap();

    assert_eq!(
        registry.install_generation(fixture.generation(1)),
        Err(RegistryError::RegistryGenerationRollback)
    );

    let mut conflicting_same_generation = fixture.generation(2);
    conflicting_same_generation.publication_digest = [0xaa; 32];
    assert_eq!(
        registry.install_generation(conflicting_same_generation),
        Err(RegistryError::RegistryGenerationRollback)
    );

    let mut changed = fixture.generation(3);
    changed.policies[0].registered_at_unix_seconds += 1;
    assert_eq!(
        registry.install_generation(changed),
        Err(RegistryError::ImmutableRecordChanged)
    );
}

#[test]
fn persisted_records_reject_derived_statuses_and_invalid_claim_fields() {
    let fixture = Fixture::new();
    let generation = fixture.generation(1);
    let config = config();

    let mut key = authority_key_to_proto(&generation.authority_keys[0]);
    key.valid_from.as_mut().unwrap().nanos = 1;
    assert!(matches!(
        authority_key_from_proto(key, &config),
        Err(RegistryError::InvalidTimestamp)
    ));

    let mut expired_policy = policy_record_to_proto(&generation.policies[0]).unwrap();
    expired_policy.status = proto::PolicyStatus::Expired as i32;
    assert!(matches!(
        policy_record_from_proto(expired_policy, &config),
        Err(RegistryError::InvalidEnum)
    ));

    let mut wrong_hash = policy_record_to_proto(&generation.policies[0]).unwrap();
    wrong_hash.policy_hash[0] ^= 1;
    assert!(matches!(
        policy_record_from_proto(wrong_hash, &config),
        Err(RegistryError::PolicyHashMismatch)
    ));

    let mut active_policy_with_reason = policy_record_to_proto(&generation.policies[0]).unwrap();
    active_policy_with_reason.revocation_reason = "not revoked".into();
    assert!(matches!(
        policy_record_from_proto(active_policy_with_reason, &config),
        Err(RegistryError::InvalidValidityWindow)
    ));

    let mut expired_root = root_record_to_proto(&generation.accepted_roots[0]);
    expired_root.status = proto::SnapshotStatus::Expired as i32;
    assert!(matches!(
        root_record_from_proto(expired_root, &config, TEST_NOW),
        Err(RegistryError::InvalidEnum)
    ));

    let mut active_root_with_reason = root_record_to_proto(&generation.accepted_roots[0]);
    active_root_with_reason.revocation_reason = "not revoked".into();
    assert!(matches!(
        root_record_from_proto(active_root_with_reason, &config, TEST_NOW),
        Err(RegistryError::InvalidValidityWindow)
    ));

    let mut future_root = root_record_to_proto(&generation.accepted_roots[0]);
    future_root
        .descriptor
        .as_mut()
        .unwrap()
        .issued_at
        .as_mut()
        .unwrap()
        .seconds = TEST_NOW + 6;
    assert!(matches!(
        root_record_from_proto(future_root, &config, TEST_NOW),
        Err(RegistryError::InvalidTimestamp)
    ));
}

#[test]
fn manifest_loader_hashes_strictly_parses_and_installs_one_generation() {
    let fixture = Fixture::new();
    let generation = fixture.generation(1);
    let directory = TempRegistryDir::new();

    let authority_keys = vec![
        directory.write_record(
            "keys/policy.pb",
            authority_key_to_proto(&generation.authority_keys[0]).encode_to_vec(),
        ),
        directory.write_record(
            "keys/snapshot.pb",
            authority_key_to_proto(&generation.authority_keys[1]).encode_to_vec(),
        ),
    ];
    let policies = vec![
        directory.write_record(
            "policies/policy-1.pb",
            policy_record_to_proto(&generation.policies[0])
                .unwrap()
                .encode_to_vec(),
        ),
    ];
    let accepted_roots = vec![directory.write_record(
        "roots/snapshot-1.pb",
        root_record_to_proto(&generation.accepted_roots[0]).encode_to_vec(),
    )];
    let manifest = proto::RegistryGenerationManifest {
        format_version: 1,
        trust_domain_id: "raziel-test".into(),
        generation: 1,
        authority_keys,
        policies,
        accepted_roots,
        published_at: Some(prost_types::Timestamp {
            seconds: TEST_NOW,
            nanos: 0,
        }),
    };
    let manifest_bytes = manifest.encode_to_vec();
    directory.write_bytes("current.pb", &manifest_bytes);

    let loader = RegistryLoader::new(config());
    let candidate = loader
        .load_candidate(directory.path(), &manifest_bytes, TEST_NOW)
        .unwrap();
    let current = loader.load_current(directory.path(), TEST_NOW).unwrap();
    assert_eq!(current.generation, candidate.generation);
    assert_eq!(current.publication_digest, candidate.publication_digest);
    assert_eq!(current.authority_keys.len(), candidate.authority_keys.len());
    assert_eq!(current.policies.len(), candidate.policies.len());
    assert_eq!(current.accepted_roots.len(), candidate.accepted_roots.len());
    let mut registry = LocalRegistry::new(config());
    registry.install_generation(candidate).unwrap();
    assert!(registry.resolve_root(&[0x22; 32], TEST_NOW).is_ok());

    let mut bad_digest = manifest.clone();
    bad_digest.policies[0].sha256[0] ^= 1;
    assert!(matches!(
        loader.load_candidate(directory.path(), &bad_digest.encode_to_vec(), TEST_NOW),
        Err(RegistryError::FileDigestMismatch)
    ));

    let mut unsafe_path = manifest.clone();
    unsafe_path.policies[0].relative_path = "../policy.pb".into();
    assert!(matches!(
        loader.load_candidate(directory.path(), &unsafe_path.encode_to_vec(), TEST_NOW),
        Err(RegistryError::FilePathInvalid)
    ));

    let mut unsorted = manifest;
    unsorted.authority_keys.swap(0, 1);
    assert!(matches!(
        loader.load_candidate(directory.path(), &unsorted.encode_to_vec(), TEST_NOW),
        Err(RegistryError::FilePathInvalid)
    ));

    let oversized_entry = directory.write_record("keys/oversized.pb", vec![0; (1 << 20) + 1]);
    let oversized = proto::RegistryGenerationManifest {
        format_version: 1,
        trust_domain_id: "raziel-test".into(),
        generation: 1,
        authority_keys: vec![oversized_entry],
        published_at: Some(prost_types::Timestamp {
            seconds: TEST_NOW,
            nanos: 0,
        }),
        ..proto::RegistryGenerationManifest::default()
    };
    assert!(matches!(
        loader.load_candidate(directory.path(), &oversized.encode_to_vec(), TEST_NOW),
        Err(RegistryError::InputTooLarge)
    ));

    let mut invalid_timestamp = proto::RegistryGenerationManifest {
        format_version: 1,
        trust_domain_id: "raziel-test".into(),
        generation: 1,
        published_at: Some(prost_types::Timestamp {
            seconds: -62_135_596_801,
            nanos: 0,
        }),
        ..proto::RegistryGenerationManifest::default()
    };
    assert!(matches!(
        loader.load_candidate(
            directory.path(),
            &invalid_timestamp.encode_to_vec(),
            TEST_NOW,
        ),
        Err(RegistryError::InvalidTimestamp)
    ));
    invalid_timestamp.published_at.as_mut().unwrap().seconds = TEST_NOW;
    invalid_timestamp.published_at.as_mut().unwrap().nanos = 1;
    assert!(matches!(
        loader.load_candidate(
            directory.path(),
            &invalid_timestamp.encode_to_vec(),
            TEST_NOW,
        ),
        Err(RegistryError::InvalidTimestamp)
    ));
}

#[cfg(unix)]
#[test]
fn manifest_loader_rejects_symbolic_links() {
    use std::os::unix::fs::symlink;

    let directory = TempRegistryDir::new();
    let target = directory.write_bytes("keys/target.pb", &[0x08, 0x01]);
    let link = directory.path().join("keys/link.pb");
    symlink(target, &link).unwrap();
    let manifest = proto::RegistryGenerationManifest {
        format_version: 1,
        trust_domain_id: "raziel-test".into(),
        generation: 1,
        authority_keys: vec![proto::RegistryFile {
            relative_path: "keys/link.pb".into(),
            sha256: Sha256::digest([0x08, 0x01]).to_vec(),
        }],
        published_at: Some(prost_types::Timestamp {
            seconds: TEST_NOW,
            nanos: 0,
        }),
        ..proto::RegistryGenerationManifest::default()
    };

    assert!(matches!(
        RegistryLoader::new(config()).load_candidate(
            directory.path(),
            &manifest.encode_to_vec(),
            TEST_NOW,
        ),
        Err(RegistryError::SymlinkForbidden)
    ));
}

struct Fixture {
    policy_signer: AuthoritySigner<PolicyApprovalRole>,
    snapshot_signer: AuthoritySigner<SnapshotRole>,
    policy: raziel_policy_ir::ValidatedPolicyProgram,
    policy_hash: [u8; 32],
}

impl Fixture {
    fn new() -> Self {
        let policy_signer = AuthoritySigner::from_pkcs8_der(&test_pkcs8(0x01)).unwrap();
        let snapshot_signer = AuthoritySigner::from_pkcs8_der(&test_pkcs8(0x02)).unwrap();
        let policy = decode_policy_program_strict(
            POLICY_BYTES,
            POLICY_BYTES.len() as u64,
            &validation_ceilings(),
        )
        .unwrap();
        let policy_hash = policy_hash(&policy).unwrap();
        Self {
            policy_signer,
            snapshot_signer,
            policy,
            policy_hash,
        }
    }

    fn generation(&self, generation: u64) -> RegistryGenerationV1 {
        RegistryGenerationV1 {
            generation,
            publication_digest: digest_for_generation(generation),
            authority_keys: vec![
                self.policy_key(generation, AuthorityKeyStatus::Active),
                self.snapshot_key(generation, AuthorityKeyStatus::Active),
            ],
            policies: vec![self.policy_record(
                1,
                "policy-1",
                generation,
                PolicyRecordStatus::Approved,
            )],
            accepted_roots: vec![self.root_record(
                1,
                "snapshot-1",
                "policy-1",
                generation,
                RootRecordStatus::Accepted,
            )],
        }
    }

    fn policy_key(&self, generation: u64, status: AuthorityKeyStatus) -> TrustedAuthorityKey {
        trusted_key(
            &self.policy_signer,
            AuthorityRole::PolicyApproval,
            generation,
            status,
        )
    }

    fn snapshot_key(&self, generation: u64, status: AuthorityKeyStatus) -> TrustedAuthorityKey {
        trusted_key(
            &self.snapshot_signer,
            AuthorityRole::Snapshot,
            generation,
            status,
        )
    }

    fn policy_record(
        &self,
        epoch: u64,
        record_id: &str,
        generation: u64,
        status: PolicyRecordStatus,
    ) -> PolicyRegistryRecordV1 {
        let claims = ValidatedPolicyApprovalClaimsV1::new(
            PolicyApprovalClaimsV1 {
                format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                record_id: record_id.into(),
                policy_name: "reconciliation".into(),
                policy_version: format!("v{epoch}"),
                approval_epoch: epoch,
                policy_hash: self.policy_hash,
                valid_from_unix_seconds: TEST_NOW - 500,
                valid_until_unix_seconds: TEST_NOW + 900,
                approved_by: "test-maintainer".into(),
                signing_key_id: self.policy_signer.key_id().into(),
            },
            PolicyApprovalLimits {
                max_string_bytes: 256,
                max_lifetime_seconds: 2_000,
            },
        )
        .unwrap();
        let signature = self.policy_signer.sign_policy_approval(&claims).unwrap();
        let revoked = status == PolicyRecordStatus::Revoked;
        PolicyRegistryRecordV1 {
            claims,
            policy: self.policy.clone(),
            canonical_policy_ir_bytes: POLICY_BYTES.to_vec(),
            authority_signature: signature,
            status,
            registered_at_unix_seconds: TEST_NOW - 600,
            revoked_at_unix_seconds: revoked.then_some(TEST_NOW),
            revocation_reason: revoked.then(|| "test revocation".into()),
            registry_generation: generation,
        }
    }

    fn root_record(
        &self,
        epoch: u64,
        snapshot_id: &str,
        policy_record_id: &str,
        generation: u64,
        status: RootRecordStatus,
    ) -> AcceptedRootRecordV1 {
        let claims = ValidatedSnapshotDescriptorClaimsV1::new(
            SnapshotDescriptorClaimsV1 {
                format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                snapshot_id: snapshot_id.into(),
                relationship_snapshot_root: [0x22; 32],
                policy_record_id: policy_record_id.into(),
                policy_hash: self.policy_hash,
                spicedb_revision: format!("zed-{epoch}"),
                epoch,
                issued_at_unix_seconds: TEST_NOW - 10,
                expires_at_unix_seconds: TEST_NOW + 100,
                canonicalization_version: RELATIONSHIP_TREE_VERSION,
                signing_key_id: self.snapshot_signer.key_id().into(),
            },
            SnapshotDescriptorLimits {
                max_string_bytes: 256,
                max_lifetime_seconds: 2_000,
            },
        )
        .unwrap();
        let signature = self
            .snapshot_signer
            .sign_snapshot_descriptor(&claims)
            .unwrap();
        let revoked = status == RootRecordStatus::Revoked;
        AcceptedRootRecordV1 {
            claims,
            authority_signature: signature,
            status,
            registered_at_unix_seconds: TEST_NOW - 9,
            revoked_at_unix_seconds: revoked.then_some(TEST_NOW),
            revocation_reason: revoked.then(|| "test revocation".into()),
            registry_generation: generation,
        }
    }
}

fn trusted_key<R: raziel_authority::RoleMarker>(
    signer: &AuthoritySigner<R>,
    role: AuthorityRole,
    generation: u64,
    status: AuthorityKeyStatus,
) -> TrustedAuthorityKey {
    let revoked = status == AuthorityKeyStatus::Revoked;
    TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            key_id: signer.key_id().into(),
            role,
            public_key: signer.public_key_bytes(),
            status,
            valid_from_unix_seconds: TEST_NOW - 1_000,
            valid_until_unix_seconds: TEST_NOW + 1_000,
            revoked_at_unix_seconds: revoked.then_some(TEST_NOW),
            revocation_reason: revoked.then(|| "test revocation".into()),
            registry_generation: generation,
        },
        "raziel-test",
    )
    .unwrap()
}

fn config() -> RegistryConfig {
    RegistryConfig::new(
        "raziel-test".into(),
        RegistryLimits {
            max_record_bytes: 1 << 20,
            max_string_bytes: 256,
            max_artifact_lifetime_seconds: 2_000,
            max_future_clock_skew_seconds: 5,
            max_manifest_files_per_kind: 100,
            max_manifest_total_bytes: 1 << 24,
            policy_validation: validation_ceilings(),
        },
    )
    .unwrap()
}

const fn validation_ceilings() -> ValidationCeilings {
    ValidationCeilings {
        policy_limits: PolicyLimits {
            max_expression_nodes: 256,
            max_caveat_nodes: 256,
            max_path_depth: 16,
            max_relationship_witnesses: 128,
            max_merkle_depth: 32,
            max_string_bytes: 256,
            max_context_entries: 32,
            max_total_private_input_bytes: 1 << 20,
        },
        max_definitions: 32,
        max_relations_per_definition: 32,
        max_allowed_subject_types_per_relation: 32,
        max_permissions: 64,
        max_caveats: 32,
        max_parameters_per_caveat: 16,
        max_syntax_depth: 32,
    }
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
}

fn digest_for_generation(generation: u64) -> [u8; 32] {
    let mut digest = [0; 32];
    digest[..8].copy_from_slice(&generation.to_be_bytes());
    digest
}

static TEMP_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempRegistryDir {
    path: std::path::PathBuf,
}

impl TempRegistryDir {
    fn new() -> Self {
        let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "raziel-registry-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn write_record(&self, relative_path: &str, bytes: Vec<u8>) -> proto::RegistryFile {
        self.write_bytes(relative_path, &bytes);
        proto::RegistryFile {
            relative_path: relative_path.into(),
            sha256: Sha256::digest(bytes).to_vec(),
        }
    }

    fn write_bytes(&self, relative_path: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = self.path.join(relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for TempRegistryDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
