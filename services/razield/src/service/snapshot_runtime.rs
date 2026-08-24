use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use raziel_admin::writer::{RegistryWriterClient, RegistryWriterClientError};
use raziel_authority::{AuthorityKeyStatus, AuthorityRole, AuthoritySigner, SnapshotRole};
use raziel_authorization_core::{TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS};
use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_proto::raziel::v1 as proto;
use raziel_snapshot::{
    RELATIONSHIP_TREE_VERSION, SnapshotDescriptorClaimsV1, SnapshotDescriptorLimits, SnapshotTree,
    ValidatedSnapshotDescriptorClaimsV1,
};
use raziel_verifier::{
    RegistryGenerationV1, RegistryLoader, RootRecordStatus, default_registry_config,
    root_record_to_proto,
};
use tokio::sync::Mutex;

use super::{SnapshotOperations, SnapshotServiceConfig, SystemClock, TrustedClock};
use crate::private_snapshot::{
    PrivateRelationshipSnapshot, PrivateSnapshotError, PrivateSnapshotStore,
};
use crate::spicedb::SpiceDbSnapshotReader;

const MAX_SECRET_BYTES: u64 = 4_096;

/// Production composition for trusted snapshot lifecycle operations.
pub struct SnapshotCoordinator {
    config: SnapshotServiceConfig,
    private_store: PrivateSnapshotStore,
    writer: RegistryWriterClient,
    mutation_lock: Mutex<()>,
}

impl SnapshotCoordinator {
    pub fn open(config: SnapshotServiceConfig) -> Result<Self, SnapshotOperationError> {
        let private_store = PrivateSnapshotStore::open(config.private_snapshot_root())
            .map_err(|_| SnapshotOperationError::PrivateStore)?;
        let writer = RegistryWriterClient::new(config.registry_writer_socket_path().to_owned())
            .map_err(|_| SnapshotOperationError::WriterUnavailable)?;
        Ok(Self {
            config,
            private_store,
            writer,
            mutation_lock: Mutex::new(()),
        })
    }

