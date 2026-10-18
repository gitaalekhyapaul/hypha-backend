use std::io::ErrorKind;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_proto::raziel::v1 as proto;
use raziel_verifier::{
    PolicyRecordStatus, RegistryConfig, RegistryGenerationV1, RegistryLoader,
    policy_record_to_proto,
};

use crate::config::{MAX_RECORD_BYTES, registry_config};
use crate::io::{atomic_write_new, lowercase_hex, read_bounded, sync_directory};
use crate::policy_authority::{
    PolicyAuthorityApprovalRequest, PolicyAuthorityClient, PolicyAuthorityClientError,
    PolicyCandidateManifestWire,
};
use crate::writer::{RegistryWriterClient, RegistryWriterClientError};

use super::wire::{PolicyActivationEnvelopeWire, PolicyActivationRequestWire};
use super::{PROTOCOL_VERSION, valid_absolute_path};

const ENVELOPE_FORMAT_VERSION: u32 = 1;
const CANDIDATE_MANIFEST_FORMAT_VERSION: u32 = 1;
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_APPROVED_BY_BYTES: usize = 4096;
const WRITER_ATTEMPTS: usize = 3;

pub(crate) struct PolicyActivationStore {
    candidate_root: PathBuf,
    activation_root: PathBuf,
    registry_root: PathBuf,
    registry_config: RegistryConfig,
    authority: PolicyAuthorityClient,
    writer: RegistryWriterClient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoredActivationProjection {
    pub record_id: String,
    pub policy_name: String,
    pub policy_version: String,
    pub policy_hash: String,
    pub approval_epoch: u64,
    pub registered_at_unix_seconds: i64,
    pub valid_from_unix_seconds: i64,
    pub valid_until_unix_seconds: i64,
    pub registry_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreActivationError {
    InvalidRequest,
    Conflict,
    CandidateRejected,
    Unavailable,
}

impl PolicyActivationStore {
    pub(crate) fn new(
        candidate_root: PathBuf,
        activation_root: PathBuf,
        registry_root: PathBuf,
        authority_socket: PathBuf,
        writer_socket: PathBuf,
        trust_domain_id: &str,
    ) -> Result<Self, StoreActivationError> {
        if !valid_absolute_path(&candidate_root)
            || !valid_absolute_path(&activation_root)
            || !valid_absolute_path(&registry_root)
        {
            return Err(StoreActivationError::InvalidRequest);
        }
        let registry_config =
            registry_config(trust_domain_id).map_err(|_| StoreActivationError::InvalidRequest)?;
        let authority = PolicyAuthorityClient::new(authority_socket)
            .map_err(|_| StoreActivationError::InvalidRequest)?;
        let writer = RegistryWriterClient::new(writer_socket)
            .map_err(|_| StoreActivationError::InvalidRequest)?;
        Ok(Self {
            candidate_root,
            activation_root,
            registry_root,
            registry_config,
            authority,
            writer,
        })
    }

    pub(super) fn activate(
        &self,
        request: PolicyActivationRequestWire,
    ) -> Result<StoredActivationProjection, StoreActivationError> {
        self.activate_inner(request, false)
    }

    pub(crate) fn activate_version(
        &self,
        request: PolicyActivationRequestWire,
    ) -> Result<StoredActivationProjection, StoreActivationError> {
        self.activate_inner(request, true)
    }

    fn activate_inner(
        &self,
        request: PolicyActivationRequestWire,
        versioned: bool,
    ) -> Result<StoredActivationProjection, StoreActivationError> {
        validate_request(&request)?;
        let now = trusted_now()?;
        let manifest = self.load_manifest(&request.candidate_id)?;
        // Versioned packages need the composed-schema publication boundary. The
        // legacy M5 activation path must not approve them as temporary candidates.
        if manifest.policy_name.starts_with("family:") != versioned {
            return Err(StoreActivationError::CandidateRejected);
        }
        let current = self.load_registry(now)?;
        if let Some(record) = existing_record(&current, &manifest)? {
            self.remove_envelope_if_present(&request.candidate_id)?;
            return projection(&record);
        }

        let nonce = self.load_or_create_nonce(&request.candidate_id)?;
        let expected_policy_hash = manifest
            .policy_hash
            .as_slice()
            .try_into()
            .map_err(|_| StoreActivationError::CandidateRejected)?;
        let staged = self
            .authority
            .approve(PolicyAuthorityApprovalRequest {
                candidate_id: request.candidate_id.clone(),
                expected_policy_hash,
                expected_approval_epoch: manifest.expected_approval_epoch,
                valid_from_unix_seconds: manifest.valid_from_unix_seconds,
                valid_until_unix_seconds: manifest.valid_until_unix_seconds,
                approved_by: request.approved_by,
                approval_nonce: nonce,
            })
            .map_err(map_authority_error)?
            .staged_policy_record;

        for _ in 0..WRITER_ATTEMPTS {
            let current = self.load_registry(trusted_now()?)?;
            if let Some(record) = existing_record(&current, &manifest)? {
                self.remove_envelope(&request.candidate_id)?;
                return projection(&record);
            }
            match self
                .writer
                .append_approved_policy(current.generation, staged.clone())
            {
                Ok(result) => {
                    self.remove_envelope(&request.candidate_id)?;
                    return projection(&result.policy_record);
                }
                Err(RegistryWriterClientError::StaleGeneration) => continue,
                Err(RegistryWriterClientError::Conflict) => {
                    let current = self.load_registry(trusted_now()?)?;
                    if let Some(record) = existing_record(&current, &manifest)? {
                        self.remove_envelope(&request.candidate_id)?;
                        return projection(&record);
                    }
                    return Err(StoreActivationError::Conflict);
                }
                Err(error) => return Err(map_writer_error(error)),
            }
        }
        Err(StoreActivationError::Unavailable)
    }

    fn load_manifest(
        &self,
        candidate_id: &str,
    ) -> Result<PolicyCandidateManifestWire, StoreActivationError> {
        let root_owner = private_directory_owner(&self.candidate_root, None)?;
        let directory = self.candidate_root.join(candidate_id);
        let owner = private_directory_owner(&directory, Some(root_owner))?;
        let path = directory.join("candidate.pb");
        let bytes = read_private_file(&path, owner, MAX_RECORD_BYTES)?;
        let manifest = PolicyCandidateManifestWire::decode(bytes.as_slice())
            .map_err(|_| StoreActivationError::CandidateRejected)?;
        if manifest.encode_to_vec() != bytes
            || manifest.format_version != CANDIDATE_MANIFEST_FORMAT_VERSION
            || manifest.candidate_id != candidate_id
            || !valid_identifier(&manifest.record_id)
            || !valid_identifier(&manifest.policy_name)
            || manifest.policy_version.is_empty()
            || manifest.policy_hash.len() != 32
            || manifest.expected_approval_epoch == 0
            || manifest.valid_from_unix_seconds <= 0
            || manifest.valid_until_unix_seconds <= manifest.valid_from_unix_seconds
        {
            return Err(StoreActivationError::CandidateRejected);
        }
        Ok(manifest)
    }

    fn load_registry(
        &self,
        now_unix_seconds: i64,
    ) -> Result<RegistryGenerationV1, StoreActivationError> {
        RegistryLoader::new(self.registry_config.clone())
            .load_current(&self.registry_root, now_unix_seconds)
            .map_err(|_| StoreActivationError::Unavailable)
    }

    fn load_or_create_nonce(&self, candidate_id: &str) -> Result<[u8; 32], StoreActivationError> {
        let owner = private_directory_owner(&self.activation_root, None)?;
        let path = envelope_path(&self.activation_root, candidate_id);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return read_envelope(&path, owner, candidate_id),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(StoreActivationError::Unavailable),
        }
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(|_| StoreActivationError::Unavailable)?;
        let envelope = PolicyActivationEnvelopeWire {
            format_version: ENVELOPE_FORMAT_VERSION,
            candidate_id: candidate_id.into(),
            approval_nonce: nonce.to_vec(),
        };
        match atomic_write_new(&path, &envelope.encode_to_vec()) {
            Ok(()) => Ok(nonce),
            Err(_) if path.exists() => read_envelope(&path, owner, candidate_id),
            Err(_) => Err(StoreActivationError::Unavailable),
        }
    }

    fn remove_envelope_if_present(&self, candidate_id: &str) -> Result<(), StoreActivationError> {
        let path = envelope_path(&self.activation_root, candidate_id);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => self.remove_envelope(candidate_id),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => Err(StoreActivationError::Unavailable),
        }
    }

