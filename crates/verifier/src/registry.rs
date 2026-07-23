use std::collections::{BTreeMap, BTreeSet};

use raziel_authority::{AuthorityKeyStatus, AuthorityRole, TrustedAuthorityKey};
use raziel_canonical::policy_hash;
use raziel_policy_ir::decode_policy_program_strict;

use crate::{
    AcceptedRootRecordV1, PolicyRecordStatus, PolicyRegistryRecordV1, RegistryConfig,
    RegistryError, RootRecordStatus,
};

const PROTOBUF_TIMESTAMP_MIN_SECONDS: i64 = -62_135_596_800;
const PROTOBUF_TIMESTAMP_MAX_SECONDS: i64 = 253_402_300_799;

#[derive(Clone)]
pub struct RegistryGenerationV1 {
    pub generation: u64,
    pub publication_digest: [u8; 32],
    pub authority_keys: Vec<TrustedAuthorityKey>,
    pub policies: Vec<PolicyRegistryRecordV1>,
    pub accepted_roots: Vec<AcceptedRootRecordV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed,
    AlreadyCurrent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRoot {
    pub root_record: AcceptedRootRecordV1,
    pub policy_record: PolicyRegistryRecordV1,
}

pub struct LocalRegistry {
    config: RegistryConfig,
    installed: Option<InstalledGeneration>,
    observed_time_floor: Option<i64>,
}

impl LocalRegistry {
    pub const fn new(config: RegistryConfig) -> Self {
        Self {
            config,
            installed: None,
            observed_time_floor: None,
        }
    }

    pub fn generation(&self) -> Option<u64> {
        self.installed.as_ref().map(|value| value.generation)
    }

    pub const fn observed_time_floor(&self) -> Option<i64> {
        self.observed_time_floor
    }

    pub fn install_generation(
        &mut self,
        candidate: RegistryGenerationV1,
    ) -> Result<InstallOutcome, RegistryError> {
        if candidate.generation == 0 {
            return Err(RegistryError::RegistryGenerationInvalid);
        }
        if let Some(current) = &self.installed {
            if candidate.generation < current.generation {
                return Err(RegistryError::RegistryGenerationRollback);
            }
            if candidate.generation == current.generation {
                return if candidate.publication_digest == current.publication_digest {
                    Ok(InstallOutcome::AlreadyCurrent)
                } else {
                    Err(RegistryError::RegistryGenerationRollback)
                };
            }
        }

        let installed = InstalledGeneration::validate(candidate, &self.config)?;
        if let Some(current) = &self.installed {
            validate_transition(current, &installed)?;
        }
        self.installed = Some(installed);
        Ok(InstallOutcome::Installed)
    }

    pub fn resolve_policy_exact(
        &mut self,
        record_id: &str,
        expected_policy_hash: &[u8; 32],
        observed_now_unix_seconds: i64,
    ) -> Result<PolicyRegistryRecordV1, RegistryError> {
        let now = self.observe_time(observed_now_unix_seconds);
        let installed = self
            .installed
            .as_ref()
            .ok_or(RegistryError::UnknownPolicy)?;
        resolve_policy_exact(installed, record_id, expected_policy_hash, now)
    }

    pub fn resolve_policy_hash(
        &mut self,
        policy_hash: &[u8; 32],
        observed_now_unix_seconds: i64,
    ) -> Result<PolicyRegistryRecordV1, RegistryError> {
        let now = self.observe_time(observed_now_unix_seconds);
        let installed = self
            .installed
            .as_ref()
            .ok_or(RegistryError::UnknownPolicy)?;

        let mut policy_names = BTreeSet::new();
        let mut record_ids = Vec::new();
        for record in installed.policies.values() {
            let claims = record.claims.claims();
            if &claims.policy_hash == policy_hash {
                policy_names.insert(claims.policy_name.as_str());
                if installed
                    .policy_high_watermarks
                    .get(&claims.policy_name)
                    .is_some_and(|epoch| *epoch == claims.approval_epoch)
                {
                    record_ids.push(claims.record_id.as_str());
                }
            }
        }
        if policy_names.is_empty() {
            return Err(RegistryError::UnknownPolicy);
        }
        if policy_names.len() != 1 || record_ids.len() != 1 {
            return Err(RegistryError::AmbiguousPolicy);
        }
        resolve_policy_exact(installed, record_ids[0], policy_hash, now)
    }

