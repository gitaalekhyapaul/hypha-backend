use std::fs::Metadata;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_authorization_core::TRUSTED_SNAPSHOT_LIMITS;
use raziel_canonical::compare_relationship_identity;
use raziel_policy_ir::{CaveatValue, ValidatedPolicyProgram};
use raziel_snapshot::{RelationshipRecord, SnapshotTree, StoredCaveat, StoredContextEntry};
use raziel_verifier::{LocalRegistry, RegistryLoader};
use sha2::{Digest, Sha256};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use crate::config::{MAX_RECORD_BYTES, registry_config};
use crate::io::{atomic_write_new, sync_directory};
use crate::policy_authority::PolicyCandidateManifestWire;

use super::wire::{RelationshipBatchInput, SpiceDbRevisionHandoffWire, StoredValueInput};
use super::{
    CandidateConfig, CandidateWriteError, PreparedCandidate, RELATIONSHIP_BATCH_FORMAT_VERSION,
    REVISION_HANDOFF_FORMAT_VERSION,
};

const MAX_SCHEMA_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RELATIONSHIP_BATCH_BYTES: u64 = 1024 * 1024;
const MAX_TOKEN_BYTES: u64 = 4096;

pub(super) fn prepare_candidate(
    config: &CandidateConfig,
    candidate_id: &str,
    now_unix_seconds: i64,
) -> Result<PreparedCandidate, CandidateWriteError> {
    validate_public_identifier(candidate_id)?;
    validate_absolute_path(&config.candidate_root)?;
    validate_absolute_path(&config.registry_root)?;
    validate_absolute_path(&config.token_path)?;

    let root_owner = validate_private_directory(&config.candidate_root, None)?;
    let candidate_dir = config.candidate_root.join(candidate_id);
    let candidate_owner = validate_private_directory(&candidate_dir, Some(root_owner))?;

    let manifest_bytes = read_private_file(
        &candidate_dir.join("candidate.pb"),
        candidate_owner,
        MAX_RECORD_BYTES,
    )?;
    let manifest: PolicyCandidateManifestWire = decode_canonical(&manifest_bytes)?;
    validate_manifest(&manifest, candidate_id)?;

    let schema_bytes = read_private_file(
        &candidate_dir.join("schema.zed"),
        candidate_owner,
        MAX_SCHEMA_BYTES,
    )?;
    let schema = std::str::from_utf8(&schema_bytes)
        .map_err(|_| CandidateWriteError::CandidateRejected)?
        .to_owned();
    if schema.is_empty() || schema.as_bytes().contains(&0) {
        return Err(CandidateWriteError::CandidateRejected);
    }
    let schema_digest: [u8; 32] = Sha256::digest(&schema_bytes).into();
    if manifest.source_schema_digest != schema_digest {
        return Err(CandidateWriteError::CandidateRejected);
    }

    let config_for_registry = registry_config(&config.trust_domain_id)
        .map_err(|_| CandidateWriteError::InvalidConfiguration)?;
    let generation = RegistryLoader::new(config_for_registry.clone())
        .load_current(&config.registry_root, now_unix_seconds)
        .map_err(|_| CandidateWriteError::RegistryRejected)?;
    let mut registry = LocalRegistry::new(config_for_registry);
    registry
        .install_generation(generation)
        .map_err(|_| CandidateWriteError::RegistryRejected)?;
    let policy_hash: [u8; 32] = manifest
        .policy_hash
        .as_slice()
        .try_into()
        .map_err(|_| CandidateWriteError::CandidateRejected)?;
    let policy_record = registry
        .resolve_policy_exact(&manifest.record_id, &policy_hash, now_unix_seconds)
        .map_err(|_| CandidateWriteError::RegistryRejected)?;
    if policy_record.policy.program().source_schema_digest != schema_digest
        || policy_record.claims.claims().record_id != manifest.record_id
    {
        return Err(CandidateWriteError::RegistryRejected);
    }

    let relationship_path = candidate_dir.join("relationships.json");
    let relationship_bytes = read_private_file(
        &relationship_path,
        candidate_owner,
        MAX_RELATIONSHIP_BATCH_BYTES,
    )?;
    let relationship_batch: RelationshipBatchInput = serde_json::from_slice(&relationship_bytes)
        .map_err(|_| CandidateWriteError::CandidateRejected)?;
    let relationships =
        validate_relationship_batch(relationship_batch, candidate_id, &policy_record.policy)?;
    let relationship_batch_digest: [u8; 32] = Sha256::digest(&relationship_bytes).into();
    let accepted_relationship_count =
        u32::try_from(relationships.len()).map_err(|_| CandidateWriteError::CandidateRejected)?;
    let handoff_path = candidate_dir.join("spicedb-revision.pb");

    Ok(PreparedCandidate {
        candidate_id: candidate_id.to_owned(),
        policy_record_id: manifest.record_id,
        policy_hash,
        resource_type: manifest.resource_type,
        schema,
        source_schema_digest: schema_digest,
        relationship_batch_digest,
        relationships,
        accepted_relationship_count,
        handoff_path,
        candidate_owner,
    })
}

