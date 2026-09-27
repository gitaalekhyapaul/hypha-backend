use std::collections::BTreeMap;
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use prost::Message;
use raziel_verifier::{
    LocalRegistry, PolicyRecordStatus, PolicyRegistryRecordV1, RegistryConfig, RegistryLoader,
    decode_policy_record_strict, policy_record_from_proto, policy_record_to_proto,
};

use crate::config::{MAX_RECORD_BYTES, registry_config};
use crate::io::atomic_write_new;

use super::validation::{ValidatedArtifacts, artifacts, canonical};
use super::wire::StoredPackage;
use super::{
    ApprovedPolicyPackage, MAX_PACKAGE_BYTES, PackageError, PolicyPackageInput, PolicyPackageKey,
    PolicyVersionIdentity, RetainedPolicyPackage,
};

pub struct PolicyPackageStore {
    root: PathBuf,
    registry_root: PathBuf,
    config: RegistryConfig,
    registry: LocalRegistry,
}

impl PolicyPackageStore {
    /// Both roots must exist. The package root must be a dedicated private 0700 directory.
    pub fn new(
        root: PathBuf,
        registry_root: PathBuf,
        trust_domain_id: &str,
    ) -> Result<Self, PackageError> {
        validate_path(&root)?;
        validate_path(&registry_root)?;
        if root.starts_with(&registry_root) || registry_root.starts_with(&root) {
            return Err(PackageError::InvalidConfiguration);
        }
        private_directory(&root)?;
        let config =
            registry_config(trust_domain_id).map_err(|_| PackageError::InvalidConfiguration)?;
        Ok(Self {
            root,
            registry_root,
            registry: LocalRegistry::new(config.clone()),
            config,
        })
    }