    async fn build_snapshot(
        &self,
        request: proto::BuildSnapshotRequest,
    ) -> Result<proto::BuildSnapshotResponse, SnapshotOperationError> {
        let _guard = self.mutation_lock.lock().await;
        let now = SystemClock
            .now_unix_seconds()
            .map_err(|_| SnapshotOperationError::Clock)?;
        let request = ValidatedBuildRequest::new(&self.config, request, now)?;
        let current = load_registry(&self.config, now).await?;
        let mut registry = raziel_verifier::LocalRegistry::new(
            default_registry_config(self.config.trust_domain_id())
                .map_err(|_| SnapshotOperationError::RegistryRead)?,
        );
        registry
            .install_generation(current.clone())
            .map_err(|_| SnapshotOperationError::RegistryRead)?;
        let policy_record = registry
            .resolve_policy_exact(&request.policy_record_id, &request.policy_hash, now)
            .map_err(|_| SnapshotOperationError::PolicyResolution)?;
        let policy = ValidatedPolicyProgram::new(
            policy_record.policy.program().clone(),
            &TRUSTED_POLICY_CEILINGS,
        )
        .map_err(|_| SnapshotOperationError::PolicyGuestLimit)?;
        let policy_claims = policy_record.claims.claims();
        if request.expires_at_unix_seconds > policy_claims.valid_until_unix_seconds {
            return Err(SnapshotOperationError::InvalidValidityWindow);
        }

        let signer = load_snapshot_signer(self.config.snapshot_key_path()).await?;
        if signer.key_id() != request.authority_key_id {
            return Err(SnapshotOperationError::AuthorityKey);
        }
        let authority_key = current
            .authority_keys
            .iter()
            .find(|key| key.key_id() == request.authority_key_id)
            .ok_or(SnapshotOperationError::AuthorityKey)?;
        let key_record = authority_key.record();
        if authority_key.role() != AuthorityRole::Snapshot
            || key_record.status != AuthorityKeyStatus::Active
            || now < key_record.valid_from_unix_seconds
            || now >= key_record.valid_until_unix_seconds
            || request.expires_at_unix_seconds > key_record.valid_until_unix_seconds
            || signer.public_key_bytes() != key_record.public_key
        {
            return Err(SnapshotOperationError::AuthorityKey);
        }

        let high_watermark = current
            .accepted_roots
            .iter()
            .filter(|root| root.claims.claims().policy_hash == request.policy_hash)
            .map(|root| root.claims.claims().epoch)
            .max()
            .unwrap_or(0);
        if request.expected_previous_epoch != high_watermark {
            return Err(SnapshotOperationError::StaleEpoch);
        }
        let epoch = high_watermark
            .checked_add(1)
            .ok_or(SnapshotOperationError::StaleEpoch)?;

        let auth_token = read_secret(self.config.spicedb_auth_token_path()).await?;
        let zed_token = read_secret(self.config.zed_token_path()).await?;
        let mut reader =
            SpiceDbSnapshotReader::connect(self.config.spicedb_endpoint(), &auth_token)
                .await
                .map_err(|_| SnapshotOperationError::SpiceDb)?;
        drop(auth_token);
        let relationships = reader
            .read_exact(&policy, &zed_token)
            .await
            .map_err(|_| SnapshotOperationError::SpiceDb)?;

        let mut snapshot_salt = [0_u8; 32];
        getrandom::fill(&mut snapshot_salt).map_err(|_| SnapshotOperationError::Entropy)?;
        let tree = SnapshotTree::build(
            &policy,
            relationships.clone(),
            &snapshot_salt,
            TRUSTED_SNAPSHOT_LIMITS,
        )
        .map_err(|_| SnapshotOperationError::SnapshotBuild)?;
        let relationship_root = tree.root();
        let snapshot_id = format!("snapshot-v1:{}", lowercase_hex(&relationship_root));
        let claims = ValidatedSnapshotDescriptorClaimsV1::new(
            SnapshotDescriptorClaimsV1 {
                format_version: raziel_snapshot::SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
                trust_domain_id: self.config.trust_domain_id().into(),
                snapshot_id: snapshot_id.clone(),
                relationship_snapshot_root: relationship_root,
                policy_record_id: request.policy_record_id,
                policy_hash: request.policy_hash,
                spicedb_revision: zed_token,
                epoch,
                issued_at_unix_seconds: now,
                expires_at_unix_seconds: request.expires_at_unix_seconds,
                canonicalization_version: RELATIONSHIP_TREE_VERSION,
                signing_key_id: signer.key_id().into(),
            },
            SnapshotDescriptorLimits {
                max_string_bytes: raziel_verifier::DEFAULT_MAX_REGISTRY_STRING_BYTES,
                max_lifetime_seconds: raziel_verifier::DEFAULT_MAX_ARTIFACT_LIFETIME_SECONDS,
            },
        )
        .map_err(|_| SnapshotOperationError::SnapshotBuild)?;
        let authority_signature = signer
            .sign_snapshot_descriptor(&claims)
            .map_err(|_| SnapshotOperationError::Signing)?;
        let descriptor = descriptor_to_proto(&claims, authority_signature);
        let private_snapshot = PrivateRelationshipSnapshot::new(
            &policy,
            snapshot_id,
            relationship_root,
            snapshot_salt,
            relationships,
        )
        .map_err(|_| SnapshotOperationError::SnapshotBuild)?;

        let store = self.private_store.clone();
        tokio::task::spawn_blocking(move || store.publish(&private_snapshot))
            .await
            .map_err(|_| SnapshotOperationError::BlockingTask)?
            .map_err(map_private_store_error)?;

        let writer = self.writer.clone();
        let descriptor_for_writer = descriptor.clone();
        let registry_generation = current.generation;
        let written = tokio::task::spawn_blocking(move || {
            writer.append_snapshot(registry_generation, descriptor_for_writer)
        })
        .await
        .map_err(|_| SnapshotOperationError::BlockingTask)?
        .map_err(map_writer_error)?;
        let confirmed = load_registry(&self.config, now).await?;
        let confirmed_record = confirmed
            .accepted_roots
            .iter()
            .find(|root| root.claims.claims().snapshot_id == descriptor.snapshot_id)
            .map(root_record_to_proto)
            .ok_or(SnapshotOperationError::RegistryConfirmation)?;
        if confirmed.generation != written.registry_generation
            || confirmed_record != written.root_record
            || confirmed_record.descriptor.as_ref() != Some(&descriptor)
        {
            return Err(SnapshotOperationError::RegistryConfirmation);
        }
        Ok(proto::BuildSnapshotResponse {
            snapshot: Some(descriptor),
        })
    }