pub(super) fn persist_relationship_batch(
    candidate_root: &Path,
    candidate_id: &str,
    mut relationships: Vec<RelationshipRecord>,
) -> Result<(), CandidateWriteError> {
    validate_absolute_path(candidate_root)?;
    validate_public_identifier(candidate_id)?;
    let root_owner = validate_private_directory(candidate_root, None)?;
    let candidate_dir = candidate_root.join(candidate_id);
    let candidate_owner = validate_private_directory(&candidate_dir, Some(root_owner))?;

    relationships.sort_by(compare_relationship_identity);
    for relationship in &mut relationships {
        if let Some(caveat) = &mut relationship.stored_caveat {
            caveat
                .context
                .sort_by(|left, right| left.name.cmp(&right.name));
        }
    }
    let batch = RelationshipBatchInput {
        format_version: RELATIONSHIP_BATCH_FORMAT_VERSION,
        candidate_id: candidate_id.to_owned(),
        relationships: relationships
            .into_iter()
            .map(|relationship| super::wire::RelationshipInput {
                resource_type: relationship.resource_type,
                resource_id: relationship.resource_id,
                relation: relationship.relation,
                subject_type: relationship.subject_type,
                subject_id: relationship.subject_id,
                subject_relation: relationship.subject_relation,
                caveat: relationship
                    .stored_caveat
                    .map(|caveat| super::wire::StoredCaveatInput {
                        name: caveat.name,
                        context: caveat
                            .context
                            .into_iter()
                            .map(|entry| super::wire::StoredContextInput {
                                name: entry.name,
                                value: match entry.value {
                                    CaveatValue::Bool(value) => StoredValueInput::Bool(value),
                                    CaveatValue::Int(value) => StoredValueInput::Int(value),
                                    CaveatValue::String(value) => StoredValueInput::String(value),
                                },
                            })
                            .collect(),
                    }),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&batch).map_err(|_| CandidateWriteError::CandidateRejected)?;
    if bytes.len() as u64 > MAX_RELATIONSHIP_BATCH_BYTES {
        return Err(CandidateWriteError::CandidateRejected);
    }
    let path = candidate_dir.join("relationships.json");
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) => {
            validate_private_file_metadata(&metadata, Some(candidate_owner))?;
            let existing = read_private_file(&path, candidate_owner, MAX_RELATIONSHIP_BATCH_BYTES)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(CandidateWriteError::Conflict)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            atomic_write_new(&path, &bytes).map_err(|_| CandidateWriteError::HandoffFailed)?;
            sync_directory(&candidate_dir).map_err(|_| CandidateWriteError::HandoffFailed)
        }
        Err(_) => Err(CandidateWriteError::CandidateRejected),
    }
}

