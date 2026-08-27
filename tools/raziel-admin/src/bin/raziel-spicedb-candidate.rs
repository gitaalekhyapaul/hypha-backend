#[tokio::main]
async fn main() {
    if let Err(error) = raziel_admin::spicedb_candidate::run_from_env().await {
        eprintln!("raziel-spicedb-candidate: {error}");
        std::process::exit(2);
    }
}
