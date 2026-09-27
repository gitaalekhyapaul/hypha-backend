//! Offline administration and the isolated runtime registry-writer boundary.

#![forbid(unsafe_code)]

pub mod access_client;
pub mod access_graph;
pub mod agent_api;
mod args;
pub mod capability_client;
mod config;
pub mod dataset_binding;
mod error;
mod io;
mod key;
mod policy;
pub mod policy_activation;
pub mod policy_authority;
pub mod policy_package;
mod policy_publication;
pub mod policy_version;
mod projection;
mod registry;
pub mod request_approval;
pub mod spicedb_candidate;
pub mod verifier_client;
pub mod writer;

pub use error::AdminError;
pub use policy_publication::provisioning as request_provisioning;

use args::Arguments;

/// Runs one offline administration command from process arguments.
pub fn run_from_env() -> Result<(), AdminError> {
    let arguments = Arguments::from_env()?;
    match arguments.command() {
        "allocate-policy-version" => policy_version::allocate_command(&arguments),
        "inspect-key" => key::inspect(&arguments),
        "inspect-policy" => policy::inspect(&arguments),
        "approve-policy" => policy::approve(&arguments),
        "publish-registry" => registry::publish(&arguments),
        "revoke-record" => registry::revoke(&arguments),
        _ => Err(AdminError::Usage(
            "expected allocate-policy-version, inspect-key, inspect-policy, approve-policy, publish-registry, or revoke-record",
        )),
    }
}
