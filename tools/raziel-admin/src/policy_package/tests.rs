use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use prost::Message;
use raziel_authority::{AuthoritySigner, PolicyApprovalRole};
use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_canonical::{
    PolicyApprovalClaimsV1, PolicyApprovalLimits, ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_proto::raziel::v1 as proto;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::wire::StoredPackage;
use super::*;
use crate::io::atomic_write;
use crate::policy_authority::PolicyCandidateManifestWire;

#[path = "retained_tests.rs"]
mod retained_tests;

const NOW: i64 = 1_800_000_000;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn identity_and_key_bounds_reject_aliases() {
    for bad in [
        "".into(),
        "A".repeat(32),
        "g".repeat(32),
        "a".repeat(31),
        "a".repeat(33),
        format!("{}\n", "a".repeat(32)),
        "../package".into(),
    ] {
        assert!(PolicyVersionIdentity::new(bad.clone(), "b".repeat(32), 1).is_err());
        assert!(PolicyVersionIdentity::new("a".repeat(32), bad.clone(), 1).is_err());
        assert!(PolicyPackageKey::new(bad, 1).is_err());
    }
    for version in [0, i32::MAX as u32 + 1] {
        assert!(PolicyVersionIdentity::new("a".repeat(32), "b".repeat(32), version).is_err());
    }
    let id = PolicyVersionIdentity::new("0".repeat(32), "f".repeat(32), i32::MAX as u32).unwrap();
    assert!(id.signed_policy_name().len() <= 128);
    assert!(PolicyPackageKey::new("a".repeat(32), 0).is_err());
}

#[test]
fn exact_artifacts_survive_candidate_cleanup_and_retry_after_registry_advance() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    let package = store.retain(a.input(), NOW).unwrap();
    let key = package.key().clone();
    assert_eq!(package.source_schema(), a.source);
    assert_eq!(package.compiler_config(), a.config.as_bytes());
    assert_eq!(package.policy_pb(), a.policy.encode_to_vec());
    assert_eq!(
        package.inventory().definitions(),
        &["agent".to_owned(), a.identity.resource_type()]
    );
    assert!(package.inventory().caveats().is_empty());
    assert_eq!(
        package.approval_record().policy_name,
        a.identity.signed_policy_name()
    );
    assert!(!format!("{package:?}").contains("definition"));
    let bytes = fs::read(f.package_path(&key)).unwrap();
    assert_eq!(
        fs::metadata(f.package_path(&key))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    fs::remove_dir_all(f.root.join("candidates")).unwrap();
    f.publish(&[&a], 2, false);
    store.retain(a.input(), NOW + 20).unwrap();
    store.read_approved(&key, NOW + 20).unwrap();
    assert_eq!(fs::read(f.package_path(&key)).unwrap(), bytes);
}

#[test]
fn versions_coexist_and_reapproval_supersedes_only_its_own_signed_name() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    let b = Artifacts::new('c', 2, 1);
    f.publish(&[&a, &b], 1, false);
    let mut store = f.store();
    let key_a = store.retain(a.input(), NOW).unwrap().key().clone();
    let key_b = store.retain(b.input(), NOW).unwrap().key().clone();
    let mut renewed = a.clone();
    renewed.manifest.expected_approval_epoch = 2;
    renewed.manifest.record_id = "policy-reapproved".into();
    renewed.refresh();
    f.publish(&[&a, &b, &renewed], 2, false);
    let key_new = store.retain(renewed.input(), NOW).unwrap().key().clone();
    assert_eq!(
        store.read_approved(&key_a, NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
    store.read_approved(&key_b, NOW).unwrap();
    store.read_approved(&key_new, NOW).unwrap();
    assert!(f.package_path(&key_a).exists());
}

#[test]
fn dataset_binding_uses_the_approved_package_identity() {
    use crate::dataset_binding::{DatasetBinding, FinalizedDatasetRevision};
    let fixture = Fixture::new();
    let artifacts = Artifacts::new('b', 1, 1);
    fixture.publish(&[&artifacts], 1, false);
    let package = fixture.store().retain(artifacts.input(), NOW).unwrap();
    let binding = DatasetBinding::for_approved_policy(
        &package,
        "scope:processor-v1".into(),
        FinalizedDatasetRevision {
            dataset_id: "processor-ledger".into(),
            revision: 1,
            checksum: [0x33; 32],
        },
    )
    .unwrap();
    assert_eq!(
        binding.policy_hash,
        crate::io::lowercase_hex(package.policy_hash())
    );
    assert_eq!(binding.resource_type, artifacts.identity.resource_type());
    assert_eq!(binding.dataset_revision, "1");
    assert_eq!(binding.action, "query");
}

#[test]
fn expired_revoked_or_missing_approval_never_authorizes_a_retained_package() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    let mut store = f.store();
    f.publish(&[], 1, false);
    assert_eq!(
        store.retain(a.input(), NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
    f.publish(&[&a], 2, false);
    let key = store.retain(a.input(), NOW).unwrap().key().clone();
    assert_eq!(
        store.read_approved(&key, NOW + 5_000).unwrap_err(),
        PackageError::RegistryRejected
    );
    f.publish(&[&a], 3, true);
    assert_eq!(
        f.store().read_approved(&key, NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
    assert!(f.package_path(&key).exists());
}

#[test]
fn stored_signature_inventory_and_unknown_fields_are_not_trusted() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    let key = store.retain(a.input(), NOW).unwrap().key().clone();
    let path = f.package_path(&key);
    let original = fs::read(&path).unwrap();
    let mut stored = StoredPackage::decode(original.as_slice()).unwrap();
    stored.definitions.push("other_version".into());
    atomic_write(&path, &stored.encode_to_vec()).unwrap();
    assert!(store.read_approved(&key, NOW).is_err());
    stored = StoredPackage::decode(original.as_slice()).unwrap();
    let mut approval = proto::PolicyRecord::decode(stored.approval_record.as_slice()).unwrap();
    approval.authority_signature[0] ^= 1;
    stored.approval_record = approval.encode_to_vec();
    atomic_write(&path, &stored.encode_to_vec()).unwrap();
    assert_eq!(
        store.read_approved(&key, NOW).unwrap_err(),
        PackageError::RegistryRejected
    );
    let mut extra = original;
    extra.extend_from_slice(&[0xa0, 0x06, 0x01]);
    atomic_write(&path, &extra).unwrap();
    assert_eq!(
        store.read_approved(&key, NOW).unwrap_err(),
        PackageError::InvalidPackage
    );
}

#[test]
fn changed_source_manifest_policy_or_compiler_config_is_rejected() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    let mut changed = a.clone();
    changed.source.push(b'\n');
    assert!(store.retain(changed.input(), NOW).is_err());
    changed = a.clone();
    changed.manifest.policy_name = "display-alias".into();
    changed.refresh();
    assert!(store.retain(changed.input(), NOW).is_err());
    for config in [
        a.config.replace(
            "\"config_version\":1",
            "\"config_version\":1,\"config_version\":1",
        ),
        a.config.replace("4096", "4097"),
        a.config
            .replace("\"caveats\":{}", "\"caveats\":{},\"extra\":true"),
    ] {
        changed = a.clone();
        changed.config = config;
        assert!(store.retain(changed.input(), NOW).is_err());
    }
    changed = a.clone();
    changed.policy.compiler_version = "other".into();
    changed.refresh();
    assert!(store.retain(changed.input(), NOW).is_err());
    assert_eq!(fs::read_dir(&f.packages).unwrap().count(), 0);
}

#[test]
fn typed_inventory_rejects_cross_version_definitions_and_references() {
    let mut a = Artifacts::new('b', 1, 1);
    let original = a.clone();
    for other in ["dataset".to_owned(), format!("dataset_{}", "c".repeat(32))] {
        a = original.clone();
        a.policy.definitions[1].name = other.clone();
        a.policy.permissions[0].object_type = other;
        a.refresh();
        assert!(validation::artifacts(&a.input()).is_err());
    }
    a = original.clone();
    a.policy.definitions[0].relations = a.policy.definitions[1].relations.clone();
    a.refresh();
    assert!(validation::artifacts(&a.input()).is_err());
    a = original.clone();
    a.policy.definitions[1].relations[0].allowed_subject_types[0].object_type =
        format!("team_{}", "c".repeat(32));
    // An unresolved cross-version reference is rejected by strict PolicyIR decoding.
    a.policy_bytes = a.policy.encode_to_vec();
    assert!(validation::artifacts(&a.input()).is_err());
}

#[test]
fn caveat_inventory_and_parameter_provenance_must_match_typed_ir() {
    let mut a = Artifacts::new('b', 1, 1);
    let name = format!("caveat_{}_0", a.identity.version_id());
    a.source = format!(
        "caveat {name}(flag bool) {{ flag }}\n\ndefinition agent {{}}\n\ndefinition {} {{\n  relation viewer: agent with {name}\n  permission query = viewer\n}}\n",
        a.identity.resource_type(),
    ).into_bytes();
    a.policy.source_schema_digest = Sha256::digest(&a.source).to_vec();
    a.manifest.source_schema_digest = a.policy.source_schema_digest.clone();
    a.policy.caveats = vec![proto::CaveatDefinition {
        name: name.clone(),
        parameters: vec![proto::CaveatParameter {
            source: proto::CaveatInputSource::RelationshipContext as i32,
            name: "flag".into(),
            value_type: proto::CaveatValueType::Bool as i32,
        }],
        expression: Some(proto::CaveatExpr {
            node: Some(proto::caveat_expr::Node::InputRef(proto::CaveatInputRef {
                source: proto::CaveatInputSource::RelationshipContext as i32,
                name: "flag".into(),
                value_type: proto::CaveatValueType::Bool as i32,
            })),
        }),
    }];
    a.policy.definitions[1].relations[0].allowed_subject_types[0].caveat_name = Some(name.clone());
    a.policy.permissions[0].expression = Some(proto::PermissionExpr {
        node: Some(proto::permission_expr::Node::CaveatedRelation(
            proto::PermissionCaveatedRelation {
                relation: "viewer".into(),
                caveat_name: name.clone(),
            },
        )),
    });
    a.config = json!({"config_version":1,"limits":TRUSTED_POLICY_CEILINGS.policy_limits,"caveats":{name.clone():{"flag":"RELATIONSHIP_CONTEXT"}}}).to_string();
    a.refresh();
    let f = Fixture::new();
    f.publish(&[&a], 1, false);
    assert_eq!(
        f.store()
            .retain(a.input(), NOW)
            .unwrap()
            .inventory()
            .caveats(),
        &[name]
    );
    for bad in [
        a.config.replace("RELATIONSHIP_CONTEXT", "REQUEST_CONTEXT"),
        a.config.replace(
            "\"flag\":\"RELATIONSHIP_CONTEXT\"",
            "\"flag\":\"RELATIONSHIP_CONTEXT\",\"flag\":\"RELATIONSHIP_CONTEXT\"",
        ),
    ] {
        assert!(
            validation::artifacts(&PolicyPackageInput {
                compiler_config: bad.as_bytes(),
                ..a.input()
            })
            .is_err()
        );
    }
    let foreign_name = format!("caveat_{}_0", "c".repeat(32));
    a.policy.caveats[0].name = foreign_name.clone();
    a.policy.definitions[1].relations[0].allowed_subject_types[0].caveat_name =
        Some(foreign_name.clone());
    a.policy.permissions[0].expression = Some(proto::PermissionExpr {
        node: Some(proto::permission_expr::Node::CaveatedRelation(
            proto::PermissionCaveatedRelation {
                relation: "viewer".into(),
                caveat_name: foreign_name,
            },
        )),
    });
    a.refresh();
    assert!(matches!(
        validation::artifacts(&a.input()),
        Err(PackageError::NamespaceRejected)
    ));
}

#[test]
fn exact_approval_retry_rejects_changed_candidate_metadata() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    store.retain(a.input(), NOW).unwrap();
    let mut changed = a.clone();
    changed.manifest.candidate_id = "different-candidate".into();
    changed.refresh();
    assert_eq!(
        store.retain(changed.input(), NOW).unwrap_err(),
        PackageError::Conflict
    );
}

#[test]
fn one_family_version_cannot_acquire_a_different_version_id() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    store.retain(a.input(), NOW).unwrap();
    let other = Artifacts::new('c', 1, 2);
    f.publish(&[&a, &other], 2, false);
    assert_eq!(
        store.retain(other.input(), NOW).unwrap_err(),
        PackageError::Conflict
    );
    assert_eq!(fs::read_dir(&f.packages).unwrap().count(), 1);
}

#[test]
fn private_files_reject_modes_symlinks_hardlinks_and_oversize() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    f.publish(&[&a], 1, false);
    let mut store = f.store();
    let key = store.retain(a.input(), NOW).unwrap().key().clone();
    let path = f.package_path(&key);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.read_approved(&key, NOW).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let copy = f.root.join("linked.pb");
    fs::hard_link(&path, &copy).unwrap();
    assert!(store.read_approved(&key, NOW).is_err());
    fs::remove_file(&copy).unwrap();
    fs::rename(&path, &copy).unwrap();
    symlink(&copy, &path).unwrap();
    assert!(store.read_approved(&key, NOW).is_err());
    fs::remove_file(&path).unwrap();
    fs::rename(&copy, &path).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_PACKAGE_BYTES + 1)
        .unwrap();
    assert!(store.read_approved(&key, NOW).is_err());
    fs::set_permissions(&f.packages, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        PolicyPackageStore::new(f.packages.clone(), f.registry.clone(), "package-test").is_err()
    );
}

