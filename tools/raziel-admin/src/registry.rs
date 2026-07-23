use std::path::{Path, PathBuf};

use prost::Message;
use raziel_proto::raziel::v1 as proto;
use raziel_verifier::{
    LocalRegistry, RegistryError, RegistryLoader, authority_key_from_proto, authority_key_to_proto,
    decode_accepted_root_record_strict, decode_authority_key_record_strict,
    decode_policy_record_strict, policy_record_from_proto, policy_record_to_proto,
    root_record_from_proto, root_record_to_proto,
};
use sha2::{Digest, Sha256};

use crate::{
    args::Arguments,
    config::{MAX_RECORD_BYTES, registry_config},
    error::AdminError,
    io::{atomic_write, atomic_write_new, read_bounded, sync_directory},
    projection,
};

pub fn publish(arguments: &Arguments) -> Result<(), AdminError> {
    arguments.require_known(&[
        "--root",
        "--trust-domain",
        "--generation",
        "--published-at",
        "--observed-now",
        "--output-directory",
        "--output-manifest",
        "--output-json",
        "--previous-manifest",
        "--authority-key",
        "--policy",
        "--root-record",
    ])?;
    let root = arguments.required_path("--root")?;
    let trust_domain_id = arguments.required("--trust-domain")?;
    let generation = arguments.required_u64("--generation")?;
    let published_at = arguments.required_i64("--published-at")?;
    let observed_now = arguments.required_i64("--observed-now")?;
    let output_directory = arguments.required("--output-directory")?;
    validate_relative_path(output_directory)?;
    let output_manifest = arguments.required_path("--output-manifest")?;
    let output_json = arguments.required_path("--output-json")?;
    let previous_manifest = arguments
        .optional("--previous-manifest")?
        .map(PathBuf::from);
    let config = registry_config(trust_domain_id)?;

    let loader = RegistryLoader::new(config.clone());
    let mut registry = LocalRegistry::new(config.clone());
    match previous_manifest {
        Some(previous_manifest) => {
            let previous_bytes = read_bounded(&previous_manifest, MAX_RECORD_BYTES)?;
            let previous = loader.load_candidate(&root, &previous_bytes, observed_now)?;
            let expected = previous
                .generation
                .checked_add(1)
                .ok_or(RegistryError::EpochOverflow)?;
            if generation != expected {
                return Err(RegistryError::RegistryGenerationInvalid.into());
            }
            registry.install_generation(previous)?;
        }
        None if generation != 1 => {
            return Err(RegistryError::RegistryGenerationInvalid.into());
        }
        None => {}
    }

    create_generation_directory(&root, output_directory)?;
    let authority_keys = copy_authority_records(
        &root,
        output_directory,
        sorted(arguments.repeated("--authority-key")),
        generation,
        &config,
    )?;
    let policies = copy_policy_records(
        &root,
        output_directory,
        sorted(arguments.repeated("--policy")),
        generation,
        &config,
    )?;
    let accepted_roots = copy_root_records(
        &root,
        output_directory,
        sorted(arguments.repeated("--root-record")),
        generation,
        observed_now,
        &config,
    )?;
    let manifest = proto::RegistryGenerationManifest {
        format_version: 1,
        trust_domain_id: trust_domain_id.into(),
        generation,
        authority_keys,
        policies,
        accepted_roots,
        published_at: Some(timestamp(published_at)),
    };
    let manifest_bytes = manifest.encode_to_vec();

    let candidate = loader.load_candidate(&root, &manifest_bytes, observed_now)?;
    registry.install_generation(candidate)?;
    atomic_write(&output_manifest, &manifest_bytes)?;
    projection::write_replace(&output_json, &projection::manifest(&manifest))?;

    println!("registry_generation={generation}");
    println!("manifest={}", output_manifest.display());
    Ok(())
}

fn create_generation_directory(root: &Path, relative_path: &str) -> Result<(), AdminError> {
    validate_relative_path(relative_path)?;
    let parts: Vec<_> = relative_path.split('/').collect();
    let mut current = root.to_path_buf();
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        let is_last = index + 1 == parts.len();
        if is_last {
            std::fs::create_dir(&current).map_err(|_| AdminError::FileWrite)?;
            sync_directory(current.parent().ok_or(AdminError::FileWrite)?)?;
            continue;
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RegistryError::SymlinkForbidden.into());
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(RegistryError::FilePathInvalid.into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(|_| AdminError::FileWrite)?;
                sync_directory(current.parent().ok_or(AdminError::FileWrite)?)?;
            }
            Err(_) => return Err(AdminError::FileWrite),
        }
    }
    Ok(())
}

