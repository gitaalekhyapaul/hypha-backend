use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_authority::{AuthorityRole, AuthoritySigner, PolicyApprovalRole};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::{ValidatedPolicyProgram, decode_policy_program_strict};
use raziel_proto::raziel::v1 as proto;
use raziel_verifier::{LocalRegistry, RegistryConfig, RegistryLoader};
use sha2::{Digest, Sha256};

use crate::config::{
    MAX_ARTIFACT_LIFETIME_SECONDS, MAX_RECORD_BYTES, MAX_STRING_BYTES, registry_config,
    validation_ceilings,
};
use crate::io::atomic_write_new;

use super::wire::{
    PolicyAuthorityRequestWire, PolicyCandidateManifestWire, StoredPolicyApprovalWire,
};
use super::{PROTOCOL_VERSION, valid_absolute_path};

const CANDIDATE_MANIFEST_FORMAT_VERSION: u32 = 1;
const STORED_APPROVAL_FORMAT_VERSION: u32 = 1;
const POLICY_RECORD_FORMAT_VERSION: u32 = 1;
const MAX_CANDIDATE_LIFETIME_SECONDS: u64 = 24 * 60 * 60;

pub(super) struct PolicyApprovalStore {
    candidate_root: PathBuf,
    registry_root: PathBuf,
    config: RegistryConfig,
    signer: AuthoritySigner<PolicyApprovalRole>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StoreApprovalError {
    StaleEpoch,
    Conflict,
    InvalidRequest,
    CandidateRejected,
    Unavailable,
}

impl PolicyApprovalStore {
    pub(super) fn new(
        candidate_root: PathBuf,
        registry_root: PathBuf,
        trust_domain_id: &str,
        signer: AuthoritySigner<PolicyApprovalRole>,
    ) -> Result<Self, StoreApprovalError> {
        if !valid_absolute_path(&candidate_root) || !valid_absolute_path(&registry_root) {
            return Err(StoreApprovalError::InvalidRequest);
        }
        let config =
            registry_config(trust_domain_id).map_err(|_| StoreApprovalError::InvalidRequest)?;
        Ok(Self {
            candidate_root,
            registry_root,
            config,
            signer,
        })
    }

