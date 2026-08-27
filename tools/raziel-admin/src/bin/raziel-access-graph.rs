//! Owner-only access-graph publication service.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), raziel_admin::access_graph::AccessGraphServerError> {
    raziel_admin::access_graph::run_from_env().await
}