pub fn revoke(arguments: &Arguments) -> Result<(), AdminError> {
    arguments.require_known(&[
        "--kind",
        "--input",
        "--output",
        "--json-output",
        "--trust-domain",
        "--generation",
        "--revoked-at",
        "--reason",
        "--observed-now",
    ])?;
    let kind = arguments.required("--kind")?;
    let input = arguments.required_path("--input")?;
    let output = arguments.required_path("--output")?;
    let json_output = arguments.required_path("--json-output")?;
    let trust_domain_id = arguments.required("--trust-domain")?;
    let generation = arguments.required_u64("--generation")?;
    let revoked_at = arguments.required_i64("--revoked-at")?;
    let reason = arguments.optional("--reason")?.unwrap_or_default();
    let observed_now = arguments.required_i64("--observed-now")?;
    let config = registry_config(trust_domain_id)?;
    let bytes = read_bounded(&input, MAX_RECORD_BYTES)?;

    let (output_bytes, output_projection) = match kind {
        "key" => {
            let mut value = decode_authority_key_record_strict(&bytes, MAX_RECORD_BYTES)?;
            if value.status != proto::AuthorityKeyStatus::Active as i32 {
                return Err(RegistryError::RevocationRollback.into());
            }
            value.status = proto::AuthorityKeyStatus::Revoked as i32;
            value.revoked_at = Some(timestamp(revoked_at));
            value.revocation_reason = reason.into();
            value.registry_generation = generation;
            let output = authority_key_to_proto(&authority_key_from_proto(value, &config)?);
            (output.encode_to_vec(), projection::authority_key(&output))
        }
        "policy" => {
            let mut value = decode_policy_record_strict(&bytes, MAX_RECORD_BYTES)?;
            if value.status != proto::PolicyStatus::Approved as i32 {
                return Err(RegistryError::RevocationRollback.into());
            }
            value.status = proto::PolicyStatus::Revoked as i32;
            value.revoked_at = Some(timestamp(revoked_at));
            value.revocation_reason = reason.into();
            value.registry_generation = generation;
            let output = policy_record_to_proto(&policy_record_from_proto(value, &config)?)?;
            (output.encode_to_vec(), projection::policy(&output))
        }
        "root" => {
            let mut value = decode_accepted_root_record_strict(&bytes, MAX_RECORD_BYTES)?;
            if value.status != proto::SnapshotStatus::Accepted as i32 {
                return Err(RegistryError::RevocationRollback.into());
            }
            value.status = proto::SnapshotStatus::Revoked as i32;
            value.revoked_at = Some(timestamp(revoked_at));
            value.revocation_reason = reason.into();
            value.registry_generation = generation;
            let output =
                root_record_to_proto(&root_record_from_proto(value, &config, observed_now)?);
            (output.encode_to_vec(), projection::accepted_root(&output))
        }
        _ => {
            return Err(AdminError::Usage("--kind must be key, policy, or root"));
        }
    };
    atomic_write_new(&output, &output_bytes)?;
    projection::write_new(&json_output, &output_projection)?;
    println!("revoked_kind={kind}");
    println!("registry_generation={generation}");
    Ok(())
}

fn copy_authority_records(
    root: &Path,
    output_directory: &str,
    inputs: Vec<&str>,
    generation: u64,
    config: &raziel_verifier::RegistryConfig,
) -> Result<Vec<proto::RegistryFile>, AdminError> {
    let mut entries = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.into_iter().enumerate() {
        let bytes = read_relative(root, input)?;
        let mut value = decode_authority_key_record_strict(&bytes, MAX_RECORD_BYTES)?;
        value.registry_generation = generation;
        let value = authority_key_to_proto(&authority_key_from_proto(value, config)?);
        entries.push(write_generation_record(
            root,
            output_directory,
            "authority",
            index,
            &value.encode_to_vec(),
        )?);
    }
    Ok(entries)
}

fn copy_policy_records(
    root: &Path,
    output_directory: &str,
    inputs: Vec<&str>,
    generation: u64,
    config: &raziel_verifier::RegistryConfig,
) -> Result<Vec<proto::RegistryFile>, AdminError> {
    let mut entries = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.into_iter().enumerate() {
        let bytes = read_relative(root, input)?;
        let mut value = decode_policy_record_strict(&bytes, MAX_RECORD_BYTES)?;
        value.registry_generation = generation;
        let value = policy_record_to_proto(&policy_record_from_proto(value, config)?)?;
        entries.push(write_generation_record(
            root,
            output_directory,
            "policy",
            index,
            &value.encode_to_vec(),
        )?);
    }
    Ok(entries)
}

