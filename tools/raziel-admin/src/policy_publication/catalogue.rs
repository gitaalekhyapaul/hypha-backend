//! Read-only owner catalogue. Selection is not request approval or authorization.

use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use raziel_authority::AuthorityError;
use raziel_verifier::{
    LocalRegistry, PolicyRegistryRecordV1, RegistryError, RegistryLoader, policy_record_from_proto,
};
use serde::Serialize;

use crate::config::registry_config;
use crate::io::lowercase_hex;
use crate::policy_package::{
    ApprovedPolicyPackage, PackageError, PolicyPackageKey, PolicyPackageStore,
    RetainedPolicyPackage,
};

use super::{files, installation};

pub struct CatalogueConfig {
    pub publication_root: PathBuf,
    pub package_root: PathBuf,
    pub registry_root: PathBuf,
    pub trust_domain_id: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueProjection {
    pub format_version: u32,
    pub policies: Vec<CataloguePolicy>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueIdentity {
    pub family_id: String,
    pub version_id: String,
    pub version: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CataloguePolicy {
    pub identity: CatalogueIdentity,
    pub record_id: String,
    pub policy_hash: String,
    pub policy_name: String,
    pub policy_version: String,
    pub resource_type: String,
    pub permission: &'static str,
    pub approval_epoch: String,
    pub valid_from_unix_seconds: String,
    pub valid_until_unix_seconds: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogueError {
    Unavailable,
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("policy catalogue is unavailable")
    }
}

impl std::error::Error for CatalogueError {}

pub(crate) enum InstalledApprovalError {
    Conflict,
    Unavailable,
}

/// Keeps publication paused until the caller finishes retaining its binding.
pub(crate) struct InstalledApproval {
    pub package: ApprovedPolicyPackage,
    _publication_lock: File,
}

pub(crate) fn resolve_installed(
    config: &CatalogueConfig,
    key: &PolicyPackageKey,
    record_id: &str,
    trusted_now: i64,
) -> Result<InstalledApproval, InstalledApprovalError> {
    use InstalledApprovalError::{Conflict, Unavailable};

    let lock = publication_read_lock(&config.publication_root).map_err(|_| Unavailable)?;
    let state = installation::load_state(&config.publication_root, "installed.json")
        .map_err(|_| Unavailable)?
        .ok_or(Conflict)?;
    if !state
        .packages
        .iter()
        .any(|entry| entry.key().is_ok_and(|installed| installed == *key))
    {
        return Err(Conflict);
    }
    let mut store = PolicyPackageStore::new(
        config.package_root.clone(),
        config.registry_root.clone(),
        &config.trust_domain_id,
    )
    .map_err(|_| Unavailable)?;
    // Authenticate history first. A corrupt retained package or registry is an
    // infrastructure failure, not merely a policy that is no longer approved.
    let retained = store
        .read_retained(key, trusted_now)
        .map_err(|_| Unavailable)?;
    if retained.approval_record().record_id != record_id {
        return Err(Conflict);
    }
    let package = store
        .read_approved(key, trusted_now)
        .map_err(|error| match error {
            PackageError::RegistryRejected => Conflict,
            _ => Unavailable,
        })?;
    Ok(InstalledApproval {
        package,
        _publication_lock: lock,
    })
}

fn publication_read_lock(root: &Path) -> Result<File, CatalogueError> {
    files::private_directory(root).map_err(|_| CatalogueError::Unavailable)?;
    let lock = File::open(root).map_err(|_| CatalogueError::Unavailable)?;
    lock.try_lock_shared()
        .map_err(|_| CatalogueError::Unavailable)?;
    reject_pending(root)?;
    Ok(lock)
}

/// Returns only installed approvals valid at server-trusted UTC seconds.
/// Uses no compiler, network, or filesystem writes. Callers must recheck approval
/// at request binding; a catalogue is not a reservation against later revocation.
pub fn list_approved(
    config: &CatalogueConfig,
    trusted_now: i64,
) -> Result<CatalogueProjection, CatalogueError> {
    use CatalogueError::Unavailable;

    let _lock = publication_read_lock(&config.publication_root)?;
    let mut projection = CatalogueProjection {
        format_version: 1,
        policies: Vec::new(),
    };
    let Some(state) = installation::load_state(&config.publication_root, "installed.json")
        .map_err(|_| Unavailable)?
    else {
        return Ok(projection);
    };
    let mut store = PolicyPackageStore::new(
        config.package_root.clone(),
        config.registry_root.clone(),
        &config.trust_domain_id,
    )
    .map_err(|_| Unavailable)?;
    let packages = state
        .packages
        .iter()
        .map(|entry| {
            let key = entry.key().map_err(|_| Unavailable)?;
            store
                .read_retained(&key, trusted_now)
                .map_err(|_| Unavailable)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Authenticate the complete current history before interpreting resolution
    // failures as inactive approvals. Corruption must never look like an empty list.
    let registry_config = registry_config(&config.trust_domain_id).map_err(|_| Unavailable)?;
    let generation = RegistryLoader::new(registry_config.clone())
        .load_current(&config.registry_root, trusted_now)
        .map_err(|_| Unavailable)?;
    let mut registry = LocalRegistry::new(registry_config.clone());
    registry
        .install_generation(generation)
        .map_err(|_| Unavailable)?;
    for package in packages {
        let original =
            policy_record_from_proto(package.approval_record().clone(), &registry_config)
                .map_err(|_| Unavailable)?;
        let Some(current) = current_approval(&mut registry, &original, trusted_now)? else {
            continue;
        };
        if current.claims != original.claims
            || current.authority_signature != original.authority_signature
            || current.canonical_policy_ir_bytes != original.canonical_policy_ir_bytes
            || current.registered_at_unix_seconds != original.registered_at_unix_seconds
            || current.registry_generation < package.registry_record().registry_generation
        {
            return Err(Unavailable);
        }
        projection.policies.push(project(&package, &current));
    }
    Ok(projection)
}

fn reject_pending(root: &Path) -> Result<(), CatalogueError> {
    match std::fs::symlink_metadata(root.join("pending.json")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(CatalogueError::Unavailable),
    }
}

fn current_approval(
    registry: &mut LocalRegistry,
    original: &PolicyRegistryRecordV1,
    now: i64,
) -> Result<Option<PolicyRegistryRecordV1>, CatalogueError> {
    let claims = original.claims.claims();
    let exact = match registry.resolve_policy_exact(&claims.record_id, &claims.policy_hash, now) {
        Ok(record) => record,
        Err(
            RegistryError::UnapprovedPolicy
            | RegistryError::ExpiredPolicy
            | RegistryError::RevokedPolicy
            | RegistryError::Authority(AuthorityError::InactiveKey | AuthorityError::RevokedKey),
        ) => return Ok(None),
        Err(_) => return Err(CatalogueError::Unavailable),
    };
    let by_hash = registry
        .resolve_policy_hash(&claims.policy_hash, now)
        .map_err(|_| CatalogueError::Unavailable)?;
    if exact != by_hash {
        return Err(CatalogueError::Unavailable);
    }
    Ok(Some(exact))
}

fn project(package: &RetainedPolicyPackage, record: &PolicyRegistryRecordV1) -> CataloguePolicy {
    let identity = package.identity();
    let claims = record.claims.claims();
    CataloguePolicy {
        identity: CatalogueIdentity {
            family_id: identity.family_id().into(),
            version_id: identity.version_id().into(),
            version: identity.version(),
        },
        record_id: claims.record_id.clone(),
        policy_hash: lowercase_hex(&claims.policy_hash),
        policy_name: claims.policy_name.clone(),
        policy_version: claims.policy_version.clone(),
        resource_type: identity.resource_type(),
        permission: "query",
        approval_epoch: claims.approval_epoch.to_string(),
        valid_from_unix_seconds: claims.valid_from_unix_seconds.to_string(),
        valid_until_unix_seconds: claims.valid_until_unix_seconds.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
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

    use crate::io::atomic_write;
    use crate::policy_authority::PolicyCandidateManifestWire;
    use crate::policy_package::{PolicyPackageInput, PolicyVersionIdentity};

    use super::*;

    pub(crate) const NOW: i64 = 1_800_000_000;
    const DOMAIN: &str = "catalogue-test";
    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn two_installed_versions_have_only_safe_projection_fields() {
        let mut f = Fixture::new();
        f.retain('b', 1, 1);
        f.retain('c', 2, 1);
        f.retain('d', 3, 1);
        f.install(&[('b', 1), ('c', 1)]);
        let manifest = fs::read(f.config.publication_root.join("installed.json")).unwrap();
        let result = list_approved(&f.config, NOW).unwrap();
        let policies: Vec<_> = f.records[..2].iter().enumerate().map(|(i, r)| json!({
            "identity": {"familyId": "a".repeat(32), "versionId": (["b", "c"][i].repeat(32)), "version": i + 1},
            "recordId": r.record_id,
            "policyHash": lowercase_hex(&r.policy_hash),
            "policyName": r.policy_name,
            "policyVersion": r.policy_version,
            "resourceType": format!("dataset_{}", ["b", "c"][i].repeat(32)),
            "permission": "query",
            "approvalEpoch": "1",
            "validFromUnixSeconds": (NOW - 100).to_string(),
            "validUntilUnixSeconds": (NOW + 5_000).to_string(),
        })).collect();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({"formatVersion": 1, "policies": policies})
        );
        assert_eq!(
            manifest,
            fs::read(f.config.publication_root.join("installed.json")).unwrap()
        );
        assert_eq!(fs::read_dir(&f.config.publication_root).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&f.config.package_root).unwrap().count(), 3);
    }

    #[test]
    fn expiry_revocation_and_supersession_are_not_selectable() {
        let mut f = Fixture::new();
        f.retain('b', 1, 1);
        f.retain('c', 2, 1);
        f.install(&[('b', 1), ('c', 1)]);
        assert!(
            list_approved(&f.config, NOW + 5_000)
                .unwrap()
                .policies
                .is_empty()
        );
        f.records[0].status = proto::PolicyStatus::Revoked as i32;
        f.records[0].revoked_at = Some(timestamp(NOW));
        f.publish();
        assert_eq!(list_approved(&f.config, NOW).unwrap().policies.len(), 1);
        f.retain('c', 2, 2);
        assert!(list_approved(&f.config, NOW).unwrap().policies.is_empty());
        f.install(&[('b', 1), ('c', 2)]);
        let policies = list_approved(&f.config, NOW).unwrap().policies;
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].approval_epoch, "2");
        assert_eq!(fs::read_dir(&f.config.package_root).unwrap().count(), 3);
    }

    #[test]
    fn invalid_registry_history_is_not_filtered_as_unapproved() {
        let mut f = Fixture::new();
        f.retain('b', 1, 1);
        f.retain('c', 2, 1);
        f.install(&[('b', 1)]);
        // This record is not installed. Full history authentication must still fail,
        // including when the installed approval has expired.
        f.records[1].authority_signature[0] ^= 1;
        f.publish();
        assert_eq!(
            list_approved(&f.config, NOW + 5_000),
            Err(CatalogueError::Unavailable)
        );
        atomic_write(
            &f.config.registry_root.join("current.pb"),
            b"invalid registry",
        )
        .unwrap();
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
    }

    #[test]
    fn retained_artifact_corruption_is_not_filtered_as_expiry() {
        let mut f = Fixture::new();
        f.retain('b', 1, 1);
        f.install(&[('b', 1)]);
        let path = f
            .config
            .package_root
            .join(format!("{}.1.pb", "b".repeat(32)));
        atomic_write(&path, b"invalid package").unwrap();
        assert_eq!(
            list_approved(&f.config, NOW + 5_000),
            Err(CatalogueError::Unavailable)
        );
    }

    #[test]
    fn missing_installation_is_empty_but_pending_and_unsafe_state_fail_closed() {
        let f = Fixture::new();
        assert!(list_approved(&f.config, NOW).unwrap().policies.is_empty());
        let pending = f.config.publication_root.join("pending.json");
        atomic_write(&pending, b"unfinished").unwrap();
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
        f.install(&[]);
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
        fs::remove_file(&pending).unwrap();
        symlink(f.config.publication_root.join("absent"), &pending).unwrap();
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
        fs::remove_file(&pending).unwrap();
        let installed = f.config.publication_root.join("installed.json");
        fs::remove_file(&installed).unwrap();
        symlink(f.config.publication_root.join("absent"), &installed).unwrap();
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
        fs::remove_file(&installed).unwrap();
        fs::set_permissions(
            &f.config.publication_root,
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
    }

    #[test]
    fn installed_manifest_validation_and_publication_lock_are_reused() {
        let f = Fixture::new();
        f.install(&[]);
        let path = f.config.publication_root.join("installed.json");
        let state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let mutations = [
            ("format_version", json!(2)),
            ("schema_sha256", json!("not-a-digest")),
            ("unknown", json!(true)),
            (
                "packages",
                json!([{"version_id": "b".repeat(32), "approval_epoch": 0}]),
            ),
            (
                "packages",
                json!(
                    (0..33)
                        .map(|i| json!({"version_id": format!("{i:032x}"), "approval_epoch": 1}))
                        .collect::<Vec<_>>()
                ),
            ),
            (
                "packages",
                json!([{"version_id": "b".repeat(32), "approval_epoch": 1}, {"version_id": "b".repeat(32), "approval_epoch": 1}]),
            ),
        ];
        for (key, value) in mutations {
            let mut invalid = state.clone();
            invalid[key] = value;
            atomic_write(&path, &serde_json::to_vec(&invalid).unwrap()).unwrap();
            assert_eq!(
                list_approved(&f.config, NOW),
                Err(CatalogueError::Unavailable)
            );
        }
        f.install(&[]);
        let writer = File::open(&f.config.publication_root).unwrap();
        writer.try_lock().unwrap();
        assert_eq!(
            list_approved(&f.config, NOW),
            Err(CatalogueError::Unavailable)
        );
        drop(writer);
        assert!(list_approved(&f.config, NOW).unwrap().policies.is_empty());
    }

    pub(crate) struct Fixture {
        root: PathBuf,
        pub(crate) config: CatalogueConfig,
        signer: AuthoritySigner<PolicyApprovalRole>,
        pub(crate) records: Vec<proto::PolicyRecord>,
        generation: u64,
    }

    impl Fixture {
        pub(crate) fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rz-catalogue-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let config = CatalogueConfig {
                publication_root: root.join("publication"),
                package_root: root.join("packages"),
                registry_root: root.join("registry"),
                trust_domain_id: DOMAIN.into(),
            };
            for path in [
                &root,
                &config.publication_root,
                &config.package_root,
                &config.registry_root,
            ] {
                fs::DirBuilder::new().mode(0o700).create(path).unwrap();
            }
            let der = [
                vec![
                    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04,
                    0x22, 0x04, 0x20,
                ],
                vec![0x51; 32],
            ]
            .concat();
            let mut fixture = Self {
                root,
                config,
                signer: AuthoritySigner::from_pkcs8_der(&der).unwrap(),
                records: vec![],
                generation: 0,
            };
            fixture.publish();
            fixture
        }

        pub(crate) fn retain(&mut self, id: char, version: u32, epoch: u64) {
            let identity =
                PolicyVersionIdentity::new("a".repeat(32), id.to_string().repeat(32), version)
                    .unwrap();
            let resource = identity.resource_type();
            let source = format!(
                "definition agent {{}}\ndefinition {resource} {{ relation viewer: agent\npermission query = viewer\n}}\n"
            );
            let limits = TRUSTED_POLICY_CEILINGS.policy_limits;
            let policy = proto::PolicyProgram {
                ir_version: 1,
                compiler_name: "authzed/spicedb".into(),
                compiler_version: "1.54.0".into(),
                source_schema_digest: Sha256::digest(source.as_bytes()).to_vec(),
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
                    max_expression_nodes: limits.max_expression_nodes,
                    max_caveat_nodes: limits.max_caveat_nodes,
                    max_path_depth: limits.max_path_depth,
                    max_relationship_witnesses: limits.max_relationship_witnesses,
                    max_merkle_depth: limits.max_merkle_depth,
                    max_string_bytes: limits.max_string_bytes,
                    max_context_entries: limits.max_context_entries,
                    max_total_private_input_bytes: limits.max_total_private_input_bytes,
                }),
            };
            let validated = ValidatedPolicyProgram::from_trusted_proto(
                policy.clone(),
                &TRUSTED_POLICY_CEILINGS,
            )
            .unwrap();
            let manifest = PolicyCandidateManifestWire {
                format_version: 1,
                candidate_id: format!("candidate-{id}-{epoch}"),
                record_id: format!("policy-{id}-{epoch}"),
                policy_name: identity.signed_policy_name(),
                policy_version: version.to_string(),
                resource_type: resource,
                permission: "query".into(),
                rule_summary: "private viewer rule".into(),
                source_schema_digest: policy.source_schema_digest.clone(),
                policy_hash: policy_hash(&validated).unwrap().to_vec(),
                expected_approval_epoch: epoch,
                valid_from_unix_seconds: NOW - 100,
                valid_until_unix_seconds: NOW + 5_000,
                created_at_unix_seconds: NOW - 10,
                expires_at_unix_seconds: NOW + 10,
            };
            let claims = ValidatedPolicyApprovalClaimsV1::new(
                PolicyApprovalClaimsV1 {
                    format_version: 1,
                    trust_domain_id: DOMAIN.into(),
                    record_id: manifest.record_id.clone(),
                    policy_name: manifest.policy_name.clone(),
                    policy_version: manifest.policy_version.clone(),
                    approval_epoch: epoch,
                    policy_hash: manifest.policy_hash.as_slice().try_into().unwrap(),
                    valid_from_unix_seconds: NOW - 100,
                    valid_until_unix_seconds: NOW + 5_000,
                    approved_by: "owner".into(),
                    signing_key_id: self.signer.key_id().into(),
                },
                PolicyApprovalLimits {
                    max_string_bytes: 512,
                    max_lifetime_seconds: 31 * 24 * 3600,
                },
            )
            .unwrap();
            self.records.push(proto::PolicyRecord {
                format_version: 1,
                record_id: manifest.record_id.clone(),
                policy_name: manifest.policy_name.clone(),
                policy_version: manifest.policy_version.clone(),
                canonical_policy_ir: Some(policy.clone()),
                policy_hash: manifest.policy_hash.clone(),
                status: proto::PolicyStatus::Approved as i32,
                valid_from: Some(timestamp(NOW - 100)),
                valid_until: Some(timestamp(NOW + 5_000)),
                approved_by: "owner".into(),
                signing_key_id: self.signer.key_id().into(),
                authority_signature: self.signer.sign_policy_approval(&claims).unwrap().to_vec(),
                registered_at: Some(timestamp(NOW)),
                trust_domain_id: DOMAIN.into(),
                approval_epoch: epoch,
                revoked_at: None,
                revocation_reason: String::new(),
                registry_generation: 0,
            });
            self.publish();
            PolicyPackageStore::new(
                self.config.package_root.clone(),
                self.config.registry_root.clone(),
                DOMAIN,
            )
            .unwrap()
            .retain(
                PolicyPackageInput {
                    identity,
                    candidate_manifest: &manifest.encode_to_vec(),
                    source_schema: source.as_bytes(),
                    compiler_config: json!({"config_version": 1, "limits": limits, "caveats": {}})
                        .to_string()
                        .as_bytes(),
                    policy_pb: &policy.encode_to_vec(),
                },
                NOW,
            )
            .unwrap();
        }

        pub(crate) fn publish(&mut self) {
            self.generation += 1;
            let key = proto::AuthorityKeyRecord {
                format_version: 1,
                trust_domain_id: DOMAIN.into(),
                key_id: self.signer.key_id().into(),
                role: proto::AuthorityRole::PolicyApproval as i32,
                public_key: self.signer.public_key_bytes().to_vec(),
                status: proto::AuthorityKeyStatus::Active as i32,
                valid_from: Some(timestamp(NOW - 1_000)),
                valid_until: Some(timestamp(NOW + 10_000)),
                revoked_at: None,
                revocation_reason: String::new(),
                registry_generation: self.generation,
            };
            let mut manifest = proto::RegistryGenerationManifest {
                format_version: 1,
                trust_domain_id: DOMAIN.into(),
                generation: self.generation,
                published_at: Some(timestamp(NOW)),
                authority_keys: vec![self.write("key.pb", &key.encode_to_vec())],
                policies: vec![],
                accepted_roots: vec![],
            };
            for index in 0..self.records.len() {
                self.records[index].registry_generation = self.generation;
                manifest.policies.push(self.write(
                    &format!("policy-{index:03}.pb"),
                    &self.records[index].encode_to_vec(),
                ));
            }
            self.write("current.pb", &manifest.encode_to_vec());
        }

        fn write(&self, name: &str, bytes: &[u8]) -> proto::RegistryFile {
            atomic_write(&self.config.registry_root.join(name), bytes).unwrap();
            proto::RegistryFile {
                relative_path: name.into(),
                sha256: Sha256::digest(bytes).to_vec(),
            }
        }

        pub(crate) fn install(&self, keys: &[(char, u64)]) {
            let state = json!({"format_version": 1, "generation": 1, "legacy": false, "schema_sha256": "e".repeat(64),
                "packages": keys.iter().map(|(id, epoch)| json!({"version_id": id.to_string().repeat(32), "approval_epoch": epoch})).collect::<Vec<_>>()});
            atomic_write(
                &self.config.publication_root.join("installed.json"),
                &serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn timestamp(seconds: i64) -> prost_types::Timestamp {
        prost_types::Timestamp { seconds, nanos: 0 }
    }
}
