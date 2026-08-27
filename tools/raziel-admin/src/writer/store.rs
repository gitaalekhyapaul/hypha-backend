use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_proto::raziel::v1 as proto;
use raziel_verifier::{
    AcceptedRootRecordV1, LocalRegistry, RegistryConfig, RegistryGenerationV1, RegistryLoader,
    RootRecordStatus, authority_key_from_proto, authority_key_to_proto, policy_record_from_proto,
    policy_record_to_proto, root_record_from_proto, root_record_to_proto,
};

use crate::config::registry_config;
use crate::io::atomic_write;
use crate::registry::{create_generation_directory, write_generation_record};

use super::wire::{RegistryWriterRequestWire, registry_writer_request_wire};
use super::{MAX_REVOCATION_REASON_BYTES, PROTOCOL_VERSION};

const REGISTRY_MANIFEST_FORMAT_VERSION: u32 = 1;
static GENERATION_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct RegistryMutationStore {
    root: std::path::PathBuf,
    config: RegistryConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StoreMutationError {
    StaleGeneration,
    Conflict,
    InvalidRequest,
    RegistryRejected,
    PublicationFailed,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum StoreMutationResult {
    Root(proto::AcceptedRootRecord),
    Policy(proto::PolicyRecord),
}

enum ChangedRecord {
    Root(String),
    Policy(String),
}

impl RegistryMutationStore {
    pub(super) fn new(
        root: std::path::PathBuf,
        trust_domain_id: &str,
    ) -> Result<Self, StoreMutationError> {
        if !root.is_absolute() {
            return Err(StoreMutationError::InvalidRequest);
        }
        let config =
            registry_config(trust_domain_id).map_err(|_| StoreMutationError::InvalidRequest)?;
        Ok(Self { root, config })
    }

    pub(super) fn apply(
        &self,
        request: RegistryWriterRequestWire,
        trusted_now_unix_seconds: i64,
    ) -> Result<StoreMutationResult, StoreMutationError> {
        validate_request(&request)?;
        let loader = RegistryLoader::new(self.config.clone());
        let current = loader
            .load_current(&self.root, trusted_now_unix_seconds)
            .map_err(|_| StoreMutationError::RegistryRejected)?;
        if request.expected_registry_generation != current.generation {
            return Err(StoreMutationError::StaleGeneration);
        }
        let next_generation = current
            .generation
            .checked_add(1)
            .ok_or(StoreMutationError::RegistryRejected)?;

        let mut authority_keys = current.authority_keys.clone();
        let mut policies = current.policies.clone();
        let mut roots = current.accepted_roots.clone();
        let changed = match request
            .operation
            .ok_or(StoreMutationError::InvalidRequest)?
        {
            registry_writer_request_wire::Operation::AppendSnapshot(descriptor) => append_root(
                &mut roots,
                *descriptor,
                next_generation,
                trusted_now_unix_seconds,
                &self.config,
            )
            .map(ChangedRecord::Root)?,
            registry_writer_request_wire::Operation::RevokeSnapshot(revocation) => revoke_root(
                &mut roots,
                &revocation.snapshot_id,
                &revocation.reason,
                next_generation,
                trusted_now_unix_seconds,
            )
            .map(ChangedRecord::Root)?,
            registry_writer_request_wire::Operation::AppendApprovedPolicy(staged) => append_policy(
                &mut policies,
                &authority_keys,
                *staged,
                next_generation,
                trusted_now_unix_seconds,
                &self.config,
            )
            .map(ChangedRecord::Policy)?,
        };

        rewrite_generation_numbers(
            &mut authority_keys,
            &mut policies,
            &mut roots,
            next_generation,
            &self.config,
            trusted_now_unix_seconds,
        )?;
        validate_transition(
            &self.config,
            &current,
            RegistryGenerationV1 {
                generation: next_generation,
                publication_digest: [0_u8; 32],
                authority_keys: authority_keys.clone(),
                policies: policies.clone(),
                accepted_roots: roots.clone(),
            },
        )?;
        let manifest = self.publish_generation(
            next_generation,
            trusted_now_unix_seconds,
            &authority_keys,
            &policies,
            &roots,
        )?;
        let manifest_bytes = manifest.encode_to_vec();

        let candidate = loader
            .load_candidate(&self.root, &manifest_bytes, trusted_now_unix_seconds)
            .map_err(|_| StoreMutationError::RegistryRejected)?;
        validate_transition(&self.config, &current, candidate)?;
        atomic_write(&self.root.join("current.pb"), &manifest_bytes)
            .map_err(|_| StoreMutationError::PublicationFailed)?;

        match changed {
            ChangedRecord::Root(snapshot_id) => roots
                .into_iter()
                .find(|root| root.claims.claims().snapshot_id == snapshot_id)
                .map(|root| StoreMutationResult::Root(root_record_to_proto(&root)))
                .ok_or(StoreMutationError::PublicationFailed),
            ChangedRecord::Policy(record_id) => policies
                .into_iter()
                .find(|policy| policy.claims.claims().record_id == record_id)
                .map(|policy| {
                    policy_record_to_proto(&policy)
                        .map(StoreMutationResult::Policy)
                        .map_err(|_| StoreMutationError::PublicationFailed)
                })
                .transpose()?
                .ok_or(StoreMutationError::PublicationFailed),
        }
    }

    fn publish_generation(
        &self,
        generation: u64,
        published_at: i64,
        authority_keys: &[raziel_authority::TrustedAuthorityKey],
        policies: &[raziel_verifier::PolicyRegistryRecordV1],
        roots: &[AcceptedRootRecordV1],
    ) -> Result<proto::RegistryGenerationManifest, StoreMutationError> {
        let sequence = GENERATION_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let output_directory = format!(
            "generations/{generation:020}-{}-{sequence}",
            std::process::id()
        );
        create_generation_directory(&self.root, &output_directory)
            .map_err(|_| StoreMutationError::PublicationFailed)?;

        let mut authority_records = authority_keys
            .iter()
            .map(authority_key_to_proto)
            .collect::<Vec<_>>();
        authority_records
            .sort_by(|left, right| left.key_id.as_bytes().cmp(right.key_id.as_bytes()));
        let authority_keys = write_records(
            &self.root,
            &output_directory,
            "authority",
            authority_records.iter().map(Message::encode_to_vec),
        )?;

        let mut policy_records = policies
            .iter()
            .map(policy_record_to_proto)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StoreMutationError::RegistryRejected)?;
        policy_records
            .sort_by(|left, right| left.record_id.as_bytes().cmp(right.record_id.as_bytes()));
        let policies = write_records(
            &self.root,
            &output_directory,
            "policy",
            policy_records.iter().map(Message::encode_to_vec),
        )?;

        let mut root_records = roots.iter().map(root_record_to_proto).collect::<Vec<_>>();
        root_records.sort_by(|left, right| {
            let left = left
                .descriptor
                .as_ref()
                .map(|value| value.snapshot_id.as_bytes());
            let right = right
                .descriptor
                .as_ref()
                .map(|value| value.snapshot_id.as_bytes());
            left.cmp(&right)
        });
        let accepted_roots = write_records(
            &self.root,
            &output_directory,
            "root",
            root_records.iter().map(Message::encode_to_vec),
        )?;

        Ok(proto::RegistryGenerationManifest {
            format_version: REGISTRY_MANIFEST_FORMAT_VERSION,
            trust_domain_id: self.config.trust_domain_id.clone(),
            generation,
            authority_keys,
            policies,
            accepted_roots,
            published_at: Some(timestamp(published_at)),
        })
    }
}

fn append_policy(
    policies: &mut Vec<raziel_verifier::PolicyRegistryRecordV1>,
    authority_keys: &[raziel_authority::TrustedAuthorityKey],
    mut staged: proto::PolicyRecord,
    generation: u64,
    registered_at: i64,
    config: &RegistryConfig,
) -> Result<String, StoreMutationError> {
    if staged.status != proto::PolicyStatus::Unspecified as i32
        || staged.registered_at.is_some()
        || staged.revoked_at.is_some()
        || !staged.revocation_reason.is_empty()
        || staged.registry_generation != 0
    {
        return Err(StoreMutationError::InvalidRequest);
    }
    staged.status = proto::PolicyStatus::Approved as i32;
    staged.registered_at = Some(timestamp(registered_at));
    staged.registry_generation = generation;
    let policy = policy_record_from_proto(staged, config)
        .map_err(|_| StoreMutationError::RegistryRejected)?;
    let claims = policy.claims.claims();
    if policies
        .iter()
        .any(|current| current.claims.claims().record_id == claims.record_id)
    {
        return Err(StoreMutationError::Conflict);
    }
    let expected_epoch = policies
        .iter()
        .filter(|current| current.claims.claims().policy_name == claims.policy_name)
        .map(|current| current.claims.claims().approval_epoch)
        .max()
        .map_or(Some(1), |epoch| epoch.checked_add(1))
        .ok_or(StoreMutationError::RegistryRejected)?;
    if claims.approval_epoch != expected_epoch {
        return Err(StoreMutationError::Conflict);
    }
    if registered_at < claims.valid_from_unix_seconds
        || registered_at >= claims.valid_until_unix_seconds
    {
        return Err(StoreMutationError::RegistryRejected);
    }
    let key = authority_keys
        .iter()
        .find(|key| key.key_id() == claims.signing_key_id)
        .ok_or(StoreMutationError::RegistryRejected)?;
    key.verify_policy_approval(registered_at, &policy.claims, &policy.authority_signature)
        .map_err(|_| StoreMutationError::RegistryRejected)?;
    let record_id = claims.record_id.clone();
    policies.push(policy);
    Ok(record_id)
}

fn append_root(
    roots: &mut Vec<AcceptedRootRecordV1>,
    descriptor: proto::RelationshipSnapshotDescriptor,
    generation: u64,
    registered_at: i64,
    config: &RegistryConfig,
) -> Result<String, StoreMutationError> {
    let snapshot_id = descriptor.snapshot_id.clone();
    let expected_snapshot_id = descriptor
        .relationship_snapshot_root
        .as_slice()
        .try_into()
        .map(|root: [u8; 32]| format!("snapshot-v1:{}", lowercase_hex(&root)))
        .map_err(|_| StoreMutationError::InvalidRequest)?;
    if snapshot_id != expected_snapshot_id {
        return Err(StoreMutationError::InvalidRequest);
    }
    let wire = proto::AcceptedRootRecord {
        format_version: 1,
        descriptor: Some(descriptor),
        status: proto::SnapshotStatus::Accepted as i32,
        registered_at: Some(timestamp(registered_at)),
        revoked_at: None,
        revocation_reason: String::new(),
        registry_generation: generation,
    };
    let root = root_record_from_proto(wire, config, registered_at)
        .map_err(|_| StoreMutationError::InvalidRequest)?;
    if roots.iter().any(|current| {
        current.claims.claims().snapshot_id == snapshot_id
            || current.claims.claims().relationship_snapshot_root
                == root.claims.claims().relationship_snapshot_root
    }) {
        return Err(StoreMutationError::Conflict);
    }
    roots.push(root);
    Ok(snapshot_id)
}

fn revoke_root(
    roots: &mut [AcceptedRootRecordV1],
    snapshot_id: &str,
    reason: &str,
    generation: u64,
    revoked_at: i64,
) -> Result<String, StoreMutationError> {
    if snapshot_id.is_empty()
        || reason.len() > MAX_REVOCATION_REASON_BYTES
        || !reason.is_ascii()
        || reason.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(StoreMutationError::InvalidRequest);
    }
    let root = roots
        .iter_mut()
        .find(|root| root.claims.claims().snapshot_id == snapshot_id)
        .ok_or(StoreMutationError::Conflict)?;
    if root.status != RootRecordStatus::Accepted {
        return Err(StoreMutationError::Conflict);
    }
    root.status = RootRecordStatus::Revoked;
    root.revoked_at_unix_seconds = Some(revoked_at);
    root.revocation_reason = (!reason.is_empty()).then(|| reason.to_owned());
    root.registry_generation = generation;
    Ok(snapshot_id.to_owned())
}

