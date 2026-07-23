use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityError, AuthorityKeyRecordV1, AuthorityKeyStatus,
    AuthorityRole, AuthoritySigner, PolicyApprovalRole, SnapshotAuthorityPublisher,
    SnapshotDescriptorDraftV1, SnapshotRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1,
};
use raziel_snapshot::{
    RELATIONSHIP_TREE_VERSION, SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SnapshotDescriptorClaimsV1,
    SnapshotDescriptorLimits, ValidatedSnapshotDescriptorClaimsV1,
};

const TEST_NOW: i64 = 1_700_000_000;
const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

#[test]
fn policy_signatures_verify_and_each_field_is_bound() {
    let signer = policy_signer();
    let key = trusted_policy_key(&signer);
    let claims = policy_claims(signer.key_id());
    let validated = validate_policy(claims.clone());
    let signature = signer.sign_policy_approval(&validated).unwrap();

    assert_eq!(
        key.verify_policy_approval(TEST_NOW, &validated, &signature),
        Ok(())
    );

    let mutations: Vec<PolicyApprovalClaimsV1> = vec![
        PolicyApprovalClaimsV1 {
            format_version: 2,
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            trust_domain_id: "other-domain".into(),
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            record_id: "policy-2".into(),
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            policy_name: "other-policy".into(),
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            policy_version: "v2".into(),
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            approval_epoch: 2,
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            policy_hash: [0x33; 32],
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            valid_from_unix_seconds: claims.valid_from_unix_seconds - 1,
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            valid_until_unix_seconds: claims.valid_until_unix_seconds + 1,
            ..claims.clone()
        },
        PolicyApprovalClaimsV1 {
            approved_by: "other-approver".into(),
            ..claims.clone()
        },
    ];

    for mutated in mutations {
        if let Ok(mutated) = ValidatedPolicyApprovalClaimsV1::new(mutated, policy_limits()) {
            assert_eq!(
                key.verify_policy_approval(TEST_NOW, &mutated, &signature),
                Err(AuthorityError::InvalidSignature)
            );
        }
    }

    let mut wrong_key_id = claims.clone();
    wrong_key_id.signing_key_id =
        "ed25519:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    assert_eq!(
        key.verify_policy_approval(TEST_NOW, &validate_policy(wrong_key_id), &signature,),
        Err(AuthorityError::InvalidKeyId)
    );

    let mut bad_signature = signature;
    bad_signature[0] ^= 1;
    assert_eq!(
        key.verify_policy_approval(TEST_NOW, &validated, &bad_signature),
        Err(AuthorityError::InvalidSignature)
    );
    assert_eq!(
        key.verify_policy_approval(TEST_NOW, &validated, &signature[..63]),
        Err(AuthorityError::InvalidSignatureLength)
    );
}

#[test]
fn snapshot_signatures_verify_and_each_field_is_bound() {
    let signer = snapshot_signer();
    let key = trusted_snapshot_key(&signer);
    let claims = snapshot_claims(signer.key_id());
    let validated = validate_snapshot(claims.clone());
    let signature = signer.sign_snapshot_descriptor(&validated).unwrap();

    assert_eq!(
        key.verify_snapshot_descriptor(TEST_NOW, &validated, &signature),
        Ok(())
    );

    let mutations: Vec<SnapshotDescriptorClaimsV1> = vec![
        SnapshotDescriptorClaimsV1 {
            format_version: 2,
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            trust_domain_id: "other-domain".into(),
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            snapshot_id: "snapshot-2".into(),
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            relationship_snapshot_root: [0x33; 32],
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            policy_record_id: "policy-2".into(),
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            policy_hash: [0x44; 32],
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            spicedb_revision: "zed-2".into(),
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            epoch: 2,
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            issued_at_unix_seconds: claims.issued_at_unix_seconds - 1,
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            expires_at_unix_seconds: claims.expires_at_unix_seconds + 1,
            ..claims.clone()
        },
        SnapshotDescriptorClaimsV1 {
            canonicalization_version: 2,
            ..claims.clone()
        },
    ];

    for mutated in mutations {
        if let Ok(mutated) = ValidatedSnapshotDescriptorClaimsV1::new(mutated, snapshot_limits()) {
            assert_eq!(
                key.verify_snapshot_descriptor(TEST_NOW, &mutated, &signature),
                Err(AuthorityError::InvalidSignature)
            );
        }
    }

    let mut wrong_key_id = claims;
    wrong_key_id.signing_key_id =
        "ed25519:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    assert_eq!(
        key.verify_snapshot_descriptor(TEST_NOW, &validate_snapshot(wrong_key_id), &signature,),
        Err(AuthorityError::InvalidKeyId)
    );
}

