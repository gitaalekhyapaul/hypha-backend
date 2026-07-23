use std::fmt::{self, Display, Formatter};

use crate::{CanonicalEncoder, CanonicalError, framed_message};

pub const AUTHORITY_CLAIMS_FORMAT_VERSION: u32 = 1;
pub const POLICY_APPROVAL_SIGNATURE_DOMAIN: &[u8] = b"raziel/policy-approval-signature/v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyApprovalClaimsV1 {
    pub format_version: u32,
    pub trust_domain_id: String,
    pub record_id: String,
    pub policy_name: String,
    pub policy_version: String,
    pub approval_epoch: u64,
    pub policy_hash: [u8; 32],
    pub valid_from_unix_seconds: i64,
    pub valid_until_unix_seconds: i64,
    pub approved_by: String,
    pub signing_key_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyApprovalLimits {
    pub max_string_bytes: u32,
    pub max_lifetime_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedPolicyApprovalClaimsV1(PolicyApprovalClaimsV1);

impl ValidatedPolicyApprovalClaimsV1 {
    pub fn new(
        claims: PolicyApprovalClaimsV1,
        limits: PolicyApprovalLimits,
    ) -> Result<Self, PolicyApprovalValidationError> {
        claims.validate(limits)?;
        Ok(Self(claims))
    }

    pub const fn claims(&self) -> &PolicyApprovalClaimsV1 {
        &self.0
    }

    pub fn into_claims(self) -> PolicyApprovalClaimsV1 {
        self.0
    }
}

impl PolicyApprovalClaimsV1 {
    pub fn validate(
        &self,
        limits: PolicyApprovalLimits,
    ) -> Result<(), PolicyApprovalValidationError> {
        if self.format_version != AUTHORITY_CLAIMS_FORMAT_VERSION {
            return Err(PolicyApprovalValidationError::UnsupportedFormatVersion);
        }
        for value in [
            self.trust_domain_id.as_str(),
            self.record_id.as_str(),
            self.policy_name.as_str(),
            self.policy_version.as_str(),
            self.approved_by.as_str(),
            self.signing_key_id.as_str(),
        ] {
            validate_identifier(value, limits.max_string_bytes)?;
        }
        if self.approval_epoch == 0 {
            return Err(PolicyApprovalValidationError::InvalidEpoch);
        }
        validate_timestamp(self.valid_from_unix_seconds)?;
        validate_timestamp(self.valid_until_unix_seconds)?;
        if self.valid_from_unix_seconds >= self.valid_until_unix_seconds {
            return Err(PolicyApprovalValidationError::InvalidValidityWindow);
        }
        let lifetime = self
            .valid_until_unix_seconds
            .checked_sub(self.valid_from_unix_seconds)
            .and_then(|seconds| u64::try_from(seconds).ok())
            .ok_or(PolicyApprovalValidationError::InvalidValidityWindow)?;
        if lifetime > limits.max_lifetime_seconds {
            return Err(PolicyApprovalValidationError::LifetimeExceeded);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyApprovalValidationError {
    UnsupportedFormatVersion,
    InvalidIdentifier,
    StringLimitExceeded,
    InvalidEpoch,
    TimestampOutOfRange,
    InvalidValidityWindow,
    LifetimeExceeded,
}

impl Display for PolicyApprovalValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedFormatVersion => "policy approval format version is unsupported",
            Self::InvalidIdentifier => "policy approval identifier is invalid",
            Self::StringLimitExceeded => "policy approval string exceeds its byte limit",
            Self::InvalidEpoch => "policy approval epoch must be positive",
            Self::TimestampOutOfRange => "policy approval timestamp is outside Protobuf range",
            Self::InvalidValidityWindow => "policy approval validity window is invalid",
            Self::LifetimeExceeded => "policy approval lifetime exceeds its limit",
        })
    }
}

impl std::error::Error for PolicyApprovalValidationError {}

pub fn encode_policy_approval_claims(
    claims: &PolicyApprovalClaimsV1,
) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(claims.format_version);
    encoder.write_str(&claims.trust_domain_id)?;
    encoder.write_str(&claims.record_id)?;
    encoder.write_str(&claims.policy_name)?;
    encoder.write_str(&claims.policy_version)?;
    encoder.write_u64(claims.approval_epoch);
    encoder.write_bytes(&claims.policy_hash)?;
    encoder.write_i64(claims.valid_from_unix_seconds);
    encoder.write_i64(claims.valid_until_unix_seconds);
    encoder.write_str(&claims.approved_by)?;
    encoder.write_str(&claims.signing_key_id)?;
    Ok(encoder.finish())
}

pub fn policy_approval_signature_message(
    claims: &PolicyApprovalClaimsV1,
) -> Result<Vec<u8>, CanonicalError> {
    let payload = encode_policy_approval_claims(claims)?;
    framed_message(POLICY_APPROVAL_SIGNATURE_DOMAIN, &payload)
}

fn validate_identifier(
    value: &str,
    max_string_bytes: u32,
) -> Result<(), PolicyApprovalValidationError> {
    if value.len() > max_string_bytes as usize {
        return Err(PolicyApprovalValidationError::StringLimitExceeded);
    }
    if value.is_empty()
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
    {
        return Err(PolicyApprovalValidationError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_timestamp(value: i64) -> Result<(), PolicyApprovalValidationError> {
    const MIN_SECONDS: i64 = -62_135_596_800;
    const MAX_SECONDS: i64 = 253_402_300_799;

    if !(MIN_SECONDS..=MAX_SECONDS).contains(&value) {
        return Err(PolicyApprovalValidationError::TimestampOutOfRange);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AUTHORITY_CLAIMS_FORMAT_VERSION, POLICY_APPROVAL_SIGNATURE_DOMAIN, PolicyApprovalClaimsV1,
        encode_policy_approval_claims, policy_approval_signature_message,
    };

    #[test]
    fn policy_approval_field_order_and_domain_are_fixed() {
        let claims = PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: "td".into(),
            record_id: "r".into(),
            policy_name: "p".into(),
            policy_version: "v".into(),
            approval_epoch: 1,
            policy_hash: [0x11; 32],
            valid_from_unix_seconds: -1,
            valid_until_unix_seconds: 1,
            approved_by: "a".into(),
            signing_key_id: "k".into(),
        };

        let payload = encode_policy_approval_claims(&claims).unwrap();
        assert_eq!(&payload[..4], &1_u32.to_be_bytes());
        assert_eq!(&payload[4..10], &[0, 0, 0, 2, b't', b'd']);
        assert_eq!(
            policy_approval_signature_message(&claims).unwrap(),
            [
                &(POLICY_APPROVAL_SIGNATURE_DOMAIN.len() as u32).to_be_bytes(),
                POLICY_APPROVAL_SIGNATURE_DOMAIN,
                &(payload.len() as u32).to_be_bytes(),
                payload.as_slice(),
            ]
            .concat()
        );
    }
}
