use std::fmt::{self, Display, Formatter};

use raziel_canonical::{CanonicalEncoder, CanonicalError, framed_message};

use crate::RELATIONSHIP_TREE_VERSION;

pub const SNAPSHOT_DESCRIPTOR_FORMAT_VERSION: u32 = 1;
pub const SNAPSHOT_DESCRIPTOR_SIGNATURE_DOMAIN: &[u8] = b"raziel/snapshot-descriptor-signature/v1";

const PROTOBUF_TIMESTAMP_MIN_SECONDS: i64 = -62_135_596_800;
const PROTOBUF_TIMESTAMP_MAX_SECONDS: i64 = 253_402_300_799;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotDescriptorLimits {
    pub max_string_bytes: u32,
    pub max_lifetime_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotDescriptorClaimsV1 {
    pub format_version: u32,
    pub trust_domain_id: String,
    pub snapshot_id: String,
    pub relationship_snapshot_root: [u8; 32],
    pub policy_record_id: String,
    pub policy_hash: [u8; 32],
    pub spicedb_revision: String,
    pub epoch: u64,
    pub issued_at_unix_seconds: i64,
    pub expires_at_unix_seconds: i64,
    pub canonicalization_version: u32,
    pub signing_key_id: String,
}

impl SnapshotDescriptorClaimsV1 {
    pub fn validate(
        &self,
        limits: SnapshotDescriptorLimits,
    ) -> Result<(), SnapshotDescriptorError> {
        if self.format_version != SNAPSHOT_DESCRIPTOR_FORMAT_VERSION {
            return Err(SnapshotDescriptorError::UnsupportedFormatVersion);
        }
        for value in [
            self.trust_domain_id.as_str(),
            self.snapshot_id.as_str(),
            self.policy_record_id.as_str(),
            self.spicedb_revision.as_str(),
            self.signing_key_id.as_str(),
        ] {
            validate_identifier(value, limits.max_string_bytes)?;
        }
        if self.epoch == 0 {
            return Err(SnapshotDescriptorError::InvalidEpoch);
        }
        if self.canonicalization_version != RELATIONSHIP_TREE_VERSION {
            return Err(SnapshotDescriptorError::UnsupportedCanonicalizationVersion);
        }
        validate_timestamp(self.issued_at_unix_seconds)?;
        validate_timestamp(self.expires_at_unix_seconds)?;
        if self.issued_at_unix_seconds >= self.expires_at_unix_seconds {
            return Err(SnapshotDescriptorError::InvalidValidityWindow);
        }

        let lifetime = self
            .expires_at_unix_seconds
            .checked_sub(self.issued_at_unix_seconds)
            .and_then(|seconds| u64::try_from(seconds).ok())
            .ok_or(SnapshotDescriptorError::InvalidValidityWindow)?;
        if lifetime > limits.max_lifetime_seconds {
            return Err(SnapshotDescriptorError::LifetimeExceeded);
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedSnapshotDescriptorClaimsV1(SnapshotDescriptorClaimsV1);

impl ValidatedSnapshotDescriptorClaimsV1 {
    pub fn new(
        claims: SnapshotDescriptorClaimsV1,
        limits: SnapshotDescriptorLimits,
    ) -> Result<Self, SnapshotDescriptorError> {
        claims.validate(limits)?;
        Ok(Self(claims))
    }

    pub const fn claims(&self) -> &SnapshotDescriptorClaimsV1 {
        &self.0
    }

    pub fn into_claims(self) -> SnapshotDescriptorClaimsV1 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotDescriptorError {
    Canonical(CanonicalError),
    UnsupportedFormatVersion,
    InvalidIdentifier,
    StringLimitExceeded,
    InvalidEpoch,
    UnsupportedCanonicalizationVersion,
    TimestampOutOfRange,
    InvalidValidityWindow,
    LifetimeExceeded,
}

impl Display for SnapshotDescriptorError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Canonical(_) => "snapshot descriptor canonical encoding failed",
            Self::UnsupportedFormatVersion => "snapshot descriptor format version is unsupported",
            Self::InvalidIdentifier => "snapshot descriptor identifier is invalid",
            Self::StringLimitExceeded => "snapshot descriptor string exceeds its byte limit",
            Self::InvalidEpoch => "snapshot descriptor epoch must be positive",
            Self::UnsupportedCanonicalizationVersion => {
                "snapshot descriptor canonicalization version is unsupported"
            }
            Self::TimestampOutOfRange => "snapshot descriptor timestamp is outside Protobuf range",
            Self::InvalidValidityWindow => "snapshot descriptor validity window is invalid",
            Self::LifetimeExceeded => "snapshot descriptor lifetime exceeds its limit",
        })
    }
}

impl std::error::Error for SnapshotDescriptorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonical(source) => Some(source),
            _ => None,
        }
    }
}