#[test]
fn role_key_id_status_and_time_fail_closed() {
    let policy_signer = policy_signer();
    let snapshot_signer = snapshot_signer();
    let policy_claims = policy_claims(policy_signer.key_id());
    let policy_claims = validate_policy(policy_claims);
    let signature = policy_signer.sign_policy_approval(&policy_claims).unwrap();

    assert_eq!(
        trusted_snapshot_key(&snapshot_signer).verify_policy_approval(
            TEST_NOW,
            &policy_claims,
            &signature,
        ),
        Err(AuthorityError::InvalidKeyId)
    );

    let mut wrong_role = policy_record(&policy_signer);
    wrong_role.role = AuthorityRole::Snapshot;
    let wrong_role = TrustedAuthorityKey::from_record(wrong_role, "raziel-test").unwrap();
    assert_eq!(
        wrong_role.verify_policy_approval(TEST_NOW, &policy_claims, &signature),
        Err(AuthorityError::WrongRole)
    );

    let mut revoked = policy_record(&policy_signer);
    revoked.status = AuthorityKeyStatus::Revoked;
    revoked.revoked_at_unix_seconds = Some(TEST_NOW);
    revoked.revocation_reason = Some("test revocation".into());
    let revoked = TrustedAuthorityKey::from_record(revoked, "raziel-test").unwrap();
    assert_eq!(
        revoked.verify_policy_approval(TEST_NOW, &policy_claims, &signature),
        Err(AuthorityError::RevokedKey)
    );

    assert_eq!(
        trusted_policy_key(&policy_signer).verify_policy_approval(
            TEST_NOW + 3_600,
            &policy_claims,
            &signature,
        ),
        Err(AuthorityError::InactiveKey)
    );
}

#[test]
fn snapshot_publisher_allocates_epochs_from_trusted_state() {
    let signer = snapshot_signer();
    let trusted_key = trusted_snapshot_key(&signer);
    let mut publisher = SnapshotAuthorityPublisher::new(signer, "raziel-test".into(), []).unwrap();
    let draft = snapshot_draft([0x11; 32]);

    let first = publisher
        .sign_next(0, draft.clone(), snapshot_limits())
        .unwrap();
    assert_eq!(first.claims.claims().epoch, 1);
    assert_eq!(
        trusted_key
            .verify_snapshot_descriptor(TEST_NOW, &first.claims, &first.authority_signature,),
        Ok(())
    );
    assert_eq!(
        publisher.sign_next(0, draft.clone(), snapshot_limits()),
        Err(AuthorityError::SnapshotEpochMismatch)
    );

    let second = publisher.sign_next(1, draft, snapshot_limits()).unwrap();
    assert_eq!(second.claims.claims().epoch, 2);

    let other_policy = publisher
        .sign_next(0, snapshot_draft([0x44; 32]), snapshot_limits())
        .unwrap();
    assert_eq!(other_policy.claims.claims().epoch, 1);
}

