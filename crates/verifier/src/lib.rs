//! Independent authority and registry verification for Raziel.

#![forbid(unsafe_code)]

mod config;
mod error;
mod ingress;
mod loader;
mod model;
mod registry;

pub use config::{
    DEFAULT_MAX_ARTIFACT_LIFETIME_SECONDS, DEFAULT_MAX_REGISTRY_RECORD_BYTES,
    DEFAULT_MAX_REGISTRY_STRING_BYTES, default_policy_validation_ceilings, default_registry_config,
};
pub use error::RegistryError;
pub use ingress::{
    REGISTRY_DESCRIPTOR_SHA256, decode_accepted_root_record_strict,
    decode_authority_key_record_strict, decode_policy_record_strict,
    decode_registry_manifest_strict,
};
pub use loader::RegistryLoader;
pub use model::{
    AcceptedRootRecordV1, PolicyRecordStatus, PolicyRegistryRecordV1, RegistryConfig,
    RegistryLimits, RootRecordStatus, authority_key_from_proto, authority_key_to_proto,
    policy_record_from_proto, policy_record_to_proto, root_record_from_proto, root_record_to_proto,
};
pub use registry::{InstallOutcome, LocalRegistry, RegistryGenerationV1, ResolvedRoot};