    pub(super) fn apply(
        &self,
        request: PolicyAuthorityRequestWire,
        trusted_now_unix_seconds: i64,
    ) -> Result<proto::PolicyRecord, StoreApprovalError> {
        validate_request(&request)?;
        let candidate_dir = self.candidate_root.join(&request.candidate_id);
        validate_private_directory(&self.candidate_root, None)?;
        let owner = validate_private_directory(&candidate_dir, Some(&self.candidate_root))?;

        let manifest_bytes =
            read_private_file(&candidate_dir.join("candidate.pb"), owner, MAX_RECORD_BYTES)?;
        let manifest: PolicyCandidateManifestWire = decode_canonical(&manifest_bytes)?;
        validate_manifest(&manifest)?;
        if manifest.candidate_id != request.candidate_id {
            return Err(StoreApprovalError::CandidateRejected);
        }
        let expected_hash =
            fixed_hash(&request.expected_policy_hash).ok_or(StoreApprovalError::InvalidRequest)?;
        let manifest_hash =
            fixed_hash(&manifest.policy_hash).ok_or(StoreApprovalError::CandidateRejected)?;
        if expected_hash != manifest_hash {
            return Err(StoreApprovalError::CandidateRejected);
        }
        if request.expected_approval_epoch != manifest.expected_approval_epoch {
            return Err(StoreApprovalError::StaleEpoch);
        }
        if request.valid_from_unix_seconds != manifest.valid_from_unix_seconds
            || request.valid_until_unix_seconds != manifest.valid_until_unix_seconds
        {
            return Err(StoreApprovalError::CandidateRejected);
        }

        let policy_bytes =
            read_private_file(&candidate_dir.join("policy.pb"), owner, MAX_RECORD_BYTES)?;
        let policy =
            decode_policy_program_strict(&policy_bytes, MAX_RECORD_BYTES, &validation_ceilings())
                .map_err(|_| StoreApprovalError::CandidateRejected)?;
        validate_policy(&manifest, &policy, manifest_hash)?;

        let approval_path = candidate_dir.join("approval.pb");
        let nonce_digest: [u8; 32] = Sha256::digest(&request.approval_nonce).into();
        match std::fs::symlink_metadata(&approval_path) {
            Ok(_) => {
                return self.load_stored(
                    &approval_path,
                    owner,
                    &request,
                    &manifest,
                    &policy_bytes,
                    nonce_digest,
                );
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(StoreApprovalError::CandidateRejected),
        }

        if manifest.created_at_unix_seconds > trusted_now_unix_seconds
            || trusted_now_unix_seconds >= manifest.expires_at_unix_seconds
        {
            return Err(StoreApprovalError::CandidateRejected);
        }

        let generation = RegistryLoader::new(self.config.clone())
            .load_current(&self.registry_root, trusted_now_unix_seconds)
            .map_err(|_| StoreApprovalError::CandidateRejected)?;
        let mut checked_registry = LocalRegistry::new(self.config.clone());
        checked_registry
            .install_generation(generation.clone())
            .map_err(|_| StoreApprovalError::CandidateRejected)?;

        if generation
            .policies
            .iter()
            .any(|policy| policy.claims.claims().record_id == manifest.record_id)
        {
            return Err(StoreApprovalError::Conflict);
        }
        let next_epoch = generation
            .policies
            .iter()
            .filter(|policy| policy.claims.claims().policy_name == manifest.policy_name)
            .map(|policy| policy.claims.claims().approval_epoch)
            .max()
            .map_or(Some(1), |epoch| epoch.checked_add(1))
            .ok_or(StoreApprovalError::CandidateRejected)?;
        if next_epoch != manifest.expected_approval_epoch {
            return Err(StoreApprovalError::StaleEpoch);
        }

        let trusted_key = generation
            .authority_keys
            .iter()
            .find(|key| key.key_id() == self.signer.key_id())
            .ok_or(StoreApprovalError::CandidateRejected)?;
        let key_record = trusted_key.record();
        if trusted_key.role() != AuthorityRole::PolicyApproval
            || key_record.public_key != self.signer.public_key_bytes()
            || manifest.valid_from_unix_seconds < key_record.valid_from_unix_seconds
            || manifest.valid_until_unix_seconds > key_record.valid_until_unix_seconds
        {
            return Err(StoreApprovalError::CandidateRejected);
        }

        let claims = claims_from_manifest(
            &manifest,
            &self.config.trust_domain_id,
            &request.approved_by,
            self.signer.key_id(),
        )?;
        let signature = self
            .signer
            .sign_policy_approval(&claims)
            .map_err(|_| StoreApprovalError::CandidateRejected)?;
        trusted_key
            .verify_policy_approval(trusted_now_unix_seconds, &claims, &signature)
            .map_err(|_| StoreApprovalError::CandidateRejected)?;
        let staged = staged_record(&manifest, &policy_bytes, &claims, signature)?;
        let stored = StoredPolicyApprovalWire {
            format_version: STORED_APPROVAL_FORMAT_VERSION,
            approval_nonce_digest: nonce_digest.to_vec(),
            staged_policy_record: Some(staged.clone()),
        };
        match atomic_write_new(&approval_path, &stored.encode_to_vec()) {
            Ok(()) => Ok(staged),
            Err(_) if approval_path.exists() => self.load_stored(
                &approval_path,
                owner,
                &request,
                &manifest,
                &policy_bytes,
                nonce_digest,
            ),
            Err(_) => Err(StoreApprovalError::Unavailable),
        }
    }

    fn load_stored(
        &self,
        path: &Path,
        owner: u32,
        request: &PolicyAuthorityRequestWire,
        manifest: &PolicyCandidateManifestWire,
        policy_bytes: &[u8],
        nonce_digest: [u8; 32],
    ) -> Result<proto::PolicyRecord, StoreApprovalError> {
        let bytes = read_private_file(path, owner, MAX_RECORD_BYTES)?;
        let stored: StoredPolicyApprovalWire = decode_canonical(&bytes)?;
        if stored.format_version != STORED_APPROVAL_FORMAT_VERSION
            || stored.approval_nonce_digest.as_slice() != nonce_digest
        {
            return Err(StoreApprovalError::Conflict);
        }
        let staged = stored
            .staged_policy_record
            .ok_or(StoreApprovalError::CandidateRejected)?;
        validate_staged(
            &staged,
            manifest,
            policy_bytes,
            request,
            &self.config,
            &self.signer,
        )?;
        Ok(staged)
    }
}

pub(super) fn trusted_now() -> Result<i64, StoreApprovalError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreApprovalError::Unavailable)
        .and_then(|value| {
            i64::try_from(value.as_secs()).map_err(|_| StoreApprovalError::Unavailable)
        })
}

fn validate_request(request: &PolicyAuthorityRequestWire) -> Result<(), StoreApprovalError> {
    if request.format_version != PROTOCOL_VERSION
        || !valid_public_identifier(&request.candidate_id)
        || request.expected_policy_hash.len() != 32
        || request.expected_approval_epoch == 0
        || !valid_window(
            request.valid_from_unix_seconds,
            request.valid_until_unix_seconds,
            MAX_ARTIFACT_LIFETIME_SECONDS,
        )
        || request.approval_nonce.len() != 32
        || !valid_text(&request.approved_by, MAX_STRING_BYTES as usize)
    {
        return Err(StoreApprovalError::InvalidRequest);
    }
    Ok(())
}