pub(super) fn read_bearer_token(path: &Path) -> Result<String, CandidateWriteError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| CandidateWriteError::InvalidCredential)?;
    #[cfg(unix)]
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(CandidateWriteError::InvalidCredential);
    }
    let file = std::fs::File::open(path).map_err(|_| CandidateWriteError::InvalidCredential)?;
    let opened_metadata = file
        .metadata()
        .map_err(|_| CandidateWriteError::InvalidCredential)?;
    #[cfg(unix)]
    if metadata.dev() != opened_metadata.dev()
        || metadata.ino() != opened_metadata.ino()
        || opened_metadata.len() > MAX_TOKEN_BYTES
    {
        return Err(CandidateWriteError::InvalidCredential);
    }
    let mut bytes = Vec::new();
    file.take(MAX_TOKEN_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CandidateWriteError::InvalidCredential)?;
    if bytes.len() as u64 > MAX_TOKEN_BYTES {
        return Err(CandidateWriteError::InvalidCredential);
    }
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| CandidateWriteError::InvalidCredential)?
        .trim();
    if value.is_empty()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == 0)
    {
        return Err(CandidateWriteError::InvalidCredential);
    }
    Ok(value.to_owned())
}

pub(super) fn load_existing_handoff(
    prepared: &PreparedCandidate,
) -> Result<Option<SpiceDbRevisionHandoffWire>, CandidateWriteError> {
    let metadata = match std::fs::symlink_metadata(&prepared.handoff_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CandidateWriteError::HandoffFailed),
    };
    validate_private_file_metadata(&metadata, Some(prepared.candidate_owner))?;
    let bytes = read_private_file(
        &prepared.handoff_path,
        prepared.candidate_owner,
        MAX_RECORD_BYTES,
    )?;
    let handoff: SpiceDbRevisionHandoffWire = decode_canonical(&bytes)?;
    validate_handoff(prepared, &handoff)?;
    Ok(Some(handoff))
}

pub(super) fn read_bound_handoff(
    candidate_root: &Path,
    candidate_id: &str,
    expected_policy_record_id: &str,
    expected_policy_hash: &[u8; 32],
) -> Result<String, CandidateWriteError> {
    validate_absolute_path(candidate_root)?;
    validate_public_identifier(candidate_id)?;
    validate_public_identifier(expected_policy_record_id)?;
    let root_owner = validate_private_directory(candidate_root, None)?;
    let candidate_dir = candidate_root.join(candidate_id);
    let candidate_owner = validate_private_directory(&candidate_dir, Some(root_owner))?;
    let bytes = read_private_file(
        &candidate_dir.join("spicedb-revision.pb"),
        candidate_owner,
        MAX_RECORD_BYTES,
    )?;
    let handoff: SpiceDbRevisionHandoffWire = decode_canonical(&bytes)?;
    validate_handoff_shape(&handoff, candidate_id)?;
    if handoff.policy_record_id != expected_policy_record_id
        || handoff.policy_hash != expected_policy_hash
    {
        return Err(CandidateWriteError::CandidateRejected);
    }
    Ok(handoff.zed_token)
}

pub(super) fn write_handoff(
    prepared: &PreparedCandidate,
    zed_token: String,
) -> Result<(), CandidateWriteError> {
    if zed_token.is_empty()
        || zed_token.len() > MAX_TOKEN_BYTES as usize
        || !zed_token.is_ascii()
        || zed_token.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(CandidateWriteError::MissingRevision);
    }
    let handoff = SpiceDbRevisionHandoffWire {
        format_version: REVISION_HANDOFF_FORMAT_VERSION,
        candidate_id: prepared.candidate_id.clone(),
        policy_record_id: prepared.policy_record_id.clone(),
        policy_hash: prepared.policy_hash.to_vec(),
        source_schema_digest: prepared.source_schema_digest.to_vec(),
        relationship_batch_digest: prepared.relationship_batch_digest.to_vec(),
        accepted_relationship_count: prepared.accepted_relationship_count,
        zed_token,
    };
    atomic_write_new(&prepared.handoff_path, &handoff.encode_to_vec())
        .map_err(|_| CandidateWriteError::HandoffFailed)
}