fn rewrite_generation_numbers(
    authority_keys: &mut [raziel_authority::TrustedAuthorityKey],
    policies: &mut [raziel_verifier::PolicyRegistryRecordV1],
    roots: &mut [AcceptedRootRecordV1],
    generation: u64,
    config: &RegistryConfig,
    observed_now: i64,
) -> Result<(), StoreMutationError> {
    for key in authority_keys {
        let mut wire = authority_key_to_proto(key);
        wire.registry_generation = generation;
        *key = authority_key_from_proto(wire, config)
            .map_err(|_| StoreMutationError::RegistryRejected)?;
    }
    for policy in policies {
        let mut wire =
            policy_record_to_proto(policy).map_err(|_| StoreMutationError::RegistryRejected)?;
        wire.registry_generation = generation;
        *policy = policy_record_from_proto(wire, config)
            .map_err(|_| StoreMutationError::RegistryRejected)?;
    }
    for root in roots {
        let mut wire = root_record_to_proto(root);
        wire.registry_generation = generation;
        *root = root_record_from_proto(wire, config, observed_now)
            .map_err(|_| StoreMutationError::RegistryRejected)?;
    }
    Ok(())
}

fn validate_transition(
    config: &RegistryConfig,
    current: &RegistryGenerationV1,
    candidate: RegistryGenerationV1,
) -> Result<(), StoreMutationError> {
    let mut registry = LocalRegistry::new(config.clone());
    registry
        .install_generation(current.clone())
        .map_err(|_| StoreMutationError::RegistryRejected)?;
    registry
        .install_generation(candidate)
        .map_err(|_| StoreMutationError::RegistryRejected)?;
    Ok(())
}

fn write_records(
    root: &std::path::Path,
    output_directory: &str,
    kind: &str,
    records: impl IntoIterator<Item = Vec<u8>>,
) -> Result<Vec<proto::RegistryFile>, StoreMutationError> {
    records
        .into_iter()
        .enumerate()
        .map(|(index, bytes)| {
            write_generation_record(root, output_directory, kind, index, &bytes)
                .map_err(|_| StoreMutationError::PublicationFailed)
        })
        .collect()
}

fn validate_request(request: &RegistryWriterRequestWire) -> Result<(), StoreMutationError> {
    if request.format_version != PROTOCOL_VERSION
        || request.expected_registry_generation == 0
        || request.operation.is_none()
    {
        return Err(StoreMutationError::InvalidRequest);
    }
    Ok(())
}

pub(super) fn trusted_now() -> Result<i64, StoreMutationError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreMutationError::PublicationFailed)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| StoreMutationError::PublicationFailed)
}

const fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
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
