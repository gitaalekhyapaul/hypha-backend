use std::collections::BTreeMap;

use raziel_snapshot::{
    SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SnapshotDescriptorClaimsV1, SnapshotDescriptorLimits,
    ValidatedSnapshotDescriptorClaimsV1,
};

use crate::{AuthorityError, AuthoritySigner, SnapshotRole};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotDescriptorDraftV1 {
    pub snapshot_id: String,
    pub relationship_snapshot_root: [u8; 32],
    pub policy_record_id: String,
    pub policy_hash: [u8; 32],
    pub spicedb_revision: String,
    pub issued_at_unix_seconds: i64,
    pub expires_at_unix_seconds: i64,
    pub canonicalization_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedSnapshotDescriptorV1 {
    pub claims: ValidatedSnapshotDescriptorClaimsV1,
    pub authority_signature: [u8; 64],
}

pub struct SnapshotAuthorityPublisher {
    signer: AuthoritySigner<SnapshotRole>,
    trust_domain_id: String,
    high_watermarks: BTreeMap<[u8; 32], u64>,
}

impl SnapshotAuthorityPublisher {
    pub fn new(
        signer: AuthoritySigner<SnapshotRole>,
        trust_domain_id: String,
        trusted_high_watermarks: impl IntoIterator<Item = ([u8; 32], u64)>,
    ) -> Result<Self, AuthorityError> {
        if !is_printable_identifier(&trust_domain_id) {
            return Err(AuthorityError::InvalidTrustDomain);
        }
        let mut high_watermarks = BTreeMap::new();
        for (policy_hash, epoch) in trusted_high_watermarks {
            if high_watermarks.insert(policy_hash, epoch).is_some() {
                return Err(AuthorityError::SnapshotEpochStateConflict);
            }
        }
        Ok(Self {
            signer,
            trust_domain_id,
            high_watermarks,
        })
    }

    pub fn high_watermark(&self, policy_hash: &[u8; 32]) -> u64 {
        self.high_watermarks.get(policy_hash).copied().unwrap_or(0)
    }

    pub fn sign_next(
        &mut self,
        expected_previous_epoch: u64,
        draft: SnapshotDescriptorDraftV1,
        limits: SnapshotDescriptorLimits,
    ) -> Result<SignedSnapshotDescriptorV1, AuthorityError> {
        let current = self.high_watermark(&draft.policy_hash);
        if expected_previous_epoch != current {
            return Err(AuthorityError::SnapshotEpochMismatch);
        }
        let epoch = current
            .checked_add(1)
            .ok_or(AuthorityError::SnapshotEpochOverflow)?;
        let policy_hash = draft.policy_hash;
        let claims = ValidatedSnapshotDescriptorClaimsV1::new(
            SnapshotDescriptorClaimsV1 {
                format_version: SNAPSHOT_DESCRIPTOR_FORMAT_VERSION,
                trust_domain_id: self.trust_domain_id.clone(),
                snapshot_id: draft.snapshot_id,
                relationship_snapshot_root: draft.relationship_snapshot_root,
                policy_record_id: draft.policy_record_id,
                policy_hash,
                spicedb_revision: draft.spicedb_revision,
                epoch,
                issued_at_unix_seconds: draft.issued_at_unix_seconds,
                expires_at_unix_seconds: draft.expires_at_unix_seconds,
                canonicalization_version: draft.canonicalization_version,
                signing_key_id: self.signer.key_id().into(),
            },
            limits,
        )?;
        let authority_signature = self.signer.sign_snapshot_descriptor(&claims)?;
        self.high_watermarks.insert(policy_hash, epoch);
        Ok(SignedSnapshotDescriptorV1 {
            claims,
            authority_signature,
        })
    }
}

fn is_printable_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
}
