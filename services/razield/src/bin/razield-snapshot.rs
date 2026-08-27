//! Snapshot-only Raziel service for the owner console.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), razield::service::ServiceRuntimeError> {
    razield::service::run_snapshot_from_env().await
}
