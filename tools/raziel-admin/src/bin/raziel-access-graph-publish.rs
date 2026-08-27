//! Safe client for the owner-only access-graph publication service.

#![forbid(unsafe_code)]

fn main() -> Result<(), raziel_admin::access_graph::AccessGraphClientError> {
    raziel_admin::access_graph::run_client_from_env()
}