    async fn get_snapshot(
        &self,
        request: proto::GetSnapshotRequest,
    ) -> Result<proto::GetSnapshotResponse, SnapshotOperationError> {
        validate_snapshot_id(&request.snapshot_id)?;
        let now = SystemClock
            .now_unix_seconds()
            .map_err(|_| SnapshotOperationError::Clock)?;
        let current = load_registry(&self.config, now).await?;
        let root = current
            .accepted_roots
            .iter()
            .find(|root| root.claims.claims().snapshot_id == request.snapshot_id)
            .ok_or(SnapshotOperationError::UnknownSnapshot)?;
        let status = public_status(root, now);
        Ok(proto::GetSnapshotResponse {
            snapshot: root_record_to_proto(root).descriptor,
            status: status as i32,
        })
    }

    async fn revoke_snapshot(
        &self,
        request: proto::RevokeSnapshotRequest,
    ) -> Result<proto::RevokeSnapshotResponse, SnapshotOperationError> {
        let _guard = self.mutation_lock.lock().await;
        validate_snapshot_id(&request.snapshot_id)?;
        validate_reason(&request.reason)?;
        let now = SystemClock
            .now_unix_seconds()
            .map_err(|_| SnapshotOperationError::Clock)?;
        let current = load_registry(&self.config, now).await?;
        let root = current
            .accepted_roots
            .iter()
            .find(|root| root.claims.claims().snapshot_id == request.snapshot_id)
            .ok_or(SnapshotOperationError::UnknownSnapshot)?;
        if root.status != RootRecordStatus::Accepted {
            return Err(SnapshotOperationError::AlreadyRevoked);
        }
        let writer = self.writer.clone();
        let snapshot_id = request.snapshot_id;
        let reason = request.reason;
        let written = tokio::task::spawn_blocking(move || {
            writer.revoke_snapshot(current.generation, snapshot_id, reason)
        })
        .await
        .map_err(|_| SnapshotOperationError::BlockingTask)?
        .map_err(map_writer_error)?;
        let confirmed = load_registry(&self.config, now).await?;
        let confirmed_record = confirmed
            .accepted_roots
            .iter()
            .find(|root| {
                root.claims.claims().snapshot_id
                    == written
                        .root_record
                        .descriptor
                        .as_ref()
                        .map_or("", |descriptor| descriptor.snapshot_id.as_str())
            })
            .map(root_record_to_proto)
            .ok_or(SnapshotOperationError::RegistryConfirmation)?;
        if confirmed.generation != written.registry_generation
            || confirmed_record != written.root_record
            || confirmed_record.status != proto::SnapshotStatus::Revoked as i32
        {
            return Err(SnapshotOperationError::RegistryConfirmation);
        }
        let descriptor = confirmed_record
            .descriptor
            .ok_or(SnapshotOperationError::RegistryConfirmation)?;
        Ok(proto::RevokeSnapshotResponse {
            snapshot_id: descriptor.snapshot_id,
            status: proto::SnapshotStatus::Revoked as i32,
            revoked_at: confirmed_record.revoked_at,
        })
    }
}

#[tonic::async_trait]
impl SnapshotOperations for SnapshotCoordinator {
    async fn build(
        &self,
        request: proto::BuildSnapshotRequest,
    ) -> Result<proto::BuildSnapshotResponse, SnapshotOperationError> {
        self.build_snapshot(request).await
    }

    async fn get(
        &self,
        request: proto::GetSnapshotRequest,
    ) -> Result<proto::GetSnapshotResponse, SnapshotOperationError> {
        self.get_snapshot(request).await
    }

