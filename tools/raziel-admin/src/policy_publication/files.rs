use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use prost::Message;
use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_canonical::policy_hash;
use raziel_policy_ir::decode_policy_program_strict;
use raziel_verifier::{LocalRegistry, RegistryLoader};
use sha2::{Digest, Sha256};

use crate::config::{MAX_RECORD_BYTES, registry_config};
use crate::policy_activation::store::StoreActivationError as Error;
use crate::policy_authority::PolicyCandidateManifestWire;
use crate::policy_package::PolicyVersionIdentity;

pub(super) const MAX_SCHEMA: u64 = 1024 * 1024;

pub(super) struct Artifacts {
    pub schema: Vec<u8>,
    pub config: Vec<u8>,
    pub policy: Vec<u8>,
    pub manifest: Vec<u8>,
}

impl Artifacts {
    pub fn load(root: &Path) -> Result<Self, Error> {
        private_directory(root)?;
        Ok(Self {
            schema: read(root, "schema.zed", MAX_SCHEMA)?,
            config: read(root, "compiler.json", 64 * 1024)?,
            policy: read(root, "policy.pb", MAX_RECORD_BYTES)?,
            manifest: read(root, "candidate.pb", MAX_RECORD_BYTES)?,
        })
    }

    pub fn manifest(&self) -> Result<PolicyCandidateManifestWire, Error> {
        let manifest = PolicyCandidateManifestWire::decode(self.manifest.as_slice())
            .map_err(|_| Error::CandidateRejected)?;
        if manifest.encode_to_vec() != self.manifest || manifest.format_version != 1 {
            return Err(Error::CandidateRejected);
        }
        Ok(manifest)
    }

    pub fn check_identity(&self, identity: &PolicyVersionIdentity) -> Result<(), Error> {
        let m = self.manifest()?;
        if m.policy_name != identity.signed_policy_name()
            || m.policy_version != identity.version().to_string()
            || m.resource_type != identity.resource_type()
        {
            return Err(Error::CandidateRejected);
        }
        Ok(())
    }

    pub fn validate_legacy(
        &self,
        registry: &Path,
        trust_domain: &str,
        now: i64,
    ) -> Result<(), Error> {
        let m = self.manifest()?;
        if m.policy_name.starts_with("family:") {
            return Err(Error::CandidateRejected);
        }
        let policy =
            decode_policy_program_strict(&self.policy, MAX_RECORD_BYTES, &TRUSTED_POLICY_CEILINGS)
                .map_err(|_| Error::CandidateRejected)?;
        let hash = policy_hash(&policy).map_err(|_| Error::CandidateRejected)?;
        if policy.program().source_schema_digest.as_slice()
            != Sha256::digest(&self.schema).as_slice()
            || m.policy_hash != hash
        {
            return Err(Error::CandidateRejected);
        }
        // Historical signatures remain checked by the registry loader. Expiry and
        // revocation bar new snapshots, not preservation of installed definitions.
        let config = registry_config(trust_domain).map_err(|_| Error::Unavailable)?;
        let registry = RegistryLoader::new(config.clone())
            .load_current(registry, now)
            .map_err(|_| Error::Unavailable)?;
        LocalRegistry::new(config)
            .install_generation(registry.clone())
            .map_err(|_| Error::CandidateRejected)?;
        let record = registry
            .policies
            .iter()
            .find(|r| r.claims.claims().record_id == m.record_id)
            .ok_or(Error::CandidateRejected)?;
        let claims = record.claims.claims();
        if record.canonical_policy_ir_bytes != self.policy
            || claims.policy_hash != hash
            || claims.policy_name != m.policy_name
            || claims.policy_version != m.policy_version
            || claims.approval_epoch != m.expected_approval_epoch
        {
            return Err(Error::CandidateRejected);
        }
        Ok(())
    }

    pub fn persist(&self, root: &Path) -> Result<(), Error> {
        create_private_directory(root)?;
        for (name, bytes) in [
            ("schema.zed", &self.schema),
            ("compiler.json", &self.config),
            ("policy.pb", &self.policy),
            ("candidate.pb", &self.manifest),
        ] {
            write_new(root, name, bytes)?;
        }
        Ok(())
    }
}

pub(super) fn private_directory(path: &Path) -> Result<u32, Error> {
    if !path.is_absolute() || path == Path::new("/") {
        return Err(Error::InvalidRequest);
    }
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|_| Error::Unavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::InvalidRequest);
        }
    }
    let m = std::fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if !m.is_dir() || m.file_type().is_symlink() || m.permissions().mode() & 0o7777 != 0o700 {
        return Err(Error::Unavailable);
    }
    Ok(m.uid())
}

pub(super) fn create_private_directory(path: &Path) -> Result<(), Error> {
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(Error::Unavailable),
    }
    private_directory(path)?;
    Ok(())
}

pub(super) fn read(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>, Error> {
    read_private(root, name, limit, false)
}

pub(super) fn read_secret(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>, Error> {
    read_private(root, name, limit, true)
}

fn read_private(
    root: &Path,
    name: &str,
    limit: u64,
    allow_read_only: bool,
) -> Result<Vec<u8>, Error> {
    let owner = private_directory(root)?;
    let path = root.join(name);
    let m = std::fs::symlink_metadata(&path).map_err(|_| Error::Unavailable)?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.nlink() != 1
        || m.uid() != owner
        || !(m.permissions().mode() & 0o7777 == 0o600
            || allow_read_only && m.permissions().mode() & 0o7777 == 0o400)
        || m.len() > limit
    {
        return Err(Error::CandidateRejected);
    }
    let file = File::open(path).map_err(|_| Error::Unavailable)?;
    let opened = file.metadata().map_err(|_| Error::Unavailable)?;
    if opened.dev() != m.dev() || opened.ino() != m.ino() {
        return Err(Error::CandidateRejected);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable)?;
    if bytes.len() as u64 > limit {
        return Err(Error::CandidateRejected);
    }
    Ok(bytes)
}

pub(super) fn write_new(root: &Path, name: &str, bytes: &[u8]) -> Result<(), Error> {
    private_directory(root)?;
    let path = root.join(name);
    if path.try_exists().map_err(|_| Error::Unavailable)? {
        if read(root, name, bytes.len() as u64)? != bytes {
            return Err(Error::Conflict);
        }
        return Ok(());
    }
    crate::io::atomic_write_new(&path, bytes).map_err(|_| Error::Unavailable)
}