    fn remove_envelope(&self, candidate_id: &str) -> Result<(), StoreActivationError> {
        let owner = private_directory_owner(&self.activation_root, None)?;
        let path = envelope_path(&self.activation_root, candidate_id);
        let _ = read_envelope(&path, owner, candidate_id)?;
        std::fs::remove_file(&path).map_err(|_| StoreActivationError::Unavailable)?;
        sync_directory(&self.activation_root).map_err(|_| StoreActivationError::Unavailable)
    }
}

fn existing_record(
    generation: &RegistryGenerationV1,
    manifest: &PolicyCandidateManifestWire,
) -> Result<Option<proto::PolicyRecord>, StoreActivationError> {
    let Some(record) = generation
        .policies
        .iter()
        .find(|record| record.claims.claims().record_id == manifest.record_id)
    else {
        return Ok(None);
    };
    let claims = record.claims.claims();
    if record.status != PolicyRecordStatus::Approved
        || claims.policy_name != manifest.policy_name
        || claims.policy_version != manifest.policy_version
        || claims.policy_hash.as_slice() != manifest.policy_hash
        || claims.approval_epoch != manifest.expected_approval_epoch
        || claims.valid_from_unix_seconds != manifest.valid_from_unix_seconds
        || claims.valid_until_unix_seconds != manifest.valid_until_unix_seconds
    {
        return Err(StoreActivationError::Conflict);
    }
    policy_record_to_proto(record)
        .map(Some)
        .map_err(|_| StoreActivationError::Unavailable)
}