impl From<CanonicalError> for SnapshotDescriptorError {
    fn from(value: CanonicalError) -> Self {
        Self::Canonical(value)
    }
}

pub fn encode_snapshot_descriptor_claims(
    claims: &SnapshotDescriptorClaimsV1,
) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(claims.format_version);
    encoder.write_str(&claims.trust_domain_id)?;
    encoder.write_str(&claims.snapshot_id)?;
    encoder.write_bytes(&claims.relationship_snapshot_root)?;
    encoder.write_str(&claims.policy_record_id)?;
    encoder.write_bytes(&claims.policy_hash)?;
    encoder.write_str(&claims.spicedb_revision)?;
    encoder.write_u64(claims.epoch);
    encoder.write_i64(claims.issued_at_unix_seconds);
    encoder.write_i64(claims.expires_at_unix_seconds);
    encoder.write_u32(claims.canonicalization_version);
    encoder.write_str(&claims.signing_key_id)?;
    Ok(encoder.finish())
}

pub fn snapshot_descriptor_signature_message(
    claims: &SnapshotDescriptorClaimsV1,
) -> Result<Vec<u8>, CanonicalError> {
    let payload = encode_snapshot_descriptor_claims(claims)?;
    framed_message(SNAPSHOT_DESCRIPTOR_SIGNATURE_DOMAIN, &payload)
}

fn validate_identifier(value: &str, max_string_bytes: u32) -> Result<(), SnapshotDescriptorError> {
    if value.len() > max_string_bytes as usize {
        return Err(SnapshotDescriptorError::StringLimitExceeded);
    }
    if value.is_empty()
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
    {
        return Err(SnapshotDescriptorError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_timestamp(value: i64) -> Result<(), SnapshotDescriptorError> {
    if !(PROTOBUF_TIMESTAMP_MIN_SECONDS..=PROTOBUF_TIMESTAMP_MAX_SECONDS).contains(&value) {
        return Err(SnapshotDescriptorError::TimestampOutOfRange);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SnapshotDescriptorClaimsV1, SnapshotDescriptorError,
        SnapshotDescriptorLimits, encode_snapshot_descriptor_claims,
        snapshot_descriptor_signature_message,
    };
    use crate::RELATIONSHIP_TREE_VERSION;

    fn claims() -> SnapshotDescriptorClaimsV1 {
        SnapshotDescriptorClaimsV1 {
            format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
            trust_domain_id: "raziel-test".into(),
            snapshot_id: "snapshot-1".into(),
            relationship_snapshot_root: [0x11; 32],
            policy_record_id: "policy-1".into(),
            policy_hash: [0x22; 32],
            spicedb_revision: "zed-token-1".into(),
            epoch: 1,
            issued_at_unix_seconds: 1_700_000_000,
            expires_at_unix_seconds: 1_700_000_900,
            canonicalization_version: RELATIONSHIP_TREE_VERSION,
            signing_key_id: "ed25519:key".into(),
        }
    }

    fn limits() -> SnapshotDescriptorLimits {
        SnapshotDescriptorLimits {
            max_string_bytes: 128,
            max_lifetime_seconds: 900,
        }
    }

    #[test]
    fn validates_half_open_time_boundaries() {
        let claims = claims();
        assert_eq!(claims.validate(limits()), Ok(()));

        let mut invalid = claims;
        invalid.issued_at_unix_seconds = invalid.expires_at_unix_seconds;
        assert_eq!(
            invalid.validate(limits()),
            Err(SnapshotDescriptorError::InvalidValidityWindow)
        );
    }

    #[test]
    fn rejects_invalid_fields_and_one_over_limits() {
        let mut value = claims();
        value.epoch = 0;
        assert_eq!(
            value.validate(limits()),
            Err(SnapshotDescriptorError::InvalidEpoch)
        );

        let mut value = claims();
        value.snapshot_id = "x".repeat(129);
        assert_eq!(
            value.validate(limits()),
            Err(SnapshotDescriptorError::StringLimitExceeded)
        );

        let mut value = claims();
        value.expires_at_unix_seconds += 1;
        assert_eq!(
            value.validate(limits()),
            Err(SnapshotDescriptorError::LifetimeExceeded)
        );
    }

    #[test]
    fn canonical_payload_and_signature_frame_are_deterministic() {
        let claims = claims();
        let payload = encode_snapshot_descriptor_claims(&claims).unwrap();
        assert_eq!(&payload[..4], &1_u32.to_be_bytes());
        assert_eq!(
            snapshot_descriptor_signature_message(&claims)
                .unwrap()
                .windows(payload.len())
                .last(),
            Some(payload.as_slice())
        );
    }
}
