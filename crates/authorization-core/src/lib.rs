//! Shared proof statement for the Raziel authorization host and SP1 guest.

#![forbid(unsafe_code)]

mod error;
mod limits;
mod model;
mod public_values;
mod statement;

pub use error::AuthorizationError;
pub use limits::{MAX_CANONICAL_POLICY_BYTES, TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS};
pub use model::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AUTHORIZATION_PUBLIC_VALUES_VERSION,
    AuthorizationGuestInputV1, AuthorizationPublicValuesV1, AuthorizationResult,
    CapabilityRequestBindingV1,
};
pub use public_values::{
    AUTHORIZATION_PUBLIC_VALUES_DOMAIN, decode_public_values, encode_public_values,
};
pub use statement::{
    CAPABILITY_REQUEST_DOMAIN, CAPABILITY_SCOPE_DOMAIN, ISSUANCE_NULLIFIER_DOMAIN, agent_subject,
    authorize, capability_request_commitment, capability_scope_hash, issuance_nullifier,
};
