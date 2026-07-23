use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
};

use raziel_proto::raziel::v1 as proto;
use sha2::{Digest, Sha256};

use crate::{
    RegistryConfig, RegistryError, RegistryGenerationV1, authority_key_from_proto,
    decode_accepted_root_record_strict, decode_authority_key_record_strict,
    decode_policy_record_strict, decode_registry_manifest_strict, policy_record_from_proto,
    root_record_from_proto,
};

const REGISTRY_MANIFEST_FORMAT_VERSION: u32 = 1;
const PROTOBUF_TIMESTAMP_MIN_SECONDS: i64 = -62_135_596_800;
const PROTOBUF_TIMESTAMP_MAX_SECONDS: i64 = 253_402_300_799;

pub struct RegistryLoader {
    config: RegistryConfig,
}

impl RegistryLoader {
    pub const fn new(config: RegistryConfig) -> Self {
        Self { config }
    }

    pub fn load_candidate(
        &self,
        registry_root: &Path,
        manifest_bytes: &[u8],
        observed_now_unix_seconds: i64,
    ) -> Result<RegistryGenerationV1, RegistryError> {
        let root_metadata =
            std::fs::symlink_metadata(registry_root).map_err(|_| RegistryError::FileRead)?;
        if root_metadata.file_type().is_symlink() {
            return Err(RegistryError::SymlinkForbidden);
        }
        if !root_metadata.is_dir() {
            return Err(RegistryError::FilePathInvalid);
        }

        let manifest =
            decode_registry_manifest_strict(manifest_bytes, self.config.limits.max_record_bytes)?;
        self.validate_manifest(&manifest, observed_now_unix_seconds)?;

        let mut all_paths = BTreeSet::new();
        validate_entries(
            &manifest.authority_keys,
            self.config.limits.max_manifest_files_per_kind,
            &mut all_paths,
        )?;
        validate_entries(
            &manifest.policies,
            self.config.limits.max_manifest_files_per_kind,
            &mut all_paths,
        )?;
        validate_entries(
            &manifest.accepted_roots,
            self.config.limits.max_manifest_files_per_kind,
            &mut all_paths,
        )?;

        let mut aggregate_bytes = 0_u64;
        let mut authority_keys = Vec::with_capacity(manifest.authority_keys.len());
        for entry in &manifest.authority_keys {
            let bytes =
                read_verified_file(registry_root, entry, &self.config, &mut aggregate_bytes)?;
            let wire =
                decode_authority_key_record_strict(&bytes, self.config.limits.max_record_bytes)?;
            if wire.registry_generation != manifest.generation {
                return Err(RegistryError::RegistryGenerationInvalid);
            }
            authority_keys.push(authority_key_from_proto(wire, &self.config)?);
        }

        let mut policies = Vec::with_capacity(manifest.policies.len());
        for entry in &manifest.policies {
            let bytes =
                read_verified_file(registry_root, entry, &self.config, &mut aggregate_bytes)?;
            let wire = decode_policy_record_strict(&bytes, self.config.limits.max_record_bytes)?;
            if wire.registry_generation != manifest.generation {
                return Err(RegistryError::RegistryGenerationInvalid);
            }
            policies.push(policy_record_from_proto(wire, &self.config)?);
        }

        let mut accepted_roots = Vec::with_capacity(manifest.accepted_roots.len());
        for entry in &manifest.accepted_roots {
            let bytes =
                read_verified_file(registry_root, entry, &self.config, &mut aggregate_bytes)?;
            let wire =
                decode_accepted_root_record_strict(&bytes, self.config.limits.max_record_bytes)?;
            if wire.registry_generation != manifest.generation {
                return Err(RegistryError::RegistryGenerationInvalid);
            }
            accepted_roots.push(root_record_from_proto(
                wire,
                &self.config,
                observed_now_unix_seconds,
            )?);
        }

        Ok(RegistryGenerationV1 {
            generation: manifest.generation,
            publication_digest: Sha256::digest(manifest_bytes).into(),
            authority_keys,
            policies,
            accepted_roots,
        })
    }