pub(super) fn validate_relationship_batch(
    batch: RelationshipBatchInput,
    candidate_id: &str,
    policy: &ValidatedPolicyProgram,
) -> Result<Vec<RelationshipRecord>, CandidateWriteError> {
    if batch.format_version != RELATIONSHIP_BATCH_FORMAT_VERSION
        || batch.candidate_id != candidate_id
        || batch.relationships.is_empty()
        || batch.relationships.len() > TRUSTED_SNAPSHOT_LIMITS.max_relationships as usize
    {
        return Err(CandidateWriteError::CandidateRejected);
    }
    let relationships = batch
        .relationships
        .into_iter()
        .map(|value| RelationshipRecord {
            resource_type: value.resource_type,
            resource_id: value.resource_id,
            relation: value.relation,
            subject_type: value.subject_type,
            subject_id: value.subject_id,
            subject_relation: value.subject_relation,
            stored_caveat: value.caveat.map(|caveat| StoredCaveat {
                name: caveat.name,
                context: caveat
                    .context
                    .into_iter()
                    .map(|entry| StoredContextEntry {
                        name: entry.name,
                        value: match entry.value {
                            StoredValueInput::Bool(value) => CaveatValue::Bool(value),
                            StoredValueInput::Int(value) => CaveatValue::Int(value),
                            StoredValueInput::String(value) => CaveatValue::String(value),
                        },
                    })
                    .collect(),
            }),
        })
        .collect::<Vec<_>>();
    SnapshotTree::build(
        policy,
        relationships.clone(),
        &[0; 32],
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .map_err(|_| CandidateWriteError::CandidateRejected)?;
    Ok(relationships)
}

fn validate_manifest(
    manifest: &PolicyCandidateManifestWire,
    candidate_id: &str,
) -> Result<(), CandidateWriteError> {
    if manifest.format_version != 1
        || manifest.candidate_id != candidate_id
        || manifest.record_id.is_empty()
        || manifest.record_id.len() > 256
        || manifest.resource_type.is_empty()
        || manifest.resource_type.len() > 256
        || manifest.source_schema_digest.len() != 32
        || manifest.policy_hash.len() != 32
    {
        return Err(CandidateWriteError::CandidateRejected);
    }
    Ok(())
}

fn validate_handoff(
    prepared: &PreparedCandidate,
    handoff: &SpiceDbRevisionHandoffWire,
) -> Result<(), CandidateWriteError> {
    validate_handoff_shape(handoff, &prepared.candidate_id)?;
    if handoff.policy_record_id != prepared.policy_record_id
        || handoff.policy_hash != prepared.policy_hash
        || handoff.source_schema_digest != prepared.source_schema_digest
        || handoff.relationship_batch_digest != prepared.relationship_batch_digest
        || handoff.accepted_relationship_count != prepared.accepted_relationship_count
    {
        return Err(CandidateWriteError::HandoffFailed);
    }
    Ok(())
}

fn validate_handoff_shape(
    handoff: &SpiceDbRevisionHandoffWire,
    candidate_id: &str,
) -> Result<(), CandidateWriteError> {
    if handoff.format_version != REVISION_HANDOFF_FORMAT_VERSION
        || handoff.candidate_id != candidate_id
        || !valid_public_identifier_value(&handoff.policy_record_id)
        || handoff.policy_hash.len() != 32
        || handoff.source_schema_digest.len() != 32
        || handoff.relationship_batch_digest.len() != 32
        || handoff.accepted_relationship_count == 0
        || handoff.zed_token.is_empty()
        || handoff.zed_token.len() > MAX_TOKEN_BYTES as usize
        || !handoff.zed_token.is_ascii()
        || handoff
            .zed_token
            .bytes()
            .any(|byte| byte.is_ascii_control())
    {
        return Err(CandidateWriteError::HandoffFailed);
    }
    Ok(())
}

fn validate_absolute_path(path: &Path) -> Result<(), CandidateWriteError> {
    if path.is_absolute()
        && path != Path::new("/")
        && !path.as_os_str().as_encoded_bytes().contains(&0)
    {
        Ok(())
    } else {
        Err(CandidateWriteError::InvalidConfiguration)
    }
}

fn validate_public_identifier(value: &str) -> Result<(), CandidateWriteError> {
    if valid_public_identifier_value(value) {
        Ok(())
    } else {
        Err(CandidateWriteError::InvalidCandidateId)
    }
}

fn valid_public_identifier_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
}