    /// Copies approved artifacts into immutable storage. Exact retries reuse the retained file.
    /// The caller must supply server-trusted time and owner-selected candidate artifacts.
    pub fn retain(
        &mut self,
        input: PolicyPackageInput<'_>,
        trusted_now: i64,
    ) -> Result<ApprovedPolicyPackage, PackageError> {
        let validated = artifacts(&input)?;
        let record = self.resolve(&validated, trusted_now)?;
        let stored = StoredPackage {
            format_version: 1,
            family_id: input.identity.family_id().into(),
            version_id: input.identity.version_id().into(),
            version: input.identity.version(),
            candidate_manifest: input.candidate_manifest.into(),
            source_schema: input.source_schema.into(),
            compiler_config: input.compiler_config.into(),
            policy_pb: input.policy_pb.into(),
            approval_record: policy_record_to_proto(&record)
                .map_err(|_| PackageError::RegistryRejected)?
                .encode_to_vec(),
            definitions: validated.inventory.definitions.clone(),
            caveats: validated.inventory.caveats.clone(),
        };
        if stored.encoded_len() as u64 > MAX_PACKAGE_BYTES {
            return Err(PackageError::InvalidPackage);
        }
        let key = PolicyPackageKey::new(
            stored.version_id.clone(),
            record.claims.claims().approval_epoch,
        )?;
        let owner = private_directory(&self.root)?;
        // One short writer lock covers collision checks and the no-replace publication.
        let lock = File::open(&self.root).map_err(|_| PackageError::Unavailable)?;
        lock.try_lock().map_err(|_| PackageError::Unavailable)?;
        self.check_conflicts(&stored, owner)?;
        let path = self.path(&key);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return self.read_approved(&key, trusted_now),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(PackageError::Unavailable),
        }
        atomic_write_new(&path, &stored.encode_to_vec()).map_err(|_| PackageError::Unavailable)?;
        self.read_approved(&key, trusted_now)
    }

    /// Does not delete retained bytes if the registry now rejects their approval.
    pub fn read_approved(
        &mut self,
        key: &PolicyPackageKey,
        trusted_now: i64,
    ) -> Result<ApprovedPolicyPackage, PackageError> {
        let retained = self.read_retained(key, trusted_now)?;
        let hash = retained
            .policy_hash()
            .try_into()
            .map_err(|_| PackageError::InvalidPackage)?;
        let exact = self
            .registry
            .resolve_policy_exact(&retained.record.record_id, &hash, trusted_now)
            .map_err(|_| PackageError::RegistryRejected)?;
        let by_hash = self
            .registry
            .resolve_policy_hash(&hash, trusted_now)
            .map_err(|_| PackageError::RegistryRejected)?;
        if exact != by_hash {
            return Err(PackageError::RegistryRejected);
        }
        Ok(ApprovedPolicyPackage { retained })
    }

    /// Reads authenticated history without granting current approval or snapshot access.
    pub fn read_retained(
        &mut self,
        key: &PolicyPackageKey,
        trusted_now: i64,
    ) -> Result<RetainedPolicyPackage, PackageError> {
        let owner = private_directory(&self.root)?;
        let bytes = read_private(&self.path(key), owner)?;
        let records = self.load_registry(trusted_now)?;
        self.retained_from_bytes(key, &bytes, &records)
    }

    /// Returns every retained approval, ordered by version ID then numeric epoch.
    /// Count and aggregate byte limits match the local registry's bounded history.
    pub fn list_retained(
        &mut self,
        trusted_now: i64,
    ) -> Result<Vec<RetainedPolicyPackage>, PackageError> {
        let owner = private_directory(&self.root)?;
        let records = self.load_registry(trusted_now)?;
        let mut paths = Vec::new();
        let mut total_bytes = 0_u64;
        for entry in std::fs::read_dir(&self.root).map_err(|_| PackageError::Unavailable)? {
            let entry = entry.map_err(|_| PackageError::Unavailable)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(PackageError::InvalidPackage)?;
            if name.starts_with('.') {
                continue;
            }
            if paths.len() >= self.config.limits.max_manifest_files_per_kind as usize {
                return Err(PackageError::InvalidPackage);
            }
            let key = key_from_name(name)?;
            let metadata =
                std::fs::symlink_metadata(entry.path()).map_err(|_| PackageError::Unavailable)?;
            private_metadata(&metadata, owner)?;
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .filter(|n| *n <= self.config.limits.max_manifest_total_bytes)
                .ok_or(PackageError::InvalidPackage)?;
            paths.push((key, entry.path()));
        }
        paths.sort_by(|(a, _), (b, _)| {
            a.version_id
                .cmp(&b.version_id)
                .then(a.approval_epoch.cmp(&b.approval_epoch))
        });
        let mut packages = Vec::with_capacity(paths.len());
        total_bytes = 0;
        for (key, path) in paths {
            let bytes = read_private(&path, owner)?;
            total_bytes = total_bytes
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.config.limits.max_manifest_total_bytes)
                .ok_or(PackageError::InvalidPackage)?;
            packages.push(self.retained_from_bytes(&key, &bytes, &records)?);
        }
        Ok(packages)
    }

    fn retained_from_bytes(
        &self,
        key: &PolicyPackageKey,
        bytes: &[u8],
        records: &BTreeMap<String, PolicyRegistryRecordV1>,
    ) -> Result<RetainedPolicyPackage, PackageError> {
        let stored: StoredPackage = canonical(bytes)?;
        let (identity, validated, recorded) = self.validate_stored(&stored)?;
        if key.version_id != identity.version_id
            || key.approval_epoch != recorded.claims.claims().approval_epoch
        {
            return Err(PackageError::InvalidPackage);
        }
        let current = records
            .get(&validated.manifest.record_id)
            .ok_or(PackageError::RegistryRejected)?;
        if !same_approval(&recorded, current)
            || recorded.registry_generation > current.registry_generation
        {
            return Err(PackageError::RegistryRejected);
        }
        let record = decode_policy_record_strict(&stored.approval_record, MAX_RECORD_BYTES)
            .map_err(|_| PackageError::InvalidPackage)?;
        Ok(RetainedPolicyPackage {
            identity,
            key: key.clone(),
            inventory: validated.inventory,
            policy: validated.policy,
            record,
            current_record: policy_record_to_proto(current)
                .map_err(|_| PackageError::RegistryRejected)?,
            stored,
        })
    }

    fn load_registry(
        &mut self,
        now: i64,
    ) -> Result<BTreeMap<String, PolicyRegistryRecordV1>, PackageError> {
        let generation = RegistryLoader::new(self.config.clone())
            .load_current(&self.registry_root, now)
            .map_err(|_| PackageError::RegistryRejected)?;
        // Loading checks framing and digests. Installation authenticates signatures,
        // immutable history, mutable status transitions, and generation rollback.
        self.registry
            .install_generation(generation.clone())
            .map_err(|_| PackageError::RegistryRejected)?;
        Ok(generation
            .policies
            .into_iter()
            .map(|record| (record.claims.claims().record_id.clone(), record))
            .collect())
    }

    fn resolve(
        &mut self,
        value: &ValidatedArtifacts,
        now: i64,
    ) -> Result<PolicyRegistryRecordV1, PackageError> {
        let generation = RegistryLoader::new(self.config.clone())
            .load_current(&self.registry_root, now)
            .map_err(|_| PackageError::RegistryRejected)?;
        self.registry
            .install_generation(generation)
            .map_err(|_| PackageError::RegistryRejected)?;
        let exact = self
            .registry
            .resolve_policy_exact(&value.manifest.record_id, &value.policy_hash, now)
            .map_err(|_| PackageError::RegistryRejected)?;
        let by_hash = self
            .registry
            .resolve_policy_hash(&value.policy_hash, now)
            .map_err(|_| PackageError::RegistryRejected)?;
        if exact != by_hash {
            return Err(PackageError::RegistryRejected);
        }
        match_record(value, &exact)?;
        Ok(exact)
    }

    fn validate_stored(
        &self,
        stored: &StoredPackage,
    ) -> Result<
        (
            PolicyVersionIdentity,
            ValidatedArtifacts,
            PolicyRegistryRecordV1,
        ),
        PackageError,
    > {
        if stored.format_version != 1 || stored.encoded_len() as u64 > MAX_PACKAGE_BYTES {
            return Err(PackageError::InvalidPackage);
        }
        let identity = PolicyVersionIdentity::new(
            stored.family_id.clone(),
            stored.version_id.clone(),
            stored.version,
        )?;
        let validated = artifacts(&PolicyPackageInput {
            identity: identity.clone(),
            candidate_manifest: &stored.candidate_manifest,
            source_schema: &stored.source_schema,
            compiler_config: &stored.compiler_config,
            policy_pb: &stored.policy_pb,
        })?;
        if stored.definitions != validated.inventory.definitions
            || stored.caveats != validated.inventory.caveats
        {
            return Err(PackageError::InvalidPackage);
        }
        let wire = decode_policy_record_strict(&stored.approval_record, MAX_RECORD_BYTES)
            .map_err(|_| PackageError::InvalidPackage)?;
        let record = policy_record_from_proto(wire, &self.config)
            .map_err(|_| PackageError::InvalidPackage)?;
        match_record(&validated, &record)?;
        if record.status != PolicyRecordStatus::Approved
            || record.canonical_policy_ir_bytes != stored.policy_pb
        {
            return Err(PackageError::InvalidPackage);
        }
        Ok((identity, validated, record))
    }

    fn check_conflicts(&self, proposed: &StoredPackage, owner: u32) -> Result<(), PackageError> {
        let (_, _, proposed_record) = self.validate_stored(proposed)?;
        let mut count = 0;
        for entry in std::fs::read_dir(&self.root).map_err(|_| PackageError::Unavailable)? {
            let entry = entry.map_err(|_| PackageError::Unavailable)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(PackageError::InvalidPackage)?;
            if name.starts_with('.') {
                continue;
            }
            count += 1;
            if count > self.config.limits.max_manifest_files_per_kind {
                return Err(PackageError::InvalidPackage);
            }
            let existing: StoredPackage = canonical(&read_private(&entry.path(), owner)?)?;
            let (_, _, record) = self.validate_stored(&existing)?;
            let key = PolicyPackageKey::new(
                existing.version_id.clone(),
                record.claims.claims().approval_epoch,
            )?;
            if self.path(&key) != entry.path() {
                return Err(PackageError::InvalidPackage);
            }
            let same_version = proposed.version_id == existing.version_id;
            let same_family_number =
                proposed.family_id == existing.family_id && proposed.version == existing.version;
            if same_version != same_family_number {
                return Err(PackageError::Conflict);
            }
            if same_version {
                if proposed.source_schema != existing.source_schema
                    || proposed.compiler_config != existing.compiler_config
                    || proposed.policy_pb != existing.policy_pb
                    || proposed.definitions != existing.definitions
                    || proposed.caveats != existing.caveats
                {
                    return Err(PackageError::Conflict);
                }
                if proposed_record.claims.claims().approval_epoch
                    == record.claims.claims().approval_epoch
                    && (proposed.candidate_manifest != existing.candidate_manifest
                        || !same_approval(&record, &proposed_record))
                {
                    return Err(PackageError::Conflict);
                }
            }
        }
        Ok(())
    }

    fn path(&self, key: &PolicyPackageKey) -> PathBuf {
        self.root
            .join(format!("{}.{}.pb", key.version_id, key.approval_epoch))
    }
}

