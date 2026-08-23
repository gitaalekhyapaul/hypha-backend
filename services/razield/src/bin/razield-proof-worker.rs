//! Production BullMQ proof-supervisor process.

#![forbid(unsafe_code)]

use razield::worker::{ProofSupervisor, SupervisorConfig, SupervisorRuntimeError};

#[tokio::main]
async fn main() -> Result<(), SupervisorRuntimeError> {
    let config = SupervisorConfig::from_env().map_err(|_| SupervisorRuntimeError::Configuration)?;
    let supervisor = ProofSupervisor::start_production(config).await?;
    tokio::signal::ctrl_c()
        .await
        .map_err(|_| SupervisorRuntimeError::WorkerLifecycleFailed)?;
    supervisor.close().await
}
