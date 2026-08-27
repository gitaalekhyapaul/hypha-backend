use std::fs::{DirBuilder, Metadata};
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use prost_types::Timestamp;
use raziel_authority::{AuthorityKeyStatus, AuthorityRole};
use raziel_authorization_core::TRUSTED_SNAPSHOT_LIMITS;
use raziel_canonical::{RelationshipRecord, StoredCaveat, StoredContextEntry};
use raziel_policy_ir::CaveatValue;
use raziel_proto::raziel::v1::BuildSnapshotRequest;
use raziel_proto::raziel::v1::snapshot_service_client::SnapshotServiceClient;
use raziel_verifier::{PolicyRecordStatus, RegistryGenerationV1, RegistryLoader, RootRecordStatus};
use sha2::{Digest, Sha256};

use super::codec::{read_request, write_response};
use super::wire::{
    AccessGraphFailureWire, AccessGraphProjectionWire, AccessGraphRequestWire,
    AccessGraphResponseWire, AccessGraphStateWire, RelationshipWire, stored_context_wire,
};
use super::{PROTOCOL_VERSION, valid_absolute_path};
use crate::config::registry_config;
use crate::io::{atomic_write_new, lowercase_hex, read_bounded};
use crate::spicedb_candidate::{
    CandidateRevisionHandoffReader, CandidateWriteError, CandidateWriteResult,
    publish_candidate_from_env, write_relationship_batch,
};

const SOCKET_DIRECTORY_MODE: u32 = 0o700;
const SOCKET_MODE: u32 = 0o600;
const STATE_FILE_MODE: u32 = 0o600;
const MAX_STATE_BYTES: u64 = 64 * 1024;
const MIN_VALIDITY_SECONDS: u32 = 15 * 60;
const MAX_VALIDITY_SECONDS: u32 = 24 * 60 * 60;
const MAX_IDENTIFIER_BYTES: usize = 256;

pub async fn run_from_env() -> Result<(), AccessGraphServerError> {
    let service = AccessGraphPublicationService::from_env()?;
    let listener = bind_private_socket(&service.socket_path)?;
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            continue;
        };
        let response = handle_connection(&service, &mut stream).await;
        let _ = write_response(&mut stream, &response);
    }
    Err(AccessGraphServerError::Listener)
}

async fn handle_connection(
    service: &AccessGraphPublicationService,
    stream: &mut UnixStream,
) -> AccessGraphResponseWire {
    if stream
        .set_read_timeout(Some(super::SOCKET_TIMEOUT))
        .is_err()
        || stream
            .set_write_timeout(Some(super::SOCKET_TIMEOUT))
            .is_err()
    {
        return failure(AccessGraphFailureWire::InvalidRequest);
    }
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(_) => return failure(AccessGraphFailureWire::InvalidRequest),
    };
    match service.publish(request).await {
        Ok(projection) => AccessGraphResponseWire {
            format_version: PROTOCOL_VERSION,
            failure: AccessGraphFailureWire::Unspecified as i32,
            projection: Some(projection),
        },
        Err(error) => failure(match error {
            AccessGraphPublicationError::InvalidRequest => AccessGraphFailureWire::InvalidRequest,
            AccessGraphPublicationError::Conflict => AccessGraphFailureWire::Conflict,
            AccessGraphPublicationError::CandidateRejected => {
                AccessGraphFailureWire::CandidateRejected
            }
            AccessGraphPublicationError::Unavailable => AccessGraphFailureWire::Unavailable,
        }),
    }
}

fn failure(code: AccessGraphFailureWire) -> AccessGraphResponseWire {
    AccessGraphResponseWire {
        format_version: PROTOCOL_VERSION,
        failure: code as i32,
        projection: None,
    }
}

struct AccessGraphPublicationService {
    candidate_root: PathBuf,
    state_root: PathBuf,
    registry_root: PathBuf,
    socket_path: PathBuf,
    snapshot_endpoint: String,
    spicedb_endpoint_ref: String,
    trust_domain_id: String,
}

