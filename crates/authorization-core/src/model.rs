use std::fmt::{self, Debug, Formatter};

use raziel_policy_ir::{ContextEntry, PolicyProgram};
use raziel_snapshot::{Digest, RelationshipWitnessSetV1};
use serde::{Deserialize, Serialize};

pub const AUTHORIZATION_GUEST_INPUT_VERSION: u32 = 1;
pub const AUTHORIZATION_PUBLIC_VALUES_VERSION: u32 = 1;

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
pub struct AuthorizationGuestInputV1 {
    pub format_version: u32,
    pub policy: PolicyProgram,
    pub expected_policy_hash: Digest,
    pub relationship_snapshot_root: Digest,
    pub relationship_witness_set: RelationshipWitnessSetV1,
    pub request: CapabilityRequestBindingV1,
    pub issued_at_unix_seconds: i64,
    pub expires_at_unix_seconds: i64,
}

impl Debug for AuthorizationGuestInputV1 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizationGuestInputV1")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
pub struct CapabilityRequestBindingV1 {
    pub format_version: u32,
    pub agent_key_thumbprint: Digest,
    pub task_id: String,
    pub purpose: String,
    pub action: String,
    pub resource_type: String,
    pub resource_scope: String,
    pub requested_expires_at_unix_seconds: i64,
    pub issuance_nonce: Digest,
    pub snapshot_id: String,
    pub request_context: Vec<ContextEntry>,
}

impl Debug for CapabilityRequestBindingV1 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityRequestBindingV1")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationPublicValuesV1 {
    pub format_version: u32,
    pub policy_hash: Digest,
    pub relationship_snapshot_root: Digest,
    pub capability_request_commitment: Digest,
    pub capability_scope_hash: Digest,
    pub issued_at_unix_seconds: i64,
    pub expires_at_unix_seconds: i64,
    pub issuance_nullifier: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationResult {
    pub public_values: AuthorizationPublicValuesV1,
    pub public_values_bytes: Vec<u8>,
}