    pub fn resolve_root(
        &mut self,
        relationship_root: &[u8; 32],
        observed_now_unix_seconds: i64,
    ) -> Result<ResolvedRoot, RegistryError> {
        let now = self.observe_time(observed_now_unix_seconds);
        let installed = self.installed.as_ref().ok_or(RegistryError::UnknownRoot)?;
        let snapshot_id = installed
            .root_by_digest
            .get(relationship_root)
            .ok_or(RegistryError::UnknownRoot)?;
        let root_record = installed
            .accepted_roots
            .get(snapshot_id)
            .ok_or(RegistryError::UnknownRoot)?;
        let claims = root_record.claims.claims();

        if root_record.status == RootRecordStatus::Revoked {
            return Err(RegistryError::RevokedRoot);
        }
        if now < claims.issued_at_unix_seconds || now >= claims.expires_at_unix_seconds {
            return Err(RegistryError::ExpiredRoot);
        }
        let key = installed
            .authority_keys
            .get(&claims.signing_key_id)
            .ok_or(RegistryError::UnknownAuthorityKey)?;
        key.verify_snapshot_descriptor(now, &root_record.claims, &root_record.authority_signature)?;

        let policy_record = resolve_policy_exact(
            installed,
            &claims.policy_record_id,
            &claims.policy_hash,
            now,
        )?;
        Ok(ResolvedRoot {
            root_record: root_record.clone(),
            policy_record,
        })
    }

