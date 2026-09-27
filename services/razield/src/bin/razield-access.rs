//! Owner-network access inbox. This process has no proof worker.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), razield::access_request::AccessError> {
    razield::access_request::run_from_env().await
}
