use prost::Message;
use prost_types::Timestamp;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    TrustedAuthorityKey,
};
use raziel_canonical::{
    PolicyApprovalClaimsV1, PolicyApprovalLimits, ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::{ValidatedPolicyProgram, ValidationCeilings};
use raziel_proto::raziel::v1 as proto;
use raziel_snapshot::{
    SnapshotDescriptorClaimsV1, SnapshotDescriptorLimits, ValidatedSnapshotDescriptorClaimsV1,
};

use crate::RegistryError;

const RECORD_FORMAT_VERSION: u32 = 1;
const PROTOBUF_TIMESTAMP_MIN_SECONDS: i64 = -62_135_596_800;
const PROTOBUF_TIMESTAMP_MAX_SECONDS: i64 = 253_402_300_799;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryConfig {
    pub trust_domain_id: String,
    pub limits: RegistryLimits,
}

impl RegistryConfig {
    pub fn new(trust_domain_id: String, limits: RegistryLimits) -> Result<Self, RegistryError> {
        validate_identifier(&trust_domain_id, limits.max_string_bytes)?;
        Ok(Self {
            trust_domain_id,
            limits,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryLimits {
    pub max_record_bytes: u64,
    pub max_string_bytes: u32,
    pub max_artifact_lifetime_seconds: u64,
    pub max_future_clock_skew_seconds: u64,
    pub max_manifest_files_per_kind: u32,
    pub max_manifest_total_bytes: u64,
    pub policy_validation: ValidationCeilings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRecordStatus {
    Approved,
    Revoked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRegistryRecordV1 {
    pub claims: ValidatedPolicyApprovalClaimsV1,
    pub policy: ValidatedPolicyProgram,
    pub canonical_policy_ir_bytes: Vec<u8>,
    pub authority_signature: [u8; 64],
    pub status: PolicyRecordStatus,
    pub registered_at_unix_seconds: i64,
    pub revoked_at_unix_seconds: Option<i64>,
    pub revocation_reason: Option<String>,
    pub registry_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootRecordStatus {
    Accepted,
    Revoked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedRootRecordV1 {
    pub claims: ValidatedSnapshotDescriptorClaimsV1,
    pub authority_signature: [u8; 64],
    pub status: RootRecordStatus,
    pub registered_at_unix_seconds: i64,
    pub revoked_at_unix_seconds: Option<i64>,
    pub revocation_reason: Option<String>,
    pub registry_generation: u64,
}

pub fn authority_key_from_proto(
    value: proto::AuthorityKeyRecord,
    config: &RegistryConfig,
) -> Result<TrustedAuthorityKey, RegistryError> {
    if value.format_version != AUTHORITY_KEY_FORMAT_VERSION {
        return Err(RegistryError::UnsupportedFormatVersion);
    }
    validate_identifier(&value.key_id, config.limits.max_string_bytes)?;
    validate_identifier(&value.trust_domain_id, config.limits.max_string_bytes)?;
    let role = match proto::AuthorityRole::try_from(value.role) {
        Ok(proto::AuthorityRole::PolicyApproval) => AuthorityRole::PolicyApproval,
        Ok(proto::AuthorityRole::Snapshot) => AuthorityRole::Snapshot,
        _ => return Err(RegistryError::InvalidEnum),
    };
    let status = match proto::AuthorityKeyStatus::try_from(value.status) {
        Ok(proto::AuthorityKeyStatus::Active) => AuthorityKeyStatus::Active,
        Ok(proto::AuthorityKeyStatus::Revoked) => AuthorityKeyStatus::Revoked,
        _ => return Err(RegistryError::InvalidEnum),
    };
    let public_key = fixed_hash(value.public_key)?;
    let valid_from_unix_seconds = required_timestamp(value.valid_from)?;
    let valid_until_unix_seconds = required_timestamp(value.valid_until)?;
    let revoked_at_unix_seconds = optional_timestamp(value.revoked_at)?;
    let revocation_reason =
        optional_string(value.revocation_reason, config.limits.max_string_bytes)?;
    validate_revocation(
        status == AuthorityKeyStatus::Revoked,
        revoked_at_unix_seconds,
        revocation_reason.as_deref(),
    )?;

    TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: value.format_version,
            trust_domain_id: value.trust_domain_id,
            key_id: value.key_id,
            role,
            public_key,
            status,
            valid_from_unix_seconds,
            valid_until_unix_seconds,
            revoked_at_unix_seconds,
            revocation_reason,
            registry_generation: value.registry_generation,
        },
        &config.trust_domain_id,
    )
    .map_err(RegistryError::from)
}

pub fn policy_record_from_proto(
    value: proto::PolicyRecord,
    config: &RegistryConfig,
) -> Result<PolicyRegistryRecordV1, RegistryError> {
    if value.format_version != RECORD_FORMAT_VERSION {
        return Err(RegistryError::UnsupportedFormatVersion);
    }
    require_trust_domain(&value.trust_domain_id, config)?;
    let status = match proto::PolicyStatus::try_from(value.status) {
        Ok(proto::PolicyStatus::Approved) => PolicyRecordStatus::Approved,
        Ok(proto::PolicyStatus::Revoked) => PolicyRecordStatus::Revoked,
        _ => return Err(RegistryError::InvalidEnum),
    };
    let policy_wire = value
        .canonical_policy_ir
        .ok_or(RegistryError::MissingField)?;
    let canonical_policy_ir_bytes = policy_wire.encode_to_vec();
    let policy =
        ValidatedPolicyProgram::from_trusted_proto(policy_wire, &config.limits.policy_validation)?;
    let recalculated_hash = policy_hash(&policy)?;
    let recorded_hash = fixed_hash(value.policy_hash)?;
    if recalculated_hash != recorded_hash {
        return Err(RegistryError::PolicyHashMismatch);
    }

    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: value.format_version,
            trust_domain_id: value.trust_domain_id,
            record_id: value.record_id,
            policy_name: value.policy_name,
            policy_version: value.policy_version,
            approval_epoch: value.approval_epoch,
            policy_hash: recorded_hash,
            valid_from_unix_seconds: required_timestamp(value.valid_from)?,
            valid_until_unix_seconds: required_timestamp(value.valid_until)?,
            approved_by: value.approved_by,
            signing_key_id: value.signing_key_id,
        },
        PolicyApprovalLimits {
            max_string_bytes: config.limits.max_string_bytes,
            max_lifetime_seconds: config.limits.max_artifact_lifetime_seconds,
        },
    )?;

    let registered_at_unix_seconds = required_timestamp(value.registered_at)?;
    let revoked_at_unix_seconds = optional_timestamp(value.revoked_at)?;
    let revocation_reason =
        optional_string(value.revocation_reason, config.limits.max_string_bytes)?;
    validate_revocation(
        status == PolicyRecordStatus::Revoked,
        revoked_at_unix_seconds,
        revocation_reason.as_deref(),
    )?;
    if value.registry_generation == 0 {
        return Err(RegistryError::RegistryGenerationInvalid);
    }

    Ok(PolicyRegistryRecordV1 {
        claims,
        policy,
        canonical_policy_ir_bytes,
        authority_signature: fixed_signature(value.authority_signature)?,
        status,
        registered_at_unix_seconds,
        revoked_at_unix_seconds,
        revocation_reason,
        registry_generation: value.registry_generation,
    })
}

pub fn authority_key_to_proto(key: &TrustedAuthorityKey) -> proto::AuthorityKeyRecord {
    let record = key.record();
    proto::AuthorityKeyRecord {
        format_version: record.format_version,
        trust_domain_id: record.trust_domain_id.clone(),
        key_id: record.key_id.clone(),
        role: match record.role {
            AuthorityRole::PolicyApproval => proto::AuthorityRole::PolicyApproval as i32,
            AuthorityRole::Snapshot => proto::AuthorityRole::Snapshot as i32,
        },
        public_key: record.public_key.to_vec(),
        status: match record.status {
            AuthorityKeyStatus::Active => proto::AuthorityKeyStatus::Active as i32,
            AuthorityKeyStatus::Revoked => proto::AuthorityKeyStatus::Revoked as i32,
        },
        valid_from: Some(timestamp(record.valid_from_unix_seconds)),
        valid_until: Some(timestamp(record.valid_until_unix_seconds)),
        revoked_at: record.revoked_at_unix_seconds.map(timestamp),
        revocation_reason: record.revocation_reason.clone().unwrap_or_default(),
        registry_generation: record.registry_generation,
    }
}

pub fn policy_record_to_proto(
    record: &PolicyRegistryRecordV1,
) -> Result<proto::PolicyRecord, RegistryError> {
    let claims = record.claims.claims();
    let policy = proto::PolicyProgram::decode(record.canonical_policy_ir_bytes.as_slice())
        .map_err(|_| RegistryError::MalformedProtobuf)?;
    Ok(proto::PolicyRecord {
        format_version: claims.format_version,
        record_id: claims.record_id.clone(),
        policy_name: claims.policy_name.clone(),
        policy_version: claims.policy_version.clone(),
        canonical_policy_ir: Some(policy),
        policy_hash: claims.policy_hash.to_vec(),
        status: match record.status {
            PolicyRecordStatus::Approved => proto::PolicyStatus::Approved as i32,
            PolicyRecordStatus::Revoked => proto::PolicyStatus::Revoked as i32,
        },
        valid_from: Some(timestamp(claims.valid_from_unix_seconds)),
        valid_until: Some(timestamp(claims.valid_until_unix_seconds)),
        approved_by: claims.approved_by.clone(),
        signing_key_id: claims.signing_key_id.clone(),
        authority_signature: record.authority_signature.to_vec(),
        registered_at: Some(timestamp(record.registered_at_unix_seconds)),
        trust_domain_id: claims.trust_domain_id.clone(),
        approval_epoch: claims.approval_epoch,
        revoked_at: record.revoked_at_unix_seconds.map(timestamp),
        revocation_reason: record.revocation_reason.clone().unwrap_or_default(),
        registry_generation: record.registry_generation,
    })
}

pub fn root_record_to_proto(record: &AcceptedRootRecordV1) -> proto::AcceptedRootRecord {
    let claims = record.claims.claims();
    proto::AcceptedRootRecord {
        format_version: RECORD_FORMAT_VERSION,
        descriptor: Some(proto::RelationshipSnapshotDescriptor {
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
            authority_signature: record.authority_signature.to_vec(),
            trust_domain_id: claims.trust_domain_id.clone(),
            policy_record_id: claims.policy_record_id.clone(),
        }),
        status: match record.status {
            RootRecordStatus::Accepted => proto::SnapshotStatus::Accepted as i32,
            RootRecordStatus::Revoked => proto::SnapshotStatus::Revoked as i32,
        },
        registered_at: Some(timestamp(record.registered_at_unix_seconds)),
        revoked_at: record.revoked_at_unix_seconds.map(timestamp),
        revocation_reason: record.revocation_reason.clone().unwrap_or_default(),
        registry_generation: record.registry_generation,
    }
}

pub fn root_record_from_proto(
    value: proto::AcceptedRootRecord,
    config: &RegistryConfig,
    trusted_now_unix_seconds: i64,
) -> Result<AcceptedRootRecordV1, RegistryError> {
    if value.format_version != RECORD_FORMAT_VERSION {
        return Err(RegistryError::UnsupportedFormatVersion);
    }
    let status = match proto::SnapshotStatus::try_from(value.status) {
        Ok(proto::SnapshotStatus::Accepted) => RootRecordStatus::Accepted,
        Ok(proto::SnapshotStatus::Revoked) => RootRecordStatus::Revoked,
        _ => return Err(RegistryError::InvalidEnum),
    };
    let descriptor = value.descriptor.ok_or(RegistryError::MissingField)?;
    require_trust_domain(&descriptor.trust_domain_id, config)?;
    let authority_signature = fixed_signature(descriptor.authority_signature)?;
    let claims = ValidatedSnapshotDescriptorClaimsV1::new(
        SnapshotDescriptorClaimsV1 {
            format_version: descriptor.format_version,
            trust_domain_id: descriptor.trust_domain_id,
            snapshot_id: descriptor.snapshot_id,
            relationship_snapshot_root: fixed_hash(descriptor.relationship_snapshot_root)?,
            policy_record_id: descriptor.policy_record_id,
            policy_hash: fixed_hash(descriptor.policy_hash)?,
            spicedb_revision: descriptor.spicedb_revision,
            epoch: descriptor.epoch,
            issued_at_unix_seconds: required_timestamp(descriptor.issued_at)?,
            expires_at_unix_seconds: required_timestamp(descriptor.expires_at)?,
            canonicalization_version: descriptor.canonicalization_version,
            signing_key_id: descriptor.signing_key_id,
        },
        SnapshotDescriptorLimits {
            max_string_bytes: config.limits.max_string_bytes,
            max_lifetime_seconds: config.limits.max_artifact_lifetime_seconds,
        },
    )?;
    validate_future_issue_time(
        claims.claims().issued_at_unix_seconds,
        trusted_now_unix_seconds,
        config.limits.max_future_clock_skew_seconds,
    )?;

    let registered_at_unix_seconds = required_timestamp(value.registered_at)?;
    let revoked_at_unix_seconds = optional_timestamp(value.revoked_at)?;
    let revocation_reason =
        optional_string(value.revocation_reason, config.limits.max_string_bytes)?;
    validate_revocation(
        status == RootRecordStatus::Revoked,
        revoked_at_unix_seconds,
        revocation_reason.as_deref(),
    )?;
    if value.registry_generation == 0 {
        return Err(RegistryError::RegistryGenerationInvalid);
    }

    Ok(AcceptedRootRecordV1 {
        claims,
        authority_signature,
        status,
        registered_at_unix_seconds,
        revoked_at_unix_seconds,
        revocation_reason,
        registry_generation: value.registry_generation,
    })
}

fn require_trust_domain(value: &str, config: &RegistryConfig) -> Result<(), RegistryError> {
    validate_identifier(value, config.limits.max_string_bytes)?;
    if value != config.trust_domain_id {
        return Err(RegistryError::TrustDomainMismatch);
    }
    Ok(())
}

fn validate_identifier(value: &str, max_bytes: u32) -> Result<(), RegistryError> {
    if value.len() > max_bytes as usize {
        return Err(RegistryError::StringLimitExceeded);
    }
    if value.is_empty()
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
    {
        return Err(RegistryError::InvalidIdentifier);
    }
    Ok(())
}

fn optional_string(value: String, max_bytes: u32) -> Result<Option<String>, RegistryError> {
    if value.is_empty() {
        return Ok(None);
    }
    validate_identifier(&value, max_bytes)?;
    Ok(Some(value))
}

fn required_timestamp(value: Option<Timestamp>) -> Result<i64, RegistryError> {
    optional_timestamp(value)?.ok_or(RegistryError::MissingField)
}

fn optional_timestamp(value: Option<Timestamp>) -> Result<Option<i64>, RegistryError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.nanos != 0
        || !(PROTOBUF_TIMESTAMP_MIN_SECONDS..=PROTOBUF_TIMESTAMP_MAX_SECONDS)
            .contains(&value.seconds)
    {
        return Err(RegistryError::InvalidTimestamp);
    }
    Ok(Some(value.seconds))
}

fn fixed_hash(value: Vec<u8>) -> Result<[u8; 32], RegistryError> {
    value
        .try_into()
        .map_err(|_| RegistryError::InvalidHashLength)
}

fn fixed_signature(value: Vec<u8>) -> Result<[u8; 64], RegistryError> {
    value
        .try_into()
        .map_err(|_| RegistryError::InvalidSignatureLength)
}

fn validate_revocation(
    is_revoked: bool,
    revoked_at: Option<i64>,
    reason: Option<&str>,
) -> Result<(), RegistryError> {
    match (is_revoked, revoked_at, reason) {
        (false, None, None) | (true, Some(_), _) => Ok(()),
        _ => Err(RegistryError::InvalidValidityWindow),
    }
}

fn validate_future_issue_time(
    issued_at: i64,
    trusted_now: i64,
    max_future_clock_skew_seconds: u64,
) -> Result<(), RegistryError> {
    if issued_at <= trusted_now {
        return Ok(());
    }
    let future_seconds = issued_at
        .checked_sub(trusted_now)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(RegistryError::InvalidTimestamp)?;
    if future_seconds > max_future_clock_skew_seconds {
        return Err(RegistryError::InvalidTimestamp);
    }
    Ok(())
}

const fn timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}
