use ed25519_dalek::{Signature, VerifyingKey};
use raziel_canonical::{ValidatedPolicyApprovalClaimsV1, policy_approval_signature_message};
use raziel_snapshot::{ValidatedSnapshotDescriptorClaimsV1, snapshot_descriptor_signature_message};
use sha2::{Digest, Sha256};

use crate::AuthorityError;

pub const AUTHORITY_KEY_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorityRole {
    PolicyApproval,
    Snapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorityKeyStatus {
    Active,
    Revoked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityKeyRecordV1 {
    pub format_version: u32,
    pub trust_domain_id: String,
    pub key_id: String,
    pub role: AuthorityRole,
    pub public_key: [u8; 32],
    pub status: AuthorityKeyStatus,
    pub valid_from_unix_seconds: i64,
    pub valid_until_unix_seconds: i64,
    pub revoked_at_unix_seconds: Option<i64>,
    pub revocation_reason: Option<String>,
    pub registry_generation: u64,
}

#[derive(Clone)]
pub struct TrustedAuthorityKey {
    record: AuthorityKeyRecordV1,
    verifying_key: VerifyingKey,
}

impl TrustedAuthorityKey {
    pub fn from_record(
        record: AuthorityKeyRecordV1,
        expected_trust_domain_id: &str,
    ) -> Result<Self, AuthorityError> {
        if record.format_version != AUTHORITY_KEY_FORMAT_VERSION {
            return Err(AuthorityError::UnsupportedFormatVersion);
        }
        if record.trust_domain_id != expected_trust_domain_id
            || !is_printable_identifier(&record.trust_domain_id)
        {
            return Err(AuthorityError::InvalidTrustDomain);
        }
        if record.valid_from_unix_seconds >= record.valid_until_unix_seconds {
            return Err(AuthorityError::InvalidValidityWindow);
        }
        if record.registry_generation == 0 {
            return Err(AuthorityError::InvalidRegistryGeneration);
        }
        match (record.status, record.revoked_at_unix_seconds) {
            (AuthorityKeyStatus::Active, None) => {
                if record.revocation_reason.is_some() {
                    return Err(AuthorityError::InvalidRevocation);
                }
            }
            (AuthorityKeyStatus::Revoked, Some(_)) => {
                if record
                    .revocation_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.is_empty() && !is_printable_identifier(reason))
                {
                    return Err(AuthorityError::InvalidRevocation);
                }
            }
            _ => return Err(AuthorityError::InvalidRevocation),
        }

        let verifying_key = VerifyingKey::from_bytes(&record.public_key)
            .map_err(|_| AuthorityError::InvalidPublicKey)?;
        if verifying_key.is_weak() {
            return Err(AuthorityError::WeakPublicKey);
        }
        if derive_key_id(&record.public_key) != record.key_id {
            return Err(AuthorityError::InvalidKeyId);
        }

        Ok(Self {
            record,
            verifying_key,
        })
    }

    pub const fn role(&self) -> AuthorityRole {
        self.record.role
    }

    pub fn key_id(&self) -> &str {
        &self.record.key_id
    }

    pub fn record(&self) -> &AuthorityKeyRecordV1 {
        &self.record
    }

    pub fn verify_policy_approval_signature(
        &self,
        claims: &ValidatedPolicyApprovalClaimsV1,
        signature_bytes: &[u8],
    ) -> Result<(), AuthorityError> {
        let claims = claims.claims();
        if claims.signing_key_id != self.record.key_id {
            return Err(AuthorityError::InvalidKeyId);
        }
        let message = policy_approval_signature_message(claims)?;
        self.verify_signature(AuthorityRole::PolicyApproval, &message, signature_bytes)
    }

    pub fn verify_policy_approval(
        &self,
        trusted_now_unix_seconds: i64,
        claims: &ValidatedPolicyApprovalClaimsV1,
        signature_bytes: &[u8],
    ) -> Result<(), AuthorityError> {
        self.validate_current_status(trusted_now_unix_seconds)?;
        self.verify_policy_approval_signature(claims, signature_bytes)
    }

    pub fn verify_snapshot_descriptor_signature(
        &self,
        claims: &ValidatedSnapshotDescriptorClaimsV1,
        signature_bytes: &[u8],
    ) -> Result<(), AuthorityError> {
        let claims = claims.claims();
        if claims.signing_key_id != self.record.key_id {
            return Err(AuthorityError::InvalidKeyId);
        }
        let message = snapshot_descriptor_signature_message(claims)?;
        self.verify_signature(AuthorityRole::Snapshot, &message, signature_bytes)
    }

    pub fn verify_snapshot_descriptor(
        &self,
        trusted_now_unix_seconds: i64,
        claims: &ValidatedSnapshotDescriptorClaimsV1,
        signature_bytes: &[u8],
    ) -> Result<(), AuthorityError> {
        self.validate_current_status(trusted_now_unix_seconds)?;
        self.verify_snapshot_descriptor_signature(claims, signature_bytes)
    }

    fn validate_current_status(&self, trusted_now_unix_seconds: i64) -> Result<(), AuthorityError> {
        match self.record.status {
            AuthorityKeyStatus::Active => {}
            AuthorityKeyStatus::Revoked => return Err(AuthorityError::RevokedKey),
        }
        if trusted_now_unix_seconds < self.record.valid_from_unix_seconds
            || trusted_now_unix_seconds >= self.record.valid_until_unix_seconds
        {
            return Err(AuthorityError::InactiveKey);
        }
        Ok(())
    }

    fn verify_signature(
        &self,
        expected_role: AuthorityRole,
        message: &[u8],
        signature_bytes: &[u8],
    ) -> Result<(), AuthorityError> {
        if self.record.role != expected_role {
            return Err(AuthorityError::WrongRole);
        }
        let signature_bytes: &[u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| AuthorityError::InvalidSignatureLength)?;
        let signature = Signature::from_bytes(signature_bytes);
        self.verifying_key
            .verify_strict(message, &signature)
            .map_err(|_| AuthorityError::InvalidSignature)
    }
}

pub fn derive_key_id(public_key: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = derive_ed25519_key_thumbprint(public_key);
    let mut key_id = String::with_capacity(72);
    key_id.push_str("ed25519:");
    for byte in digest {
        key_id.push(char::from(HEX[usize::from(byte >> 4)]));
        key_id.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    key_id
}

/// Derives the version 1 identity from canonical raw Ed25519 public-key bytes.
pub fn derive_ed25519_key_thumbprint(public_key: &[u8; 32]) -> [u8; 32] {
    Sha256::digest(public_key).into()
}

fn is_printable_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| !byte.is_ascii_control() && byte != 0x7f)
}

#[cfg(test)]
mod tests {
    use super::{derive_ed25519_key_thumbprint, derive_key_id};

    #[test]
    fn derived_key_id_has_fixed_lowercase_form() {
        let key_id = derive_key_id(&[0; 32]);
        assert_eq!(key_id.len(), 72);
        assert_eq!(
            key_id,
            "ed25519:66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925"
        );
    }

    #[test]
    fn agent_thumbprint_matches_adr_0014_rfc_8032_vector() {
        let public_key = [
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ];
        let expected = [
            0x21, 0xfe, 0x31, 0xdf, 0xa1, 0x54, 0xa2, 0x61, 0x62, 0x6b, 0xf8, 0x54, 0x04, 0x6f,
            0xd2, 0x27, 0x1b, 0x7b, 0xed, 0x4b, 0x6a, 0xbe, 0x45, 0xaa, 0x58, 0x87, 0x7e, 0xf4,
            0x7f, 0x97, 0x21, 0xb9,
        ];

        assert_eq!(derive_ed25519_key_thumbprint(&public_key), expected);
    }
}