impl AccessGraphPublicationService {
    fn from_env() -> Result<Self, AccessGraphServerError> {
        let candidate_root = required_path("RAZIEL_POLICY_CANDIDATE_ROOT")?;
        let state_root = required_path("RAZIEL_ACCESS_GRAPH_STATE_ROOT")?;
        let registry_root = required_path("RAZIEL_REGISTRY_PATH")?;
        let socket_path = required_path("RAZIEL_ACCESS_GRAPH_SOCKET_PATH")?;
        validate_private_directory(
            &std::fs::symlink_metadata(&state_root)
                .map_err(|_| AccessGraphServerError::Configuration)?,
        )?;
        let snapshot_endpoint = required_endpoint("RAZIEL_SNAPSHOT_ENDPOINT")?;
        let spicedb_endpoint_ref = required_identifier_env("RAZIEL_SPICEDB_ENDPOINT_REF")?;
        let trust_domain_id = required_identifier_env("RAZIEL_TRUST_DOMAIN_ID")?;
        registry_config(&trust_domain_id).map_err(|_| AccessGraphServerError::Configuration)?;
        Ok(Self {
            candidate_root,
            state_root,
            registry_root,
            socket_path,
            snapshot_endpoint,
            spicedb_endpoint_ref,
            trust_domain_id,
        })
    }

    async fn publish(
        &self,
        request: AccessGraphRequestWire,
    ) -> Result<AccessGraphProjectionWire, AccessGraphPublicationError> {
        validate_request(&request)?;
        let request_digest: [u8; 32] = Sha256::digest(request.encode_to_vec()).into();
        if let Some(stored) = self.load_state(&request.candidate_id)? {
            if stored.request_digest != request_digest {
                return Err(AccessGraphPublicationError::Conflict);
            }
            return stored
                .projection
                .ok_or(AccessGraphPublicationError::Unavailable);
        }

        let relationships = request
            .relationships
            .iter()
            .map(relationship_from_wire)
            .collect::<Result<Vec<_>, _>>()?;
        write_relationship_batch(&self.candidate_root, &request.candidate_id, relationships)
            .map_err(map_candidate_error)?;
        let candidate = publish_candidate_from_env(&request.candidate_id)
            .await
            .map_err(map_candidate_error)?;
        let now = trusted_now()?;
        let current = self.load_registry(now)?;
        let revision = CandidateRevisionHandoffReader::new(self.candidate_root.clone())
            .map_err(|_| AccessGraphPublicationError::Unavailable)?
            .read_bound(
                &request.candidate_id,
                &candidate.policy_record_id,
                candidate.policy_hash(),
            )
            .map_err(|_| AccessGraphPublicationError::Unavailable)?;

        if let Some(projection) =
            projection_for_revision(&current, &candidate, revision.zed_token())
        {
            self.store_state(&request.candidate_id, request_digest, &projection)?;
            return Ok(projection);
        }

        let (authority_key_id, expected_previous_epoch, expires_at) =
            snapshot_inputs(&current, &candidate, now, request.valid_for_seconds)?;
        let mut client = SnapshotServiceClient::connect(self.snapshot_endpoint.clone())
            .await
            .map_err(|_| AccessGraphPublicationError::Unavailable)?;
        let response = client
            .build_snapshot(BuildSnapshotRequest {
                spicedb_endpoint_ref: self.spicedb_endpoint_ref.clone(),
                zed_token_secret_ref: request.candidate_id.clone(),
                policy_record_id: candidate.policy_record_id.clone(),
                policy_hash: candidate.policy_hash().to_vec(),
                expires_at: Some(Timestamp {
                    seconds: expires_at,
                    nanos: 0,
                }),
                expected_previous_epoch,
                authority_key_id,
            })
            .await
            .map_err(|_| AccessGraphPublicationError::Unavailable)?
            .into_inner();
        let snapshot_id = response
            .snapshot
            .ok_or(AccessGraphPublicationError::Unavailable)?
            .snapshot_id;
        let confirmed = self.load_registry(trusted_now()?)?;
        let projection = projection_for_snapshot(&confirmed, &candidate, &snapshot_id)
            .ok_or(AccessGraphPublicationError::Unavailable)?;
        self.store_state(&request.candidate_id, request_digest, &projection)?;
        Ok(projection)
    }

    fn load_registry(&self, now: i64) -> Result<RegistryGenerationV1, AccessGraphPublicationError> {
        let config = registry_config(&self.trust_domain_id)
            .map_err(|_| AccessGraphPublicationError::Unavailable)?;
        RegistryLoader::new(config)
            .load_current(&self.registry_root, now)
            .map_err(|_| AccessGraphPublicationError::Unavailable)
    }