#[test]
fn input_size_limits_fail_before_publication() {
    let f = Fixture::new();
    let a = Artifacts::new('b', 1, 1);
    let oversized_source = vec![b'x'; MAX_SOURCE_BYTES + 1];
    let oversized_config = vec![b' '; MAX_COMPILER_CONFIG_BYTES + 1];
    for input in [
        PolicyPackageInput {
            source_schema: &oversized_source,
            ..a.input()
        },
        PolicyPackageInput {
            compiler_config: &oversized_config,
            ..a.input()
        },
    ] {
        assert_eq!(
            f.store().retain(input, NOW).unwrap_err(),
            PackageError::InvalidPackage
        );
    }
    assert_eq!(fs::read_dir(&f.packages).unwrap().count(), 0);
}

#[derive(Clone)]
struct Artifacts {
    identity: PolicyVersionIdentity,
    source: Vec<u8>,
    config: String,
    policy: proto::PolicyProgram,
    policy_bytes: Vec<u8>,
    manifest: PolicyCandidateManifestWire,
    manifest_bytes: Vec<u8>,
}

impl Artifacts {
    fn new(id: char, version: u32, epoch: u64) -> Self {
        let identity =
            PolicyVersionIdentity::new("a".repeat(32), id.to_string().repeat(32), version).unwrap();
        let resource = identity.resource_type();
        let source = format!("definition agent {{}}\n\ndefinition {resource} {{\n  relation viewer: agent\n  permission query = viewer\n}}\n").into_bytes();
        let l = TRUSTED_POLICY_CEILINGS.policy_limits;
        let policy = proto::PolicyProgram {
            ir_version: 1,
            compiler_name: "authzed/spicedb".into(),
            compiler_version: "1.54.0".into(),
            source_schema_digest: Sha256::digest(&source).to_vec(),
            definitions: vec![
                proto::ObjectDefinition {
                    name: "agent".into(),
                    relations: vec![],
                },
                proto::ObjectDefinition {
                    name: resource.clone(),
                    relations: vec![proto::RelationDefinition {
                        name: "viewer".into(),
                        allowed_subject_types: vec![proto::AllowedSubjectType {
                            object_type: "agent".into(),
                            subject_relation: None,
                            caveat_name: None,
                        }],
                    }],
                },
            ],
            permissions: vec![proto::PermissionDefinition {
                object_type: resource.clone(),
                name: "query".into(),
                expression: Some(proto::PermissionExpr {
                    node: Some(proto::permission_expr::Node::RelationRef(
                        proto::PermissionRelationRef {
                            name: "viewer".into(),
                        },
                    )),
                }),
            }],
            caveats: vec![],
            limits: Some(proto::PolicyLimits {
                max_expression_nodes: l.max_expression_nodes,
                max_caveat_nodes: l.max_caveat_nodes,
                max_path_depth: l.max_path_depth,
                max_relationship_witnesses: l.max_relationship_witnesses,
                max_merkle_depth: l.max_merkle_depth,
                max_string_bytes: l.max_string_bytes,
                max_context_entries: l.max_context_entries,
                max_total_private_input_bytes: l.max_total_private_input_bytes,
            }),
        };
        let manifest = PolicyCandidateManifestWire {
            format_version: 1,
            candidate_id: format!("candidate-{id}-{epoch}"),
            record_id: format!("policy-{id}-{epoch}"),
            policy_name: identity.signed_policy_name(),
            policy_version: version.to_string(),
            resource_type: resource,
            permission: "query".into(),
            rule_summary: "viewer".into(),
            source_schema_digest: policy.source_schema_digest.clone(),
            policy_hash: vec![],
            expected_approval_epoch: epoch,
            valid_from_unix_seconds: NOW - 100,
            valid_until_unix_seconds: NOW + 5_000,
            created_at_unix_seconds: NOW - 10,
            expires_at_unix_seconds: NOW + 10,
        };
        let mut a = Self {
            identity,
            source,
            config: json!({"config_version":1,"limits":l,"caveats":{}}).to_string(),
            policy,
            policy_bytes: vec![],
            manifest,
            manifest_bytes: vec![],
        };
        a.refresh();
        a
    }