fn projection(
    record: &proto::PolicyRecord,
) -> Result<StoredActivationProjection, StoreActivationError> {
    let registered_at = required_timestamp(record.registered_at.as_ref())?;
    let valid_from = required_timestamp(record.valid_from.as_ref())?;
    let valid_until = required_timestamp(record.valid_until.as_ref())?;
    if record.status != proto::PolicyStatus::Approved as i32
        || record.policy_hash.len() != 32
        || record.approval_epoch == 0
        || record.registry_generation == 0
    {
        return Err(StoreActivationError::Unavailable);
    }
    Ok(StoredActivationProjection {
        record_id: record.record_id.clone(),
        policy_name: record.policy_name.clone(),
        policy_version: record.policy_version.clone(),
        policy_hash: lowercase_hex(&record.policy_hash),
        approval_epoch: record.approval_epoch,
        registered_at_unix_seconds: registered_at,
        valid_from_unix_seconds: valid_from,
        valid_until_unix_seconds: valid_until,
        registry_generation: record.registry_generation,
    })
}

fn validate_request(request: &PolicyActivationRequestWire) -> Result<(), StoreActivationError> {
    if request.format_version != PROTOCOL_VERSION
        || !valid_identifier(&request.candidate_id)
        || request.approved_by.is_empty()
        || request.approved_by.len() > MAX_APPROVED_BY_BYTES
        || !request.approved_by.is_ascii()
        || request
            .approved_by
            .bytes()
            .any(|byte| byte.is_ascii_control())
    {
        return Err(StoreActivationError::InvalidRequest);
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.is_ascii()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && b"._:-".contains(&byte))
        })
}

fn private_directory_owner(
    path: &Path,
    expected_owner: Option<u32>,
) -> Result<u32, StoreActivationError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| StoreActivationError::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o7777 != PRIVATE_DIRECTORY_MODE
        || expected_owner.is_some_and(|owner| owner != metadata.uid())
    {
        return Err(StoreActivationError::Unavailable);
    }
    Ok(metadata.uid())
}

fn read_private_file(
    path: &Path,
    owner: u32,
    max_bytes: u64,
) -> Result<Vec<u8>, StoreActivationError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreActivationError::CandidateRejected
        } else {
            StoreActivationError::Unavailable
        }
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o7777 != PRIVATE_FILE_MODE
        || metadata.uid() != owner
        || metadata.len() == 0
        || metadata.len() > max_bytes
    {
        return Err(StoreActivationError::CandidateRejected);
    }
    read_bounded(path, max_bytes).map_err(|_| StoreActivationError::CandidateRejected)
}

