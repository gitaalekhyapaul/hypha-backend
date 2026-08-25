use std::error::Error;
use std::fmt::{self, Formatter};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tonic::transport::Server;

use super::{
    ReloadingAdmission, StrictVerificationServiceServer, VerificationServiceHandler,
    VerifierdConfig,
};
use crate::worker::{UnixSocketVerificationExecutor, VERIFIER_LAUNCHER_SOCKET_PATH_ENV};

pub async fn run_from_env() -> Result<(), VerifierdRuntimeError> {
    let config = VerifierdConfig::from_env().map_err(|_| VerifierdRuntimeError::Configuration)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| VerifierdRuntimeError::TrustedState)?
        .as_secs();
    let now = i64::try_from(now).map_err(|_| VerifierdRuntimeError::TrustedState)?;
    let mut admission = ReloadingAdmission::new(
        config.registry_path().to_owned(),
        config.registry_config().clone(),
        config.trusted_program_key().clone(),
    );
    admission
        .load_at_startup(now)
        .map_err(|_| VerifierdRuntimeError::TrustedState)?;

    let launcher_path = std::env::var_os(VERIFIER_LAUNCHER_SOCKET_PATH_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(VerifierdRuntimeError::Configuration)?;
    let parent = launcher_path
        .parent()
        .ok_or(VerifierdRuntimeError::Configuration)?;
    let owner_uid = std::fs::symlink_metadata(parent)
        .map_err(|_| VerifierdRuntimeError::WorkerUnavailable)?
        .uid();
    let executor = UnixSocketVerificationExecutor::new(launcher_path, owner_uid)
        .map_err(|_| VerifierdRuntimeError::Configuration)?;
    let service =
        StrictVerificationServiceServer::new(VerificationServiceHandler::new(admission, executor));

    Server::builder()
        .add_service(service)
        .serve(config.bind_address())
        .await
        .map_err(|_| VerifierdRuntimeError::Transport)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifierdRuntimeError {
    Configuration,
    TrustedState,
    WorkerUnavailable,
    Transport,
}

impl fmt::Display for VerifierdRuntimeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("verifierd failed to start")
    }
}

impl Error for VerifierdRuntimeError {}