    async fn revoke(
        &self,
        request: proto::RevokeSnapshotRequest,
    ) -> Result<proto::RevokeSnapshotResponse, SnapshotOperationError> {
        self.revoke_snapshot(request).await
    }
}

struct ValidatedBuildRequest {
    policy_record_id: String,
    policy_hash: [u8; 32],
    expires_at_unix_seconds: i64,
    expected_previous_epoch: u64,
    authority_key_id: String,
}

impl ValidatedBuildRequest {
    fn new(
        config: &SnapshotServiceConfig,
        request: proto::BuildSnapshotRequest,
        now: i64,
    ) -> Result<Self, SnapshotOperationError> {
        if request.spicedb_endpoint_ref != config.spicedb_endpoint_ref()
            || request.zed_token_secret_ref != config.zed_token_secret_ref()
        {
            return Err(SnapshotOperationError::ConfigurationReferenceMismatch);
        }
        if !valid_identifier(&request.policy_record_id)
            || !valid_identifier(&request.authority_key_id)
        {
            return Err(SnapshotOperationError::InvalidRequest);
        }
        let policy_hash = request
            .policy_hash
            .try_into()
            .map_err(|_| SnapshotOperationError::InvalidRequest)?;
        let expires_at = request
            .expires_at
            .ok_or(SnapshotOperationError::InvalidRequest)?;
        if expires_at.nanos != 0 || expires_at.seconds <= now {
            return Err(SnapshotOperationError::InvalidRequest);
        }
        Ok(Self {
            policy_record_id: request.policy_record_id,
            policy_hash,
            expires_at_unix_seconds: expires_at.seconds,
            expected_previous_epoch: request.expected_previous_epoch,
            authority_key_id: request.authority_key_id,
        })
    }
}

async fn load_registry(
    config: &SnapshotServiceConfig,
    now: i64,
) -> Result<RegistryGenerationV1, SnapshotOperationError> {
    let root = config.registry_path().to_owned();
    let trust_domain_id = config.trust_domain_id().to_owned();
    tokio::task::spawn_blocking(move || {
        let config = default_registry_config(&trust_domain_id)
            .map_err(|_| SnapshotOperationError::RegistryRead)?;
        RegistryLoader::new(config)
            .load_current(&root, now)
            .map_err(|_| SnapshotOperationError::RegistryRead)
    })
    .await
    .map_err(|_| SnapshotOperationError::BlockingTask)?
}

async fn load_snapshot_signer(
    path: &Path,
) -> Result<AuthoritySigner<SnapshotRole>, SnapshotOperationError> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        AuthoritySigner::<SnapshotRole>::from_read_only_pkcs8_file(&path)
            .map_err(|_| SnapshotOperationError::AuthorityKey)
    })
    .await
    .map_err(|_| SnapshotOperationError::BlockingTask)?
}

async fn read_secret(path: &Path) -> Result<String, SnapshotOperationError> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || read_secret_file(&path))
        .await
        .map_err(|_| SnapshotOperationError::BlockingTask)?
}