fn match_record(
    value: &ValidatedArtifacts,
    record: &PolicyRegistryRecordV1,
) -> Result<(), PackageError> {
    let m = &value.manifest;
    let c = record.claims.claims();
    if c.record_id != m.record_id
        || c.policy_name != m.policy_name
        || c.policy_version != m.policy_version
        || c.approval_epoch != m.expected_approval_epoch
        || c.policy_hash != value.policy_hash
        || c.valid_from_unix_seconds != m.valid_from_unix_seconds
        || c.valid_until_unix_seconds != m.valid_until_unix_seconds
        || record.policy != value.policy
    {
        return Err(PackageError::RegistryRejected);
    }
    Ok(())
}

fn same_approval(left: &PolicyRegistryRecordV1, right: &PolicyRegistryRecordV1) -> bool {
    left.claims == right.claims
        && left.authority_signature == right.authority_signature
        && left.policy == right.policy
        && left.canonical_policy_ir_bytes == right.canonical_policy_ir_bytes
        && left.registered_at_unix_seconds == right.registered_at_unix_seconds
}

fn key_from_name(name: &str) -> Result<PolicyPackageKey, PackageError> {
    let (version, epoch) = name
        .strip_suffix(".pb")
        .and_then(|stem| stem.split_once('.'))
        .ok_or(PackageError::InvalidPackage)?;
    let parsed = epoch
        .parse::<u64>()
        .map_err(|_| PackageError::InvalidPackage)?;
    if parsed.to_string() != epoch {
        return Err(PackageError::InvalidPackage);
    }
    PolicyPackageKey::new(version.into(), parsed).map_err(|_| PackageError::InvalidPackage)
}