    fn validate_manifest(
        &self,
        manifest: &proto::RegistryGenerationManifest,
        observed_now_unix_seconds: i64,
    ) -> Result<(), RegistryError> {
        if manifest.format_version != REGISTRY_MANIFEST_FORMAT_VERSION {
            return Err(RegistryError::UnsupportedFormatVersion);
        }
        if manifest.trust_domain_id != self.config.trust_domain_id {
            return Err(RegistryError::TrustDomainMismatch);
        }
        if manifest.generation == 0 {
            return Err(RegistryError::RegistryGenerationInvalid);
        }
        let published_at = manifest
            .published_at
            .as_ref()
            .ok_or(RegistryError::MissingField)?;
        if published_at.nanos != 0
            || !(PROTOBUF_TIMESTAMP_MIN_SECONDS..=PROTOBUF_TIMESTAMP_MAX_SECONDS)
                .contains(&published_at.seconds)
        {
            return Err(RegistryError::InvalidTimestamp);
        }
        validate_future_time(
            published_at.seconds,
            observed_now_unix_seconds,
            self.config.limits.max_future_clock_skew_seconds,
        )
    }
}

fn validate_entries(
    entries: &[proto::RegistryFile],
    max_files: u32,
    all_paths: &mut BTreeSet<String>,
) -> Result<(), RegistryError> {
    if entries.len() > max_files as usize {
        return Err(RegistryError::ManifestFileLimitExceeded);
    }
    if entries
        .windows(2)
        .any(|pair| pair[0].relative_path.as_bytes() >= pair[1].relative_path.as_bytes())
    {
        return Err(RegistryError::FilePathInvalid);
    }
    for entry in entries {
        validate_relative_path(&entry.relative_path)?;
        let _: [u8; 32] = entry
            .sha256
            .as_slice()
            .try_into()
            .map_err(|_| RegistryError::InvalidHashLength)?;
        if !all_paths.insert(entry.relative_path.clone()) {
            return Err(RegistryError::DuplicateRecord);
        }
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), RegistryError> {
    if value.is_empty()
        || !value.is_ascii()
        || value.contains('\\')
        || value.starts_with('/')
        || value.ends_with('/')
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(RegistryError::FilePathInvalid);
    }
    Ok(())
}

fn read_verified_file(
    registry_root: &Path,
    entry: &proto::RegistryFile,
    config: &RegistryConfig,
    aggregate_bytes: &mut u64,
) -> Result<Vec<u8>, RegistryError> {
    let path = checked_path(registry_root, &entry.relative_path)?;
    let read_limit = config
        .limits
        .max_record_bytes
        .checked_add(1)
        .ok_or(RegistryError::InputTooLarge)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| RegistryError::FileRead)?
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| RegistryError::FileRead)?;
    let byte_count = u64::try_from(bytes.len()).map_err(|_| RegistryError::InputTooLarge)?;
    if byte_count > config.limits.max_record_bytes {
        return Err(RegistryError::InputTooLarge);
    }
    *aggregate_bytes = aggregate_bytes
        .checked_add(byte_count)
        .ok_or(RegistryError::AggregateFileBytesExceeded)?;
    if *aggregate_bytes > config.limits.max_manifest_total_bytes {
        return Err(RegistryError::AggregateFileBytesExceeded);
    }
    let expected: [u8; 32] = entry
        .sha256
        .as_slice()
        .try_into()
        .map_err(|_| RegistryError::InvalidHashLength)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected {
        return Err(RegistryError::FileDigestMismatch);
    }
    Ok(bytes)
}

fn checked_path(registry_root: &Path, relative_path: &str) -> Result<PathBuf, RegistryError> {
    validate_relative_path(relative_path)?;
    let mut current = registry_root.to_path_buf();
    let parts: Vec<_> = relative_path.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        let metadata = std::fs::symlink_metadata(&current).map_err(|_| RegistryError::FileRead)?;
        if metadata.file_type().is_symlink() {
            return Err(RegistryError::SymlinkForbidden);
        }
        let is_last = index + 1 == parts.len();
        if (is_last && !metadata.is_file()) || (!is_last && !metadata.is_dir()) {
            return Err(RegistryError::FilePathInvalid);
        }
    }
    Ok(current)
}

fn validate_future_time(
    value: i64,
    trusted_now: i64,
    max_future_clock_skew_seconds: u64,
) -> Result<(), RegistryError> {
    if value <= trusted_now {
        return Ok(());
    }
    let future_seconds = value
        .checked_sub(trusted_now)
        .and_then(|seconds| u64::try_from(seconds).ok())
        .ok_or(RegistryError::InvalidTimestamp)?;
    if future_seconds > max_future_clock_skew_seconds {
        return Err(RegistryError::InvalidTimestamp);
    }
    Ok(())
}