    fn refresh(&mut self) {
        let p = ValidatedPolicyProgram::from_trusted_proto(
            self.policy.clone(),
            &TRUSTED_POLICY_CEILINGS,
        )
        .unwrap();
        self.manifest.policy_hash = policy_hash(&p).unwrap().to_vec();
        self.policy_bytes = self.policy.encode_to_vec();
        self.manifest_bytes = self.manifest.encode_to_vec();
    }

    fn input(&self) -> PolicyPackageInput<'_> {
        PolicyPackageInput {
            identity: self.identity.clone(),
            candidate_manifest: &self.manifest_bytes,
            source_schema: &self.source,
            compiler_config: self.config.as_bytes(),
            policy_pb: &self.policy_bytes,
        }
    }

    fn record(
        &self,
        signer: &AuthoritySigner<PolicyApprovalRole>,
        generation: u64,
        revoked: bool,
    ) -> proto::PolicyRecord {
        let m = &self.manifest;
        let claims = ValidatedPolicyApprovalClaimsV1::new(
            PolicyApprovalClaimsV1 {
                format_version: 1,
                trust_domain_id: "package-test".into(),
                record_id: m.record_id.clone(),
                policy_name: m.policy_name.clone(),
                policy_version: m.policy_version.clone(),
                approval_epoch: m.expected_approval_epoch,
                policy_hash: m.policy_hash.as_slice().try_into().unwrap(),
                valid_from_unix_seconds: m.valid_from_unix_seconds,
                valid_until_unix_seconds: m.valid_until_unix_seconds,
                approved_by: "owner".into(),
                signing_key_id: signer.key_id().into(),
            },
            PolicyApprovalLimits {
                max_string_bytes: 512,
                max_lifetime_seconds: 31 * 24 * 60 * 60,
            },
        )
        .unwrap();
        proto::PolicyRecord {
            format_version: 1,
            record_id: m.record_id.clone(),
            policy_name: m.policy_name.clone(),
            policy_version: m.policy_version.clone(),
            canonical_policy_ir: Some(self.policy.clone()),
            policy_hash: m.policy_hash.clone(),
            status: if revoked {
                proto::PolicyStatus::Revoked
            } else {
                proto::PolicyStatus::Approved
            } as i32,
            valid_from: Some(timestamp(m.valid_from_unix_seconds)),
            valid_until: Some(timestamp(m.valid_until_unix_seconds)),
            approved_by: "owner".into(),
            signing_key_id: signer.key_id().into(),
            authority_signature: signer.sign_policy_approval(&claims).unwrap().to_vec(),
            registered_at: Some(timestamp(NOW)),
            trust_domain_id: "package-test".into(),
            approval_epoch: m.expected_approval_epoch,
            revoked_at: revoked.then(|| timestamp(NOW)),
            revocation_reason: String::new(),
            registry_generation: generation,
        }
    }
}