fn validate_manifest(manifest: &PolicyCandidateManifestWire) -> Result<(), StoreApprovalError> {
    if manifest.format_version != CANDIDATE_MANIFEST_FORMAT_VERSION
        || !valid_public_identifier(&manifest.candidate_id)
        || !valid_public_identifier(&manifest.record_id)
        || !valid_public_identifier(&manifest.policy_name)
        || !canonical_policy_version(&manifest.policy_version)
        || !schema_identifier(&manifest.resource_type)
        || !schema_identifier(&manifest.permission)
        || !valid_text(&manifest.rule_summary, MAX_STRING_BYTES as usize)
        || manifest.source_schema_digest.len() != 32
        || manifest.policy_hash.len() != 32
        || manifest.expected_approval_epoch == 0
        || !valid_window(
            manifest.valid_from_unix_seconds,
            manifest.valid_until_unix_seconds,
            MAX_ARTIFACT_LIFETIME_SECONDS,
        )
        || !valid_window(
            manifest.created_at_unix_seconds,
            manifest.expires_at_unix_seconds,
            MAX_CANDIDATE_LIFETIME_SECONDS,
        )
    {
        return Err(StoreApprovalError::CandidateRejected);
    }
    Ok(())
}

fn validate_policy(
    manifest: &PolicyCandidateManifestWire,
    policy: &ValidatedPolicyProgram,
    expected_hash: [u8; 32],
) -> Result<(), StoreApprovalError> {
    let program = policy.program();
    if manifest.source_schema_digest != program.source_schema_digest
        || !program.permissions.iter().any(|permission| {
            permission.object_type == manifest.resource_type
                && permission.name == manifest.permission
        })
        || policy_hash(policy).map_err(|_| StoreApprovalError::CandidateRejected)? != expected_hash
    {
        return Err(StoreApprovalError::CandidateRejected);
    }
    Ok(())
}

fn claims_from_manifest(
    manifest: &PolicyCandidateManifestWire,
    trust_domain_id: &str,
    approved_by: &str,
    signing_key_id: &str,
) -> Result<ValidatedPolicyApprovalClaimsV1, StoreApprovalError> {
    ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: trust_domain_id.into(),
            record_id: manifest.record_id.clone(),
            policy_name: manifest.policy_name.clone(),
            policy_version: manifest.policy_version.clone(),
            approval_epoch: manifest.expected_approval_epoch,
            policy_hash: fixed_hash(&manifest.policy_hash)
                .ok_or(StoreApprovalError::CandidateRejected)?,
            valid_from_unix_seconds: manifest.valid_from_unix_seconds,
            valid_until_unix_seconds: manifest.valid_until_unix_seconds,
            approved_by: approved_by.into(),
            signing_key_id: signing_key_id.into(),
        },
        PolicyApprovalLimits {
            max_string_bytes: MAX_STRING_BYTES,
            max_lifetime_seconds: MAX_ARTIFACT_LIFETIME_SECONDS,
        },
    )
    .map_err(|_| StoreApprovalError::CandidateRejected)
}

fn staged_record(
    manifest: &PolicyCandidateManifestWire,
    policy_bytes: &[u8],
    claims: &ValidatedPolicyApprovalClaimsV1,
    signature: [u8; 64],
) -> Result<proto::PolicyRecord, StoreApprovalError> {
    let policy = proto::PolicyProgram::decode(policy_bytes)
        .map_err(|_| StoreApprovalError::CandidateRejected)?;
    let claims = claims.claims();
    Ok(proto::PolicyRecord {
        format_version: POLICY_RECORD_FORMAT_VERSION,
        record_id: manifest.record_id.clone(),
        policy_name: manifest.policy_name.clone(),
        policy_version: manifest.policy_version.clone(),
        canonical_policy_ir: Some(policy),
        policy_hash: claims.policy_hash.to_vec(),
        status: proto::PolicyStatus::Unspecified as i32,
        valid_from: Some(timestamp(claims.valid_from_unix_seconds)),
        valid_until: Some(timestamp(claims.valid_until_unix_seconds)),
        approved_by: claims.approved_by.clone(),
        signing_key_id: claims.signing_key_id.clone(),
        authority_signature: signature.to_vec(),
        registered_at: None,
        trust_domain_id: claims.trust_domain_id.clone(),
        approval_epoch: claims.approval_epoch,
        revoked_at: None,
        revocation_reason: String::new(),
        registry_generation: 0,
    })
}