#[test]
fn snapshot_publisher_does_not_advance_on_failure_or_overflow() {
    let mut publisher =
        SnapshotAuthorityPublisher::new(snapshot_signer(), "raziel-test".into(), []).unwrap();
    let mut invalid = snapshot_draft([0x55; 32]);
    invalid.expires_at_unix_seconds = invalid.issued_at_unix_seconds;
    assert!(matches!(
        publisher.sign_next(0, invalid, snapshot_limits()),
        Err(AuthorityError::Snapshot(_))
    ));
    assert_eq!(publisher.high_watermark(&[0x55; 32]), 0);

    let mut overflow = SnapshotAuthorityPublisher::new(
        snapshot_signer(),
        "raziel-test".into(),
        [([0x66; 32], u64::MAX)],
    )
    .unwrap();
    assert_eq!(
        overflow.sign_next(u64::MAX, snapshot_draft([0x66; 32]), snapshot_limits(),),
        Err(AuthorityError::SnapshotEpochOverflow)
    );
}

#[test]
fn signer_debug_never_exposes_private_key_material() {
    let signer = policy_signer();
    let debug = format!("{signer:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains(&"01".repeat(32)));
}

#[test]
fn malformed_pkcs8_and_wrong_algorithm_fail_closed() {
    assert!(matches!(
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&[0x30, 0x00]),
        Err(AuthorityError::InvalidPkcs8)
    ));

    let mut wrong_algorithm = test_pkcs8(0x01);
    wrong_algorithm[12] ^= 1;
    assert!(matches!(
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&wrong_algorithm),
        Err(AuthorityError::InvalidPkcs8)
    ));
}

#[test]
fn weak_public_keys_fail_closed() {
    let signer = policy_signer();
    let mut record = policy_record(&signer);
    record.public_key = [0; 32];
    record.key_id = raziel_authority::derive_key_id(&record.public_key);
    assert!(matches!(
        TrustedAuthorityKey::from_record(record, "raziel-test"),
        Err(AuthorityError::WeakPublicKey)
    ));
}

#[cfg(unix)]
#[test]
fn key_file_loader_requires_owner_read_only_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let path = std::env::temp_dir().join(format!(
        "raziel-authority-key-{}-{}.pk8",
        std::process::id(),
        TEST_NOW
    ));
    std::fs::write(&path, test_pkcs8(0x01)).unwrap();

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        AuthoritySigner::<PolicyApprovalRole>::from_read_only_pkcs8_file(&path),
        Err(AuthorityError::KeyFileWritable)
    ));

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o440)).unwrap();
    assert!(matches!(
        AuthoritySigner::<PolicyApprovalRole>::from_read_only_pkcs8_file(&path),
        Err(AuthorityError::KeyFileWritable)
    ));

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    let effectively_writable = std::fs::OpenOptions::new().write(true).open(&path).is_ok();
    let result = AuthoritySigner::<PolicyApprovalRole>::from_read_only_pkcs8_file(&path);
    if effectively_writable {
        assert!(matches!(result, Err(AuthorityError::KeyFileWritable)));
    } else {
        assert!(result.is_ok());
    }

    let link = path.with_extension("link");
    symlink(&path, &link).unwrap();
    assert!(matches!(
        AuthoritySigner::<PolicyApprovalRole>::from_read_only_pkcs8_file(&link),
        Err(AuthorityError::KeyFileNotRegular)
    ));
    std::fs::remove_file(link).unwrap();
    std::fs::remove_file(path).unwrap();
}

fn policy_signer() -> AuthoritySigner<PolicyApprovalRole> {
    AuthoritySigner::from_pkcs8_der(&test_pkcs8(0x01)).unwrap()
}

fn snapshot_signer() -> AuthoritySigner<SnapshotRole> {
    AuthoritySigner::from_pkcs8_der(&test_pkcs8(0x02)).unwrap()
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
}

fn validate_policy(claims: PolicyApprovalClaimsV1) -> ValidatedPolicyApprovalClaimsV1 {
    ValidatedPolicyApprovalClaimsV1::new(claims, policy_limits()).unwrap()
}