fn validate_private_directory(
    path: &Path,
    expected_owner: Option<u32>,
) -> Result<u32, CandidateWriteError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| CandidateWriteError::CandidateRejected)?;
    #[cfg(unix)]
    {
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.permissions().mode() & 0o7777 != 0o700
            || expected_owner.is_some_and(|owner| metadata.uid() != owner)
        {
            return Err(CandidateWriteError::CandidateRejected);
        }
        Ok(metadata.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = expected_owner;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CandidateWriteError::CandidateRejected);
        }
        Ok(0)
    }
}

fn read_private_file(
    path: &Path,
    expected_owner: u32,
    max_bytes: u64,
) -> Result<Vec<u8>, CandidateWriteError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| CandidateWriteError::CandidateRejected)?;
    validate_private_file_metadata(&metadata, Some(expected_owner))?;
    let file = std::fs::File::open(path).map_err(|_| CandidateWriteError::CandidateRejected)?;
    let opened_metadata = file
        .metadata()
        .map_err(|_| CandidateWriteError::CandidateRejected)?;
    #[cfg(unix)]
    if opened_metadata.dev() != metadata.dev() || opened_metadata.ino() != metadata.ino() {
        return Err(CandidateWriteError::CandidateRejected);
    }
    let limit = max_bytes
        .checked_add(1)
        .ok_or(CandidateWriteError::CandidateRejected)?;
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| CandidateWriteError::CandidateRejected)?;
    if bytes.len() as u64 > max_bytes {
        return Err(CandidateWriteError::CandidateRejected);
    }
    Ok(bytes)
}

fn validate_private_file_metadata(
    metadata: &Metadata,
    expected_owner: Option<u32>,
) -> Result<u32, CandidateWriteError> {
    #[cfg(unix)]
    {
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.permissions().mode() & 0o7777 != 0o600
            || expected_owner.is_some_and(|owner| metadata.uid() != owner)
        {
            return Err(CandidateWriteError::CandidateRejected);
        }
        Ok(metadata.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = expected_owner;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CandidateWriteError::CandidateRejected);
        }
        Ok(0)
    }
}

fn decode_canonical<M: Message + Default>(bytes: &[u8]) -> Result<M, CandidateWriteError> {
    let value = M::decode(bytes).map_err(|_| CandidateWriteError::CandidateRejected)?;
    if value.encode_to_vec() != bytes {
        return Err(CandidateWriteError::CandidateRejected);
    }
    Ok(value)
}

pub(super) fn trusted_now() -> Result<i64, CandidateWriteError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CandidateWriteError::Unavailable)
        .and_then(|value| {
            i64::try_from(value.as_secs()).map_err(|_| CandidateWriteError::Unavailable)
        })
}

pub(super) fn configured_path(name: &str) -> Result<PathBuf, CandidateWriteError> {
    let value = std::env::var(name).map_err(|_| CandidateWriteError::InvalidConfiguration)?;
    let path = PathBuf::from(value);
    validate_absolute_path(&path)?;
    Ok(path)
}
