//! Bounded transport client for the independent verification service.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() {
    if let Err(error) = raziel_admin::verifier_client::run_from_env().await {
        eprintln!("raziel-verifier-client: {error}");
        std::process::exit(2);
    }
}