fn validate_staged(
    staged: &proto::PolicyRecord,
    manifest: &PolicyCandidateManifestWire,
    policy_bytes: &[u8],
    request: &PolicyAuthorityRequestWire,
    config: &RegistryConfig,
    signer: &AuthoritySigner<PolicyApprovalRole>,
) -> Result<(), StoreApprovalError> {
    let canonical_policy_ir = staged
        .canonical_policy_ir
        .as_ref()
        .ok_or(StoreApprovalError::CandidateRejected)?;
    if staged.format_version != POLICY_RECORD_FORMAT_VERSION
        || staged.record_id != manifest.record_id
        || staged.policy_name != manifest.policy_name
        || staged.policy_version != manifest.policy_version
        || canonical_policy_ir.encode_to_vec() != policy_bytes
        || staged.policy_hash != manifest.policy_hash
        || staged.status != proto::PolicyStatus::Unspecified as i32
        || staged.valid_from.as_ref() != Some(&timestamp(manifest.valid_from_unix_seconds))
        || staged.valid_until.as_ref() != Some(&timestamp(manifest.valid_until_unix_seconds))
        || staged.approved_by != request.approved_by
        || staged.signing_key_id != signer.key_id()
        || staged.registered_at.is_some()
        || staged.trust_domain_id != config.trust_domain_id
        || staged.approval_epoch != manifest.expected_approval_epoch
        || staged.revoked_at.is_some()
        || !staged.revocation_reason.is_empty()
        || staged.registry_generation != 0
    {
        return Err(StoreApprovalError::CandidateRejected);
    }
    let claims = claims_from_manifest(
        manifest,
        &config.trust_domain_id,
        &request.approved_by,
        signer.key_id(),
    )?;
    let expected_signature = signer
        .sign_policy_approval(&claims)
        .map_err(|_| StoreApprovalError::CandidateRejected)?;
    if staged.authority_signature != expected_signature {
        return Err(StoreApprovalError::CandidateRejected);
    }
    Ok(())
}

fn decode_canonical<M: Message + Default>(bytes: &[u8]) -> Result<M, StoreApprovalError> {
    let value = M::decode(bytes).map_err(|_| StoreApprovalError::CandidateRejected)?;
    if value.encode_to_vec() != bytes {
        return Err(StoreApprovalError::CandidateRejected);
    }
    Ok(value)
}

fn fixed_hash(value: &[u8]) -> Option<[u8; 32]> {
    value.try_into().ok()
}

fn valid_public_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STRING_BYTES as usize
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
}

fn schema_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'_'))
        })
}

fn canonical_policy_version(value: &str) -> bool {
    value
        .parse::<u32>()
        .is_ok_and(|parsed| parsed > 0 && value == parsed.to_string())
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.is_ascii()
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
}

fn valid_window(start: i64, end: i64, maximum_seconds: u64) -> bool {
    end.checked_sub(start)
        .and_then(|seconds| u64::try_from(seconds).ok())
        .is_some_and(|seconds| seconds > 0 && seconds <= maximum_seconds)
}

fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
}

#[cfg(unix)]
fn validate_private_directory(
    path: &Path,
    expected_parent: Option<&Path>,
) -> Result<u32, StoreApprovalError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| StoreApprovalError::CandidateRejected)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(StoreApprovalError::CandidateRejected);
    }
    if let Some(parent) = expected_parent {
        let parent_metadata =
            std::fs::symlink_metadata(parent).map_err(|_| StoreApprovalError::CandidateRejected)?;
        if metadata.uid() != parent_metadata.uid() {
            return Err(StoreApprovalError::CandidateRejected);
        }
    }
    Ok(metadata.uid())
}

#[cfg(not(unix))]
fn validate_private_directory(
    _path: &Path,
    _expected_parent: Option<&Path>,
) -> Result<u32, StoreApprovalError> {
    Err(StoreApprovalError::CandidateRejected)
}

#[cfg(unix)]
fn read_private_file(
    path: &Path,
    expected_owner: u32,
    max_bytes: u64,
) -> Result<Vec<u8>, StoreApprovalError> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| StoreApprovalError::CandidateRejected)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_owner
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.len() > max_bytes
    {
        return Err(StoreApprovalError::CandidateRejected);
    }
    let file = std::fs::File::open(path).map_err(|_| StoreApprovalError::CandidateRejected)?;
    let opened_metadata = file
        .metadata()
        .map_err(|_| StoreApprovalError::CandidateRejected)?;
    if metadata.dev() != opened_metadata.dev() || metadata.ino() != opened_metadata.ino() {
        return Err(StoreApprovalError::CandidateRejected);
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StoreApprovalError::CandidateRejected)?;
    if bytes.len() as u64 > max_bytes {
        return Err(StoreApprovalError::CandidateRejected);
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_private_file(
    _path: &Path,
    _expected_owner: u32,
    _max_bytes: u64,
) -> Result<Vec<u8>, StoreApprovalError> {
    Err(StoreApprovalError::CandidateRejected)
}
