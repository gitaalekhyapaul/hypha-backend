//! Offline policy approval and local registry administration.

#![forbid(unsafe_code)]

mod args;
mod config;
mod error;
mod io;
mod key;
mod policy;
mod projection;
mod registry;

use args::Arguments;
use error::AdminError;

fn main() {
    if let Err(error) = run() {
        eprintln!("raziel-admin: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), AdminError> {
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