fn read_envelope(
    path: &Path,
    owner: u32,
    candidate_id: &str,
) -> Result<[u8; 32], StoreActivationError> {
    let bytes = read_private_file(path, owner, MAX_RECORD_BYTES)?;
    let envelope = PolicyActivationEnvelopeWire::decode(bytes.as_slice())
        .map_err(|_| StoreActivationError::Conflict)?;
    if envelope.encode_to_vec() != bytes
        || envelope.format_version != ENVELOPE_FORMAT_VERSION
        || envelope.candidate_id != candidate_id
    {
        return Err(StoreActivationError::Conflict);
    }
    envelope
        .approval_nonce
        .as_slice()
        .try_into()
        .map_err(|_| StoreActivationError::Conflict)
}

fn envelope_path(root: &Path, candidate_id: &str) -> PathBuf {
    root.join(format!("{candidate_id}.pb"))
}

fn required_timestamp(value: Option<&prost_types::Timestamp>) -> Result<i64, StoreActivationError> {
    let value = value.ok_or(StoreActivationError::Unavailable)?;
    if value.seconds <= 0 || value.nanos != 0 {
        return Err(StoreActivationError::Unavailable);
    }
    Ok(value.seconds)
}

pub(crate) fn trusted_now() -> Result<i64, StoreActivationError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreActivationError::Unavailable)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| StoreActivationError::Unavailable)
}

fn map_authority_error(error: PolicyAuthorityClientError) -> StoreActivationError {
    match error {
        PolicyAuthorityClientError::InvalidRequest => StoreActivationError::InvalidRequest,
        PolicyAuthorityClientError::StaleEpoch | PolicyAuthorityClientError::Conflict => {
            StoreActivationError::Conflict
        }
        PolicyAuthorityClientError::CandidateRejected => StoreActivationError::CandidateRejected,
        PolicyAuthorityClientError::InvalidConfiguration
        | PolicyAuthorityClientError::Unavailable
        | PolicyAuthorityClientError::Protocol => StoreActivationError::Unavailable,
    }
}

fn map_writer_error(error: RegistryWriterClientError) -> StoreActivationError {
    match error {
        RegistryWriterClientError::InvalidRequest | RegistryWriterClientError::Conflict => {
            StoreActivationError::Conflict
        }
        RegistryWriterClientError::InvalidConfiguration
        | RegistryWriterClientError::Unavailable
        | RegistryWriterClientError::Protocol
        | RegistryWriterClientError::StaleGeneration
        | RegistryWriterClientError::RegistryRejected
        | RegistryWriterClientError::PublicationFailed => StoreActivationError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;

    #[test]
    fn versioned_candidate_cannot_bypass_composed_publication_through_legacy_activation() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("raziel-activation-{}-{unique}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let candidates = root.join("candidates");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&candidates)
            .unwrap();
        let directory = candidates.join("candidate-1");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let manifest = PolicyCandidateManifestWire {
            format_version: 1,
            candidate_id: "candidate-1".into(),
            record_id: "record-1".into(),
            policy_name: format!("family:{}:v1", "12".repeat(16)),
            policy_version: "1".into(),
            policy_hash: vec![1; 32],
            expected_approval_epoch: 1,
            valid_from_unix_seconds: 1,
            valid_until_unix_seconds: i64::MAX,
            ..Default::default()
        };
        atomic_write_new(&directory.join("candidate.pb"), &manifest.encode_to_vec()).unwrap();
        let store = PolicyActivationStore::new(
            candidates,
            root.join("activation"),
            root.join("registry"),
            root.join("authority.sock"),
            root.join("writer.sock"),
            "test-version-publication",
        )
        .unwrap();
        // No registry or sockets exist. Rejection must precede authority and writer access.
        assert_eq!(
            store.activate(PolicyActivationRequestWire {
                format_version: 1,
                candidate_id: "candidate-1".into(),
                approved_by: "operator-test".into(),
                operation: 0,
                role_request: Vec::new(),
            }),
            Err(StoreActivationError::CandidateRejected)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