fn copy_root_records(
    root: &Path,
    output_directory: &str,
    inputs: Vec<&str>,
    generation: u64,
    observed_now: i64,
    config: &raziel_verifier::RegistryConfig,
) -> Result<Vec<proto::RegistryFile>, AdminError> {
    let mut entries = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.into_iter().enumerate() {
        let bytes = read_relative(root, input)?;
        let mut value = decode_accepted_root_record_strict(&bytes, MAX_RECORD_BYTES)?;
        value.registry_generation = generation;
        let value = root_record_to_proto(&root_record_from_proto(value, config, observed_now)?);
        entries.push(write_generation_record(
            root,
            output_directory,
            "root",
            index,
            &value.encode_to_vec(),
        )?);
    }
    Ok(entries)
}

fn write_generation_record(
    root: &Path,
    output_directory: &str,
    kind: &str,
    index: usize,
    bytes: &[u8],
) -> Result<proto::RegistryFile, AdminError> {
    let relative_path = format!("{output_directory}/{kind}-{index:05}.pb");
    validate_relative_path(&relative_path)?;
    atomic_write_new(&root.join(&relative_path), bytes)?;
    Ok(proto::RegistryFile {
        relative_path,
        sha256: Sha256::digest(bytes).to_vec(),
    })
}

fn read_relative(root: &Path, relative_path: &str) -> Result<Vec<u8>, AdminError> {
    validate_relative_path(relative_path)?;
    let path = root.join(relative_path);
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| AdminError::FileRead)?;
    if metadata.file_type().is_symlink() {
        return Err(RegistryError::SymlinkForbidden.into());
    }
    read_bounded(&path, MAX_RECORD_BYTES)
}

fn validate_relative_path(value: &str) -> Result<(), AdminError> {
    if value.is_empty()
        || !value.is_ascii()
        || value.contains('\\')
        || value.starts_with('/')
        || value.ends_with('/')
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(RegistryError::FilePathInvalid.into());
    }
    Ok(())
}

fn sorted(mut values: Vec<&str>) -> Vec<&str> {
    values.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    values
}

const fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use prost::Message;
    use raziel_authority::{
        AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
        AuthoritySigner, PolicyApprovalRole, SnapshotRole, TrustedAuthorityKey,
    };
    use raziel_canonical::{
        AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
        ValidatedPolicyApprovalClaimsV1, policy_hash,
    };
    use raziel_policy_ir::decode_policy_program_strict;
    use raziel_snapshot::{
        RELATIONSHIP_TREE_VERSION, SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SnapshotDescriptorClaimsV1,
        SnapshotDescriptorLimits, ValidatedSnapshotDescriptorClaimsV1,
    };
    use raziel_verifier::{
        AcceptedRootRecordV1, LocalRegistry, PolicyRecordStatus, PolicyRegistryRecordV1,
        RegistryLoader, RootRecordStatus, authority_key_to_proto, policy_record_to_proto,
        root_record_to_proto,
    };

    use super::{create_generation_directory, publish, revoke};
    use crate::{
        args::Arguments,
        config::{registry_config, validation_ceilings},
        error::AdminError,
    };

    const TEST_NOW: i64 = 1_700_000_000;
    const POLICY_BYTES: &[u8] =
        include_bytes!("../../spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb");
    const TEST_PKCS8_PREFIX: [u8; 16] = [
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];

    #[test]
    fn generation_directories_are_new_and_immutable() {
        let root = temp_registry_root();

        assert!(create_generation_directory(&root, "generations/000001").is_ok());
        assert!(matches!(
            create_generation_directory(&root, "generations/000001"),
            Err(AdminError::FileWrite)
        ));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn publish_and_revoke_commands_preserve_generation_atomicity() {
        let root = temp_registry_root();
        std::fs::create_dir(root.join("drafts")).unwrap();
        write_draft_records(&root);
        let manifest = root.join("current.pb");
        let manifest_json = root.join("current.json");

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
            manifest.to_str().unwrap(),
            "--output-json",
            manifest_json.to_str().unwrap(),
            "--authority-key",
            "drafts/policy-key.pb",
            "--authority-key",
            "drafts/snapshot-key.pb",
            "--policy",
            "drafts/policy.pb",
            "--root-record",
            "drafts/root.pb",
        ]))
        .unwrap();
        let first_projection: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_json).unwrap()).unwrap();
        assert_eq!(first_projection["generation"], "1");

        let active_policy = root.join("generations/000001/policy-00000.pb");
        let active_bytes = std::fs::read(&active_policy).unwrap();
        assert!(matches!(
            revoke(&arguments([
                "revoke-record",
                "--kind",
                "policy",
                "--input",
                active_policy.to_str().unwrap(),
                "--output",
                active_policy.to_str().unwrap(),
                "--json-output",
                root.join("drafts/rejected.json").to_str().unwrap(),
                "--trust-domain",
                "raziel-test",
                "--generation",
                "2",
                "--revoked-at",
                "1700000000",
                "--reason",
                "operator test",
                "--observed-now",
                "1700000000",
            ])),
            Err(AdminError::FileWrite)
        ));
        assert_eq!(std::fs::read(&active_policy).unwrap(), active_bytes);

        let revoked_draft = root.join("drafts/policy-revoked.pb");
        revoke(&arguments([
            "revoke-record",
            "--kind",
            "policy",
            "--input",
            active_policy.to_str().unwrap(),
            "--output",
            revoked_draft.to_str().unwrap(),
            "--json-output",
            root.join("drafts/policy-revoked.json").to_str().unwrap(),
            "--trust-domain",
            "raziel-test",
            "--generation",
            "2",
            "--revoked-at",
            "1700000000",
            "--reason",
            "operator test",
            "--observed-now",
            "1700000000",
        ]))
        .unwrap();

        publish(&arguments([
            "publish-registry",
            "--root",
            root.to_str().unwrap(),
            "--trust-domain",
            "raziel-test",
            "--generation",
            "2",
            "--published-at",
            "1700000000",
            "--observed-now",
            "1700000000",
            "--output-directory",
            "generations/000002",
            "--output-manifest",
            manifest.to_str().unwrap(),
            "--output-json",
            manifest_json.to_str().unwrap(),
            "--previous-manifest",
            manifest.to_str().unwrap(),
            "--authority-key",
            "drafts/policy-key.pb",
            "--authority-key",
            "drafts/snapshot-key.pb",
            "--policy",
            "drafts/policy-revoked.pb",
            "--root-record",
            "drafts/root.pb",
        ]))
        .unwrap();
        let second_projection: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_json).unwrap()).unwrap();
        assert_eq!(second_projection["generation"], "2");

        let config = registry_config("raziel-test").unwrap();
        let loader = RegistryLoader::new(config.clone());
        let current = std::fs::read(&manifest).unwrap();
        let candidate = loader.load_candidate(&root, &current, TEST_NOW).unwrap();
        let mut registry = LocalRegistry::new(config);
        registry.install_generation(candidate).unwrap();
        assert!(registry.resolve_root(&[0x22; 32], TEST_NOW).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }

    fn write_draft_records(root: &std::path::Path) {
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
        let policy_claims = ValidatedPolicyApprovalClaimsV1::new(
            PolicyApprovalClaimsV1 {
                format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                record_id: "policy-1".into(),
                policy_name: "reconciliation".into(),
                policy_version: "v1".into(),
                approval_epoch: 1,
                policy_hash,
                valid_from_unix_seconds: TEST_NOW - 100,
                valid_until_unix_seconds: TEST_NOW + 100,
                approved_by: "test-maintainer".into(),
                signing_key_id: policy_signer.key_id().into(),
            },
            PolicyApprovalLimits {
                max_string_bytes: 512,
                max_lifetime_seconds: 31 * 24 * 60 * 60,
            },
        )
        .unwrap();
        let policy_record = PolicyRegistryRecordV1 {
            authority_signature: policy_signer.sign_policy_approval(&policy_claims).unwrap(),
            claims: policy_claims,
            policy,
            canonical_policy_ir_bytes: POLICY_BYTES.to_vec(),
            status: PolicyRecordStatus::Approved,
            registered_at_unix_seconds: TEST_NOW,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        };
        let root_claims = ValidatedSnapshotDescriptorClaimsV1::new(
            SnapshotDescriptorClaimsV1 {
                format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                snapshot_id: "snapshot-1".into(),
                relationship_snapshot_root: [0x22; 32],
                policy_record_id: "policy-1".into(),
                policy_hash,
                spicedb_revision: "zed-1".into(),
                epoch: 1,
                issued_at_unix_seconds: TEST_NOW,
                expires_at_unix_seconds: TEST_NOW + 100,
                canonicalization_version: RELATIONSHIP_TREE_VERSION,
                signing_key_id: snapshot_signer.key_id().into(),
            },
            SnapshotDescriptorLimits {
                max_string_bytes: 512,
                max_lifetime_seconds: 31 * 24 * 60 * 60,
            },
        )
        .unwrap();
        let root_record = AcceptedRootRecordV1 {
            authority_signature: snapshot_signer
                .sign_snapshot_descriptor(&root_claims)
                .unwrap(),
            claims: root_claims,
            status: RootRecordStatus::Accepted,
            registered_at_unix_seconds: TEST_NOW,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        };

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
        std::fs::write(
            root.join("drafts/root.pb"),
            root_record_to_proto(&root_record).encode_to_vec(),
        )
        .unwrap();
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
                valid_until_unix_seconds: TEST_NOW + 1_000,
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

    fn temp_registry_root() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "raziel-admin-registry-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }
}
