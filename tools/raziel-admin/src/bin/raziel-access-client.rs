#[tokio::main]
async fn main() {
    if let Err(error) = raziel_admin::access_client::run_from_env().await {
        std::process::exit(raziel_admin::access_client::exit_code(error));
    }
}
