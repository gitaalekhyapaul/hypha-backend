fn main() -> Result<(), verifierd::worker::VerifierSandboxLauncherError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| verifierd::worker::VerifierSandboxLauncherError::Configuration)?;
    runtime.block_on(verifierd::worker::run_launcher_from_env())
}