    fn load_state(
        &self,
        candidate_id: &str,
    ) -> Result<Option<AccessGraphStateWire>, AccessGraphPublicationError> {
        let path = self.state_root.join(format!("{candidate_id}.pb"));
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccessGraphPublicationError::Unavailable),
        };
        validate_private_state_file(&metadata)?;
        let bytes = read_bounded(&path, MAX_STATE_BYTES)
            .map_err(|_| AccessGraphPublicationError::Unavailable)?;
        let state = AccessGraphStateWire::decode(bytes.as_slice())
            .map_err(|_| AccessGraphPublicationError::Unavailable)?;
        if state.encode_to_vec() != bytes
            || state.format_version != PROTOCOL_VERSION
            || state.candidate_id != candidate_id
            || state.request_digest.len() != 32
        {
            return Err(AccessGraphPublicationError::Unavailable);
        }
        Ok(Some(state))
    }

    fn store_state(
        &self,
        candidate_id: &str,
        request_digest: [u8; 32],
        projection: &AccessGraphProjectionWire,
    ) -> Result<(), AccessGraphPublicationError> {
        let state = AccessGraphStateWire {
            format_version: PROTOCOL_VERSION,
            candidate_id: candidate_id.to_owned(),
            request_digest: request_digest.to_vec(),
            projection: Some(projection.clone()),
        };
        let path = self.state_root.join(format!("{candidate_id}.pb"));
        match atomic_write_new(&path, &state.encode_to_vec()) {
            Ok(()) => Ok(()),
            Err(_) => {
                let stored = self
                    .load_state(candidate_id)?
                    .ok_or(AccessGraphPublicationError::Unavailable)?;
                if stored.request_digest == request_digest
                    && stored.projection.as_ref() == Some(projection)
                {
                    Ok(())
                } else {
                    Err(AccessGraphPublicationError::Conflict)
                }
            }
        }
    }
}

fn validate_request(request: &AccessGraphRequestWire) -> Result<(), AccessGraphPublicationError> {
    if request.format_version != PROTOCOL_VERSION
        || !valid_identifier(&request.candidate_id)
        || !valid_identifier(&request.operator_id)
        || request.valid_for_seconds < MIN_VALIDITY_SECONDS
        || request.valid_for_seconds > MAX_VALIDITY_SECONDS
        || request.relationships.is_empty()
        || request.relationships.len() > TRUSTED_SNAPSHOT_LIMITS.max_relationships as usize
    {
        return Err(AccessGraphPublicationError::InvalidRequest);
    }
    Ok(())
}

