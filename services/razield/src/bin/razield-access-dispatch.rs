//! Serial access workflow. Proof execution remains in the isolated supervisor.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), razield::access_request::AccessError> {
    razield::access_dispatch::run_from_env().await
}