    fn observe_time(&mut self, observed_now_unix_seconds: i64) -> i64 {
        let effective_now = self
            .observed_time_floor
            .map_or(observed_now_unix_seconds, |floor| {
                floor.max(observed_now_unix_seconds)
            });
        self.observed_time_floor = Some(effective_now);
        effective_now
    }
}

struct InstalledGeneration {
    generation: u64,
    publication_digest: [u8; 32],
    authority_keys: BTreeMap<String, TrustedAuthorityKey>,
    policies: BTreeMap<String, PolicyRegistryRecordV1>,
    accepted_roots: BTreeMap<String, AcceptedRootRecordV1>,
    root_by_digest: BTreeMap<[u8; 32], String>,
    policy_high_watermarks: BTreeMap<String, u64>,
}

impl InstalledGeneration {
    fn validate(
        candidate: RegistryGenerationV1,
        config: &RegistryConfig,
    ) -> Result<Self, RegistryError> {
        let generation = candidate.generation;
        let mut authority_keys = BTreeMap::new();
        let mut key_roles_by_public_key = BTreeMap::new();
        for key in candidate.authority_keys {
            if key.record().registry_generation != generation {
                return Err(RegistryError::RegistryGenerationInvalid);
            }
            if key.record().trust_domain_id != config.trust_domain_id {
                return Err(RegistryError::TrustDomainMismatch);
            }
            validate_timestamp(key.record().valid_from_unix_seconds)?;
            validate_timestamp(key.record().valid_until_unix_seconds)?;
            validate_optional_timestamp(key.record().revoked_at_unix_seconds)?;
            validate_optional_text(
                key.record().revocation_reason.as_deref(),
                config.limits.max_string_bytes,
            )?;
            if let Some(previous_role) =
                key_roles_by_public_key.insert(key.record().public_key, key.role())
            {
                if previous_role != key.role() {
                    return Err(RegistryError::WrongAuthorityRole);
                }
                return Err(RegistryError::DuplicateRecord);
            }
            if authority_keys.insert(key.key_id().into(), key).is_some() {
                return Err(RegistryError::DuplicateRecord);
            }
        }

        let mut policies = BTreeMap::new();
        let mut policy_epochs: BTreeMap<String, BTreeMap<u64, String>> = BTreeMap::new();
        for policy in candidate.policies {
            if policy.registry_generation != generation {
                return Err(RegistryError::RegistryGenerationInvalid);
            }
            validate_mutable_record(
                policy.registered_at_unix_seconds,
                policy.revoked_at_unix_seconds,
                policy.revocation_reason.as_deref(),
                policy.status == PolicyRecordStatus::Revoked,
                config.limits.max_string_bytes,
            )?;
            let claims = policy.claims.claims();
            if claims.trust_domain_id != config.trust_domain_id {
                return Err(RegistryError::TrustDomainMismatch);
            }
            let key = authority_keys
                .get(&claims.signing_key_id)
                .ok_or(RegistryError::UnknownAuthorityKey)?;
            if key.role() != AuthorityRole::PolicyApproval {
                return Err(RegistryError::WrongAuthorityRole);
            }
            let in_memory_hash = policy_hash(&policy.policy)?;
            let decoded_policy = decode_policy_program_strict(
                &policy.canonical_policy_ir_bytes,
                config.limits.max_record_bytes,
                &config.limits.policy_validation,
            )?;
            let encoded_hash = policy_hash(&decoded_policy)?;
            if in_memory_hash != claims.policy_hash
                || encoded_hash != claims.policy_hash
                || decoded_policy != policy.policy
            {
                return Err(RegistryError::PolicyHashMismatch);
            }
            key.verify_policy_approval_signature(&policy.claims, &policy.authority_signature)?;
            validate_key_window(
                key,
                claims.valid_from_unix_seconds,
                claims.valid_until_unix_seconds,
            )?;
            if policy_epochs
                .entry(claims.policy_name.clone())
                .or_default()
                .insert(claims.approval_epoch, claims.record_id.clone())
                .is_some()
            {
                return Err(RegistryError::EpochConflict);
            }
            if policies.insert(claims.record_id.clone(), policy).is_some() {
                return Err(RegistryError::DuplicateRecord);
            }
        }

        let mut policy_high_watermarks = BTreeMap::new();
        for (policy_name, epochs) in &policy_epochs {
            validate_contiguous_epochs(epochs.keys().copied())?;
            let high = epochs
                .last_key_value()
                .map(|(epoch, _)| *epoch)
                .ok_or(RegistryError::InvalidEpoch)?;
            policy_high_watermarks.insert(policy_name.clone(), high);
        }

        let mut accepted_roots = BTreeMap::new();
        let mut root_by_digest = BTreeMap::new();
        let mut snapshot_epochs: BTreeMap<[u8; 32], BTreeMap<u64, String>> = BTreeMap::new();
        for root in candidate.accepted_roots {
            if root.registry_generation != generation {
                return Err(RegistryError::RegistryGenerationInvalid);
            }
            validate_mutable_record(
                root.registered_at_unix_seconds,
                root.revoked_at_unix_seconds,
                root.revocation_reason.as_deref(),
                root.status == RootRecordStatus::Revoked,
                config.limits.max_string_bytes,
            )?;
            let claims = root.claims.claims();
            if claims.trust_domain_id != config.trust_domain_id {
                return Err(RegistryError::TrustDomainMismatch);
            }
            let key = authority_keys
                .get(&claims.signing_key_id)
                .ok_or(RegistryError::UnknownAuthorityKey)?;
            if key.role() != AuthorityRole::Snapshot {
                return Err(RegistryError::WrongAuthorityRole);
            }
            key.verify_snapshot_descriptor_signature(&root.claims, &root.authority_signature)?;
            validate_key_window(
                key,
                claims.issued_at_unix_seconds,
                claims.expires_at_unix_seconds,
            )?;
            let policy = policies
                .get(&claims.policy_record_id)
                .ok_or(RegistryError::UnknownPolicy)?;
            let policy_claims = policy.claims.claims();
            if policy_claims.policy_hash != claims.policy_hash {
                return Err(RegistryError::PolicyRecordMismatch);
            }
            if claims.issued_at_unix_seconds < policy_claims.valid_from_unix_seconds
                || claims.expires_at_unix_seconds > policy_claims.valid_until_unix_seconds
            {
                return Err(RegistryError::InvalidValidityWindow);
            }
            if snapshot_epochs
                .entry(claims.policy_hash)
                .or_default()
                .insert(claims.epoch, claims.snapshot_id.clone())
                .is_some()
            {
                return Err(RegistryError::EpochConflict);
            }
            if root_by_digest
                .insert(
                    claims.relationship_snapshot_root,
                    claims.snapshot_id.clone(),
                )
                .is_some()
            {
                return Err(RegistryError::DuplicateRecord);
            }
            if accepted_roots
                .insert(claims.snapshot_id.clone(), root)
                .is_some()
            {
                return Err(RegistryError::DuplicateRecord);
            }
        }
        for epochs in snapshot_epochs.values() {
            validate_contiguous_epochs(epochs.keys().copied())?;
        }

        Ok(Self {
            generation,
            publication_digest: candidate.publication_digest,
            authority_keys,
            policies,
            accepted_roots,
            root_by_digest,
            policy_high_watermarks,
        })
    }
}

fn resolve_policy_exact(
    installed: &InstalledGeneration,
    record_id: &str,
    expected_policy_hash: &[u8; 32],
    now: i64,
) -> Result<PolicyRegistryRecordV1, RegistryError> {
    let record = installed
        .policies
        .get(record_id)
        .ok_or(RegistryError::UnknownPolicy)?;
    let claims = record.claims.claims();
    if &claims.policy_hash != expected_policy_hash {
        return Err(RegistryError::PolicyHashMismatch);
    }
    if installed
        .policy_high_watermarks
        .get(&claims.policy_name)
        .is_some_and(|epoch| *epoch != claims.approval_epoch)
    {
        return Err(RegistryError::UnapprovedPolicy);
    }
    if record.status == PolicyRecordStatus::Revoked {
        return Err(RegistryError::RevokedPolicy);
    }
    if now < claims.valid_from_unix_seconds || now >= claims.valid_until_unix_seconds {
        return Err(RegistryError::ExpiredPolicy);
    }
    let key = installed
        .authority_keys
        .get(&claims.signing_key_id)
        .ok_or(RegistryError::UnknownAuthorityKey)?;
    key.verify_policy_approval(now, &record.claims, &record.authority_signature)?;
    Ok(record.clone())
}

fn validate_key_window(
    key: &TrustedAuthorityKey,
    artifact_start: i64,
    artifact_end: i64,
) -> Result<(), RegistryError> {
    let record = key.record();
    if artifact_start < record.valid_from_unix_seconds
        || artifact_end > record.valid_until_unix_seconds
    {
        return Err(RegistryError::InvalidValidityWindow);
    }
    Ok(())
}

fn validate_contiguous_epochs(epochs: impl Iterator<Item = u64>) -> Result<(), RegistryError> {
    let mut expected = 1_u64;
    for epoch in epochs {
        if epoch != expected {
            return Err(RegistryError::EpochGap);
        }
        expected = expected
            .checked_add(1)
            .ok_or(RegistryError::EpochOverflow)?;
    }
    Ok(())
}

fn validate_mutable_record(
    registered_at: i64,
    revoked_at: Option<i64>,
    revocation_reason: Option<&str>,
    is_revoked: bool,
    max_string_bytes: u32,
) -> Result<(), RegistryError> {
    validate_timestamp(registered_at)?;
    validate_optional_timestamp(revoked_at)?;
    validate_optional_text(revocation_reason, max_string_bytes)?;
    match (is_revoked, revoked_at, revocation_reason) {
        (false, None, None) | (true, Some(_), _) => Ok(()),
        _ => Err(RegistryError::InvalidValidityWindow),
    }
}

fn validate_timestamp(value: i64) -> Result<(), RegistryError> {
    if !(PROTOBUF_TIMESTAMP_MIN_SECONDS..=PROTOBUF_TIMESTAMP_MAX_SECONDS).contains(&value) {
        return Err(RegistryError::InvalidTimestamp);
    }
    Ok(())
}

fn validate_optional_timestamp(value: Option<i64>) -> Result<(), RegistryError> {
    if let Some(value) = value {
        validate_timestamp(value)?;
    }
    Ok(())
}

fn validate_optional_text(value: Option<&str>, max_string_bytes: u32) -> Result<(), RegistryError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_empty()
        || value.len() > max_string_bytes as usize
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
    {
        return Err(RegistryError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_transition(
    current: &InstalledGeneration,
    candidate: &InstalledGeneration,
) -> Result<(), RegistryError> {
    for (key_id, old) in &current.authority_keys {
        let new = candidate
            .authority_keys
            .get(key_id)
            .ok_or(RegistryError::ImmutableRecordChanged)?;
        validate_key_transition(old, new)?;
    }
    for (record_id, old) in &current.policies {
        let new = candidate
            .policies
            .get(record_id)
            .ok_or(RegistryError::ImmutableRecordChanged)?;
        validate_policy_transition(old, new)?;
    }
    for (snapshot_id, old) in &current.accepted_roots {
        let new = candidate
            .accepted_roots
            .get(snapshot_id)
            .ok_or(RegistryError::ImmutableRecordChanged)?;
        validate_root_transition(old, new)?;
    }
    Ok(())
}

fn validate_key_transition(
    old: &TrustedAuthorityKey,
    new: &TrustedAuthorityKey,
) -> Result<(), RegistryError> {
    let old = old.record();
    let new = new.record();
    if old.format_version != new.format_version
        || old.trust_domain_id != new.trust_domain_id
        || old.key_id != new.key_id
        || old.role != new.role
        || old.public_key != new.public_key
        || old.valid_from_unix_seconds != new.valid_from_unix_seconds
        || old.valid_until_unix_seconds != new.valid_until_unix_seconds
    {
        return Err(RegistryError::ImmutableRecordChanged);
    }
    validate_status_transition(
        old.status == AuthorityKeyStatus::Revoked,
        old.revoked_at_unix_seconds,
        old.revocation_reason.as_deref(),
        new.status == AuthorityKeyStatus::Revoked,
        new.revoked_at_unix_seconds,
        new.revocation_reason.as_deref(),
    )
}

fn validate_policy_transition(
    old: &PolicyRegistryRecordV1,
    new: &PolicyRegistryRecordV1,
) -> Result<(), RegistryError> {
    if old.claims != new.claims
        || old.policy != new.policy
        || old.canonical_policy_ir_bytes != new.canonical_policy_ir_bytes
        || old.authority_signature != new.authority_signature
        || old.registered_at_unix_seconds != new.registered_at_unix_seconds
    {
        return Err(RegistryError::ImmutableRecordChanged);
    }
    validate_status_transition(
        old.status == PolicyRecordStatus::Revoked,
        old.revoked_at_unix_seconds,
        old.revocation_reason.as_deref(),
        new.status == PolicyRecordStatus::Revoked,
        new.revoked_at_unix_seconds,
        new.revocation_reason.as_deref(),
    )
}

fn validate_root_transition(
    old: &AcceptedRootRecordV1,
    new: &AcceptedRootRecordV1,
) -> Result<(), RegistryError> {
    if old.claims != new.claims
        || old.authority_signature != new.authority_signature
        || old.registered_at_unix_seconds != new.registered_at_unix_seconds
    {
        return Err(RegistryError::ImmutableRecordChanged);
    }
    validate_status_transition(
        old.status == RootRecordStatus::Revoked,
        old.revoked_at_unix_seconds,
        old.revocation_reason.as_deref(),
        new.status == RootRecordStatus::Revoked,
        new.revoked_at_unix_seconds,
        new.revocation_reason.as_deref(),
    )
}

fn validate_status_transition(
    old_revoked: bool,
    old_revoked_at: Option<i64>,
    old_reason: Option<&str>,
    new_revoked: bool,
    new_revoked_at: Option<i64>,
    new_reason: Option<&str>,
) -> Result<(), RegistryError> {
    if old_revoked && !new_revoked {
        return Err(RegistryError::RevocationRollback);
    }
    if old_revoked && (old_revoked_at != new_revoked_at || old_reason != new_reason) {
        return Err(RegistryError::ImmutableRecordChanged);
    }
    Ok(())
}