fn relationship_from_wire(
    value: &RelationshipWire,
) -> Result<RelationshipRecord, AccessGraphPublicationError> {
    for identifier in [
        value.resource_type.as_str(),
        value.resource_id.as_str(),
        value.relation.as_str(),
        value.subject_type.as_str(),
        value.subject_id.as_str(),
    ] {
        if !valid_identifier(identifier) {
            return Err(AccessGraphPublicationError::InvalidRequest);
        }
    }
    if value
        .subject_relation
        .as_deref()
        .is_some_and(|relation| !valid_identifier(relation))
    {
        return Err(AccessGraphPublicationError::InvalidRequest);
    }
    let stored_caveat = value
        .caveat
        .as_ref()
        .map(|caveat| {
            if !valid_identifier(&caveat.name) {
                return Err(AccessGraphPublicationError::InvalidRequest);
            }
            let context = caveat
                .context
                .iter()
                .map(|entry| {
                    if !valid_identifier(&entry.name) {
                        return Err(AccessGraphPublicationError::InvalidRequest);
                    }
                    let value = match entry.value.as_ref() {
                        Some(stored_context_wire::Value::Bool(value)) => CaveatValue::Bool(*value),
                        Some(stored_context_wire::Value::Int(value)) => CaveatValue::Int(*value),
                        Some(stored_context_wire::Value::String(value))
                            if value.len() <= MAX_IDENTIFIER_BYTES
                                && !value.as_bytes().contains(&0) =>
                        {
                            CaveatValue::String(value.clone())
                        }
                        _ => return Err(AccessGraphPublicationError::InvalidRequest),
                    };
                    Ok(StoredContextEntry {
                        name: entry.name.clone(),
                        value,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StoredCaveat {
                name: caveat.name.clone(),
                context,
            })
        })
        .transpose()?;
    Ok(RelationshipRecord {
        resource_type: value.resource_type.clone(),
        resource_id: value.resource_id.clone(),
        relation: value.relation.clone(),
        subject_type: value.subject_type.clone(),
        subject_id: value.subject_id.clone(),
        subject_relation: value.subject_relation.clone(),
        stored_caveat,
    })
}

fn snapshot_inputs(
    registry: &RegistryGenerationV1,
    candidate: &CandidateWriteResult,
    now: i64,
    valid_for_seconds: u32,
) -> Result<(String, u64, i64), AccessGraphPublicationError> {
    let policy = registry
        .policies
        .iter()
        .find(|record| {
            record.claims.claims().record_id == candidate.policy_record_id
                && record.claims.claims().policy_hash == *candidate.policy_hash()
                && record.status == PolicyRecordStatus::Approved
        })
        .ok_or(AccessGraphPublicationError::CandidateRejected)?;
    let key = registry
        .authority_keys
        .iter()
        .find(|key| {
            let record = key.record();
            key.role() == AuthorityRole::Snapshot
                && record.status == AuthorityKeyStatus::Active
                && record.valid_from_unix_seconds <= now
                && now < record.valid_until_unix_seconds
        })
        .ok_or(AccessGraphPublicationError::Unavailable)?;
    let requested_expiry = now
        .checked_add(i64::from(valid_for_seconds))
        .ok_or(AccessGraphPublicationError::InvalidRequest)?;
    let expires_at = requested_expiry
        .min(policy.claims.claims().valid_until_unix_seconds)
        .min(key.record().valid_until_unix_seconds);
    if expires_at <= now {
        return Err(AccessGraphPublicationError::CandidateRejected);
    }
    let expected_previous_epoch = registry
        .accepted_roots
        .iter()
        .filter(|root| root.claims.claims().policy_hash == *candidate.policy_hash())
        .map(|root| root.claims.claims().epoch)
        .max()
        .unwrap_or(0);
    Ok((key.key_id().to_owned(), expected_previous_epoch, expires_at))
}

fn projection_for_revision(
    registry: &RegistryGenerationV1,
    candidate: &CandidateWriteResult,
    revision: &str,
) -> Option<AccessGraphProjectionWire> {
    registry
        .accepted_roots
        .iter()
        .find(|root| {
            let claims = root.claims.claims();
            claims.policy_record_id == candidate.policy_record_id
                && claims.policy_hash == *candidate.policy_hash()
                && claims.spicedb_revision == revision
                && root.status == RootRecordStatus::Accepted
        })
        .map(|root| projection_from_root(root, candidate.accepted_relationship_count))
}

fn projection_for_snapshot(
    registry: &RegistryGenerationV1,
    candidate: &CandidateWriteResult,
    snapshot_id: &str,
) -> Option<AccessGraphProjectionWire> {
    registry
        .accepted_roots
        .iter()
        .find(|root| {
            root.claims.claims().snapshot_id == snapshot_id
                && root.claims.claims().policy_record_id == candidate.policy_record_id
                && root.claims.claims().policy_hash == *candidate.policy_hash()
                && root.status == RootRecordStatus::Accepted
        })
        .map(|root| projection_from_root(root, candidate.accepted_relationship_count))
}

fn projection_from_root(
    root: &raziel_verifier::AcceptedRootRecordV1,
    accepted_relationship_count: u32,
) -> AccessGraphProjectionWire {
    let claims = root.claims.claims();
    AccessGraphProjectionWire {
        policy_record_id: claims.policy_record_id.clone(),
        policy_hash: lowercase_hex(&claims.policy_hash),
        snapshot_id: claims.snapshot_id.clone(),
        relationship_root: lowercase_hex(&claims.relationship_snapshot_root),
        epoch: claims.epoch,
        issued_at_unix_seconds: claims.issued_at_unix_seconds,
        expires_at_unix_seconds: claims.expires_at_unix_seconds,
        accepted_relationship_count,
        registry_generation: root.registry_generation,
    }
}

fn map_candidate_error(error: CandidateWriteError) -> AccessGraphPublicationError {
    match error {
        CandidateWriteError::Conflict => AccessGraphPublicationError::Conflict,
        CandidateWriteError::InvalidCandidateId | CandidateWriteError::Usage => {
            AccessGraphPublicationError::InvalidRequest
        }
        CandidateWriteError::CandidateRejected | CandidateWriteError::RegistryRejected => {
            AccessGraphPublicationError::CandidateRejected
        }
        _ => AccessGraphPublicationError::Unavailable,
    }
}

fn trusted_now() -> Result<i64, AccessGraphPublicationError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AccessGraphPublicationError::Unavailable)
        .and_then(|value| {
            i64::try_from(value.as_secs()).map_err(|_| AccessGraphPublicationError::Unavailable)
        })
}

fn bind_private_socket(path: &Path) -> Result<UnixListener, AccessGraphServerError> {
    let parent = path.parent().ok_or(AccessGraphServerError::Configuration)?;
    prepare_private_directory(parent)?;
    remove_safe_stale_socket(path, parent)?;
    let listener = UnixListener::bind(path).map_err(|_| AccessGraphServerError::Listener)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))
        .map_err(|_| AccessGraphServerError::Listener)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| AccessGraphServerError::Listener)?;
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| AccessGraphServerError::Listener)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(AccessGraphServerError::Listener);
    }
    Ok(listener)
}

