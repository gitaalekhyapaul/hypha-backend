//! Trusted host assembly of one authorization guest input.

mod error;

#[cfg(test)]
mod tests;

use std::fmt::{self, Debug, Formatter};

use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AuthorizationGuestInputV1, AuthorizationPublicValuesV1,
    AuthorizationResult, CapabilityRequestBindingV1, TRUSTED_SNAPSHOT_LIMITS,
    authorization_evaluation_request, authorize,
};
use raziel_policy_ir::ValidatedPolicyProgram;
use raziel_snapshot::{Digest, RelationshipRecord};
use raziel_verifier::{LocalRegistry, ResolvedRoot};
use raziel_witness::build_positive_witness;

pub use error::InputAssemblyError;

use crate::admission::{LOCAL_PLONK_ADMISSION_WINDOW_SECONDS, MAX_PROTOBUF_TIMESTAMP_SECONDS};
use crate::private_envelope::PrivateCapabilityEnvelope;

/// Private snapshot material selected inside the data-owner proof boundary.
pub struct SelectedPrivateSnapshot {
    snapshot_id: String,
    relationships: Vec<RelationshipRecord>,
    snapshot_secret_salt: Digest,
}

impl SelectedPrivateSnapshot {
    /// Takes ownership of the exact, strictly ordered relationships for one snapshot.
    pub const fn new(
        snapshot_id: String,
        relationships: Vec<RelationshipRecord>,
        snapshot_secret_salt: Digest,
    ) -> Self {
        Self {
            snapshot_id,
            relationships,
            snapshot_secret_salt,
        }
    }
}

impl Debug for SelectedPrivateSnapshot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedPrivateSnapshot")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Guest input that has passed the shared host authorization statement.
pub struct ValidatedAuthorizationInput {
    guest_input: AuthorizationGuestInputV1,
    expected_authorization: AuthorizationResult,
}

impl ValidatedAuthorizationInput {
    pub const fn guest_input(&self) -> &AuthorizationGuestInputV1 {
        &self.guest_input
    }

    pub const fn expected_public_values(&self) -> &AuthorizationPublicValuesV1 {
        &self.expected_authorization.public_values
    }

    pub fn expected_public_values_bytes(&self) -> &[u8] {
        &self.expected_authorization.public_values_bytes
    }

    pub fn into_parts(self) -> (AuthorizationGuestInputV1, AuthorizationResult) {
        (self.guest_input, self.expected_authorization)
    }
}

