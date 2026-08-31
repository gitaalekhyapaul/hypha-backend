//! Bounded transport client for the asynchronous capability service.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() {
    if let Err(error) = raziel_admin::capability_client::run_from_env().await {
        eprintln!("raziel-capability-client: {error}");
        std::process::exit(raziel_admin::capability_client::exit_code(error));
    }
}
