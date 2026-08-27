//! Capability-only Raziel API for the owner console.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), razield::service::ServiceRuntimeError> {
    razield::service::run_capability_from_env().await
}