impl Debug for ValidatedAuthorizationInput {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedAuthorizationInput")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Resolves trust records and assembles one host-validated guest input.
pub fn assemble_authorization_input(
    registry: &mut LocalRegistry,
    envelope: &PrivateCapabilityEnvelope,
    selected_snapshot: SelectedPrivateSnapshot,
    issued_at_unix_seconds: i64,
) -> Result<ValidatedAuthorizationInput, InputAssemblyError> {
    let request = envelope.request();
    let expires_at_unix_seconds = proof_window_end(request, issued_at_unix_seconds)?;
    let resolved = registry
        .resolve_snapshot_exact(&request.snapshot_id, issued_at_unix_seconds)
        .map_err(InputAssemblyError::Registry)?;

    assemble_from_trusted_selection(
        request,
        envelope.capability_request_commitment(),
        TrustedAuthorizationSelection::from(resolved),
        selected_snapshot,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    )
}

fn assemble_from_trusted_selection(
    request: &CapabilityRequestBindingV1,
    expected_request_commitment: &Digest,
    trusted: TrustedAuthorizationSelection,
    selected_snapshot: SelectedPrivateSnapshot,
    issued_at_unix_seconds: i64,
    expires_at_unix_seconds: i64,
) -> Result<ValidatedAuthorizationInput, InputAssemblyError> {
    if trusted.snapshot_id != request.snapshot_id
        || trusted.snapshot_policy_record_id != trusted.policy_record_id
        || trusted.snapshot_policy_hash != trusted.policy_hash
    {
        return Err(InputAssemblyError::TrustedSelectionMismatch);
    }
    if selected_snapshot.snapshot_id != request.snapshot_id {
        return Err(InputAssemblyError::SnapshotSelectionMismatch);
    }
    if trusted.policy_valid_until_unix_seconds < expires_at_unix_seconds
        || trusted.snapshot_expires_at_unix_seconds < expires_at_unix_seconds
    {
        return Err(InputAssemblyError::InsufficientValidityWindow);
    }

    let evaluation_request = authorization_evaluation_request(
        &trusted.policy,
        request,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    )
    .map_err(InputAssemblyError::RequestMapping)?;
    let relationship_witness_set = build_positive_witness(
        &trusted.policy,
        &evaluation_request,
        selected_snapshot.relationships,
        selected_snapshot.snapshot_secret_salt,
        trusted.relationship_snapshot_root,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .map_err(InputAssemblyError::Witness)?;

    let guest_input = AuthorizationGuestInputV1 {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        policy: trusted.policy.program().clone(),
        expected_policy_hash: trusted.policy_hash,
        relationship_snapshot_root: trusted.relationship_snapshot_root,
        relationship_witness_set,
        request: request.clone(),
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    };
    let expected_authorization =
        authorize(&guest_input).map_err(InputAssemblyError::HostValidation)?;
    if &expected_authorization
        .public_values
        .capability_request_commitment
        != expected_request_commitment
    {
        return Err(InputAssemblyError::EnvelopeCommitmentMismatch);
    }

    Ok(ValidatedAuthorizationInput {
        guest_input,
        expected_authorization,
    })
}

struct TrustedAuthorizationSelection {
    policy: ValidatedPolicyProgram,
    policy_hash: Digest,
    policy_record_id: String,
    policy_valid_until_unix_seconds: i64,
    snapshot_id: String,
    relationship_snapshot_root: Digest,
    snapshot_policy_record_id: String,
    snapshot_policy_hash: Digest,
    snapshot_expires_at_unix_seconds: i64,
}

impl From<ResolvedRoot> for TrustedAuthorizationSelection {
    fn from(resolved: ResolvedRoot) -> Self {
        let root_claims = resolved.root_record.claims.claims();
        let policy_claims = resolved.policy_record.claims.claims();
        Self {
            policy: resolved.policy_record.policy.clone(),
            policy_hash: policy_claims.policy_hash,
            policy_record_id: policy_claims.record_id.clone(),
            policy_valid_until_unix_seconds: policy_claims.valid_until_unix_seconds,
            snapshot_id: root_claims.snapshot_id.clone(),
            relationship_snapshot_root: root_claims.relationship_snapshot_root,
            snapshot_policy_record_id: root_claims.policy_record_id.clone(),
            snapshot_policy_hash: root_claims.policy_hash,
            snapshot_expires_at_unix_seconds: root_claims.expires_at_unix_seconds,
        }
    }
}

fn proof_window_end(
    request: &CapabilityRequestBindingV1,
    issued_at_unix_seconds: i64,
) -> Result<i64, InputAssemblyError> {
    if !(0..=MAX_PROTOBUF_TIMESTAMP_SECONDS).contains(&issued_at_unix_seconds) {
        return Err(InputAssemblyError::InvalidWorkerStart);
    }
    let expires_at_unix_seconds = issued_at_unix_seconds
        .checked_add(LOCAL_PLONK_ADMISSION_WINDOW_SECONDS)
        .filter(|expires_at| *expires_at <= MAX_PROTOBUF_TIMESTAMP_SECONDS)
        .ok_or(InputAssemblyError::InvalidWorkerStart)?;
    if request.requested_expires_at_unix_seconds < expires_at_unix_seconds {
        return Err(InputAssemblyError::InsufficientValidityWindow);
    }
    Ok(expires_at_unix_seconds)
}