fn validate_path(path: &Path) -> Result<(), PackageError> {
    if !path.is_absolute()
        || path == Path::new("/")
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(PackageError::InvalidConfiguration);
    }
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        let metadata =
            std::fs::symlink_metadata(&prefix).map_err(|_| PackageError::InvalidConfiguration)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(PackageError::InvalidConfiguration);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn private_directory(path: &Path) -> Result<u32, PackageError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let m = std::fs::symlink_metadata(path).map_err(|_| PackageError::Unavailable)?;
    if !m.is_dir() || m.file_type().is_symlink() || m.permissions().mode() & 0o7777 != 0o700 {
        return Err(PackageError::InvalidConfiguration);
    }
    Ok(m.uid())
}

#[cfg(not(unix))]
fn private_directory(_: &Path) -> Result<u32, PackageError> {
    Err(PackageError::InvalidConfiguration)
}

#[cfg(unix)]
fn private_metadata(m: &Metadata, owner: u32) -> Result<(), PackageError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.nlink() != 1
        || m.uid() != owner
        || m.permissions().mode() & 0o7777 != 0o600
        || m.len() > MAX_PACKAGE_BYTES
    {
        return Err(PackageError::InvalidPackage);
    }
    Ok(())
}

#[cfg(not(unix))]
fn private_metadata(_: &Metadata, _: u32) -> Result<(), PackageError> {
    Err(PackageError::InvalidConfiguration)
}

fn read_private(path: &Path, owner: u32) -> Result<Vec<u8>, PackageError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            PackageError::NotFound
        } else {
            PackageError::Unavailable
        }
    })?;
    private_metadata(&metadata, owner)?;
    let file = File::open(path).map_err(|_| PackageError::Unavailable)?;
    let opened = file.metadata().map_err(|_| PackageError::Unavailable)?;
    private_metadata(&opened, owner)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.dev() != opened.dev() || metadata.ino() != opened.ino() {
            return Err(PackageError::InvalidPackage);
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_PACKAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PackageError::Unavailable)?;
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::InvalidPackage);
    }
    Ok(bytes)
}
