//! Offline administration and the isolated runtime registry-writer boundary.

#![forbid(unsafe_code)]

mod args;
mod config;
mod error;
mod io;
mod key;
mod policy;
mod projection;
mod registry;
pub mod writer;

pub use error::AdminError;

use args::Arguments;

/// Runs one offline administration command from process arguments.
pub fn run_from_env() -> Result<(), AdminError> {
    let arguments = Arguments::from_env()?;
    match arguments.command() {
        "inspect-key" => key::inspect(&arguments),
        "approve-policy" => policy::approve(&arguments),
        "publish-registry" => registry::publish(&arguments),
        "revoke-record" => registry::revoke(&arguments),
        _ => Err(AdminError::Usage(
            "expected inspect-key, approve-policy, publish-registry, or revoke-record",
        )),
    }
}