struct Fixture {
    root: PathBuf,
    registry: PathBuf,
    packages: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "raziel-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        private_dir(&root);
        let registry = root.join("registry");
        let packages = root.join("packages");
        private_dir(&registry);
        private_dir(&packages);
        private_dir(&root.join("candidates"));
        Self {
            root,
            registry,
            packages,
        }
    }
    fn store(&self) -> PolicyPackageStore {
        PolicyPackageStore::new(self.packages.clone(), self.registry.clone(), "package-test")
            .unwrap()
    }
    fn package_path(&self, key: &PolicyPackageKey) -> PathBuf {
        self.packages
            .join(format!("{}.{}.pb", key.version_id(), key.approval_epoch()))
    }
    fn publish(&self, artifacts: &[&Artifacts], generation: u64, revoked: bool) {
        let der = [
            vec![
                0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
                0x04, 0x20,
            ],
            vec![0x41; 32],
        ]
        .concat();
        let signer = AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&der).unwrap();
        let key = proto::AuthorityKeyRecord {
            format_version: 1,
            trust_domain_id: "package-test".into(),
            key_id: signer.key_id().into(),
            role: proto::AuthorityRole::PolicyApproval as i32,
            public_key: signer.public_key_bytes().to_vec(),
            status: proto::AuthorityKeyStatus::Active as i32,
            valid_from: Some(timestamp(NOW - 1_000)),
            valid_until: Some(timestamp(NOW + 10_000)),
            revoked_at: None,
            revocation_reason: String::new(),
            registry_generation: generation,
        };
        let key_file = self.write("key.pb", &key.encode_to_vec());
        let policies = artifacts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                self.write(
                    &format!("policy-{i:03}.pb"),
                    &a.record(&signer, generation, revoked).encode_to_vec(),
                )
            })
            .collect();
        let manifest = proto::RegistryGenerationManifest {
            format_version: 1,
            trust_domain_id: "package-test".into(),
            generation,
            published_at: Some(timestamp(NOW)),
            authority_keys: vec![key_file],
            policies,
            accepted_roots: vec![],
        };
        atomic_write(&self.registry.join("current.pb"), &manifest.encode_to_vec()).unwrap();
    }
    fn write(&self, name: &str, bytes: &[u8]) -> proto::RegistryFile {
        atomic_write(&self.registry.join(name), bytes).unwrap();
        proto::RegistryFile {
            relative_path: name.into(),
            sha256: Sha256::digest(bytes).to_vec(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
}