const fn policy_limits() -> PolicyApprovalLimits {
    PolicyApprovalLimits {
        max_string_bytes: 128,
        max_lifetime_seconds: 3_600,
    }
}

fn validate_snapshot(claims: SnapshotDescriptorClaimsV1) -> ValidatedSnapshotDescriptorClaimsV1 {
    ValidatedSnapshotDescriptorClaimsV1::new(claims, snapshot_limits()).unwrap()
}

const fn snapshot_limits() -> SnapshotDescriptorLimits {
    SnapshotDescriptorLimits {
        max_string_bytes: 128,
        max_lifetime_seconds: 900,
    }
}

fn policy_claims(key_id: &str) -> PolicyApprovalClaimsV1 {
    PolicyApprovalClaimsV1 {
        format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
        trust_domain_id: "raziel-test".into(),
        record_id: "policy-1".into(),
        policy_name: "reconciliation".into(),
        policy_version: "v1".into(),
        approval_epoch: 1,
        policy_hash: [0x11; 32],
        valid_from_unix_seconds: TEST_NOW - 60,
        valid_until_unix_seconds: TEST_NOW + 1_800,
        approved_by: "test-maintainer".into(),
        signing_key_id: key_id.into(),
    }
}

fn snapshot_claims(key_id: &str) -> SnapshotDescriptorClaimsV1 {
    SnapshotDescriptorClaimsV1 {
        format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
        trust_domain_id: "raziel-test".into(),
        snapshot_id: "snapshot-1".into(),
        relationship_snapshot_root: [0x22; 32],
        policy_record_id: "policy-1".into(),
        policy_hash: [0x11; 32],
        spicedb_revision: "zed-1".into(),
        epoch: 1,
        issued_at_unix_seconds: TEST_NOW,
        expires_at_unix_seconds: TEST_NOW + 900,
        canonicalization_version: RELATIONSHIP_TREE_VERSION,
        signing_key_id: key_id.into(),
    }
}

fn snapshot_draft(policy_hash: [u8; 32]) -> SnapshotDescriptorDraftV1 {
    SnapshotDescriptorDraftV1 {
        snapshot_id: "snapshot-published".into(),
        relationship_snapshot_root: [0x22; 32],
        policy_record_id: "policy-1".into(),
        policy_hash,
        spicedb_revision: "zed-published".into(),
        issued_at_unix_seconds: TEST_NOW,
        expires_at_unix_seconds: TEST_NOW + 900,
        canonicalization_version: RELATIONSHIP_TREE_VERSION,
    }
}

fn policy_record(signer: &AuthoritySigner<PolicyApprovalRole>) -> AuthorityKeyRecordV1 {
    AuthorityKeyRecordV1 {
        format_version: AUTHORITY_KEY_FORMAT_VERSION,
        trust_domain_id: "raziel-test".into(),
        key_id: signer.key_id().into(),
        role: AuthorityRole::PolicyApproval,
        public_key: signer.public_key_bytes(),
        status: AuthorityKeyStatus::Active,
        valid_from_unix_seconds: TEST_NOW - 3_600,
        valid_until_unix_seconds: TEST_NOW + 3_600,
        revoked_at_unix_seconds: None,
        revocation_reason: None,
        registry_generation: 1,
    }
}

fn trusted_policy_key(signer: &AuthoritySigner<PolicyApprovalRole>) -> TrustedAuthorityKey {
    TrustedAuthorityKey::from_record(policy_record(signer), "raziel-test").unwrap()
}

fn trusted_snapshot_key(signer: &AuthoritySigner<SnapshotRole>) -> TrustedAuthorityKey {
    TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            key_id: signer.key_id().into(),
            role: AuthorityRole::Snapshot,
            public_key: signer.public_key_bytes(),
            status: AuthorityKeyStatus::Active,
            valid_from_unix_seconds: TEST_NOW - 3_600,
            valid_until_unix_seconds: TEST_NOW + 3_600,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        },
        "raziel-test",
    )
    .unwrap()
}
