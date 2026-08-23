fn main() -> Result<(), razield::one_shot::ProofSandboxLauncherError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| razield::one_shot::ProofSandboxLauncherError::Configuration)?;
    runtime.block_on(razield::one_shot::run_launcher_from_env())
}