fn prepare_private_directory(path: &Path) -> Result<(), AccessGraphServerError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => validate_private_directory(&metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(SOCKET_DIRECTORY_MODE);
            builder
                .create(path)
                .map_err(|_| AccessGraphServerError::Configuration)?;
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|_| AccessGraphServerError::Configuration)?;
            validate_private_directory(&metadata)
        }
        Err(_) => Err(AccessGraphServerError::Configuration),
    }
}

fn validate_private_directory(metadata: &Metadata) -> Result<(), AccessGraphServerError> {
    if metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == SOCKET_DIRECTORY_MODE
    {
        Ok(())
    } else {
        Err(AccessGraphServerError::Configuration)
    }
}

fn validate_private_state_file(metadata: &Metadata) -> Result<(), AccessGraphPublicationError> {
    if metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == STATE_FILE_MODE
        && metadata.len() <= MAX_STATE_BYTES
    {
        Ok(())
    } else {
        Err(AccessGraphPublicationError::Unavailable)
    }
}

fn remove_safe_stale_socket(path: &Path, parent: &Path) -> Result<(), AccessGraphServerError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(AccessGraphServerError::Configuration),
    };
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| AccessGraphServerError::Configuration)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != SOCKET_MODE
        || metadata.uid() != parent_metadata.uid()
    {
        return Err(AccessGraphServerError::Configuration);
    }
    std::fs::remove_file(path).map_err(|_| AccessGraphServerError::Configuration)
}

fn required_path(name: &str) -> Result<PathBuf, AccessGraphServerError> {
    let path =
        PathBuf::from(std::env::var(name).map_err(|_| AccessGraphServerError::Configuration)?);
    if valid_absolute_path(&path) {
        Ok(path)
    } else {
        Err(AccessGraphServerError::Configuration)
    }
}

fn required_endpoint(name: &str) -> Result<String, AccessGraphServerError> {
    let value = std::env::var(name).map_err(|_| AccessGraphServerError::Configuration)?;
    if (value.starts_with("http://") || value.starts_with("https://"))
        && value.len() <= 512
        && !value.as_bytes().contains(&0)
    {
        Ok(value)
    } else {
        Err(AccessGraphServerError::Configuration)
    }
}

fn required_identifier_env(name: &str) -> Result<String, AccessGraphServerError> {
    let value = std::env::var(name).map_err(|_| AccessGraphServerError::Configuration)?;
    if valid_identifier(&value) {
        Ok(value)
    } else {
        Err(AccessGraphServerError::Configuration)
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b'\\')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccessGraphPublicationError {
    InvalidRequest,
    Conflict,
    CandidateRejected,
    Unavailable,
}

#[derive(Debug)]
pub enum AccessGraphServerError {
    Configuration,
    Listener,
}

impl std::fmt::Display for AccessGraphServerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "access-graph configuration is invalid",
            Self::Listener => "access-graph local listener failed",
        })
    }
}

impl std::error::Error for AccessGraphServerError {}