fn read_secret_file(path: &Path) -> Result<String, SnapshotOperationError> {
    let path_metadata =
        std::fs::symlink_metadata(path).map_err(|_| SnapshotOperationError::SecretRead)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || path_metadata.permissions().mode() & 0o077 != 0
        || path_metadata.len() == 0
        || path_metadata.len() > MAX_SECRET_BYTES
    {
        return Err(SnapshotOperationError::SecretRead);
    }
    let file = File::open(path).map_err(|_| SnapshotOperationError::SecretRead)?;
    let opened = file
        .metadata()
        .map_err(|_| SnapshotOperationError::SecretRead)?;
    if opened.dev() != path_metadata.dev()
        || opened.ino() != path_metadata.ino()
        || opened.len() != path_metadata.len()
    {
        return Err(SnapshotOperationError::SecretRead);
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(MAX_SECRET_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SnapshotOperationError::SecretRead)?;
    if bytes.len() as u64 != opened.len()
        || !bytes.is_ascii()
        || bytes.iter().any(u8::is_ascii_control)
    {
        bytes.fill(0);
        return Err(SnapshotOperationError::SecretRead);
    }
    let value = String::from_utf8(bytes).map_err(|_| SnapshotOperationError::SecretRead)?;
    if value.is_empty() {
        return Err(SnapshotOperationError::SecretRead);
    }
    Ok(value)
}

fn descriptor_to_proto(
    claims: &ValidatedSnapshotDescriptorClaimsV1,
    authority_signature: [u8; 64],
) -> proto::RelationshipSnapshotDescriptor {
    let claims = claims.claims();
    proto::RelationshipSnapshotDescriptor {
        format_version: claims.format_version,
        snapshot_id: claims.snapshot_id.clone(),
        relationship_snapshot_root: claims.relationship_snapshot_root.to_vec(),
        policy_hash: claims.policy_hash.to_vec(),
        spicedb_revision: claims.spicedb_revision.clone(),
        epoch: claims.epoch,
        issued_at: Some(timestamp(claims.issued_at_unix_seconds)),
        expires_at: Some(timestamp(claims.expires_at_unix_seconds)),
        canonicalization_version: claims.canonicalization_version,
        signing_key_id: claims.signing_key_id.clone(),
        authority_signature: authority_signature.to_vec(),
        trust_domain_id: claims.trust_domain_id.clone(),
        policy_record_id: claims.policy_record_id.clone(),
    }
}

fn public_status(root: &raziel_verifier::AcceptedRootRecordV1, now: i64) -> proto::SnapshotStatus {
    if root.status == RootRecordStatus::Revoked {
        proto::SnapshotStatus::Revoked
    } else if now >= root.claims.claims().expires_at_unix_seconds {
        proto::SnapshotStatus::Expired
    } else {
        proto::SnapshotStatus::Accepted
    }
}

fn map_private_store_error(_error: PrivateSnapshotError) -> SnapshotOperationError {
    SnapshotOperationError::PrivateStore
}

fn map_writer_error(error: RegistryWriterClientError) -> SnapshotOperationError {
    match error {
        RegistryWriterClientError::Unavailable => SnapshotOperationError::WriterUnavailable,
        RegistryWriterClientError::StaleGeneration => SnapshotOperationError::WriterStaleGeneration,
        RegistryWriterClientError::Conflict => SnapshotOperationError::WriterConflict,
        RegistryWriterClientError::InvalidConfiguration
        | RegistryWriterClientError::InvalidRequest
        | RegistryWriterClientError::Protocol
        | RegistryWriterClientError::RegistryRejected
        | RegistryWriterClientError::PublicationFailed => SnapshotOperationError::WriterRejected,
    }
}

fn validate_snapshot_id(value: &str) -> Result<(), SnapshotOperationError> {
    let digest = value
        .strip_prefix("snapshot-v1:")
        .ok_or(SnapshotOperationError::InvalidRequest)?;
    if digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(SnapshotOperationError::InvalidRequest)
    }
}

fn validate_reason(value: &str) -> Result<(), SnapshotOperationError> {
    if value.len() <= 512 && value.is_ascii() && !value.bytes().any(|byte| byte.is_ascii_control())
    {
        Ok(())
    } else {
        Err(SnapshotOperationError::InvalidRequest)
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

const fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotOperationError {
    InvalidRequest,
    ConfigurationReferenceMismatch,
    Clock,
    RegistryRead,
    PolicyResolution,
    PolicyGuestLimit,
    AuthorityKey,
    InvalidValidityWindow,
    StaleEpoch,
    SecretRead,
    SpiceDb,
    Entropy,
    SnapshotBuild,
    Signing,
    PrivateStore,
    WriterUnavailable,
    WriterStaleGeneration,
    WriterConflict,
    WriterRejected,
    RegistryConfirmation,
    UnknownSnapshot,
    AlreadyRevoked,
    BlockingTask,
}

impl std::fmt::Display for SnapshotOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("snapshot operation failed")
    }
}

impl std::error::Error for SnapshotOperationError {}
