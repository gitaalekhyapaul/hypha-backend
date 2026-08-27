use std::error::Error;
use std::fmt;
#[cfg(feature = "capability-service")]
use std::future::Future;
use std::net::SocketAddr;
#[cfg(feature = "capability-service")]
use std::time::SystemTime;

use tonic::transport::Server;

#[cfg(feature = "capability-service")]
use super::{
    ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError, SystemClock, bounded_capability_service,
};
use super::{
    SnapshotCoordinator, SnapshotOperationError, SnapshotServiceConfig, SnapshotServiceConfigError,
    bounded_snapshot_service,
};
#[cfg(feature = "capability-service")]
use crate::private_envelope::{PrivateEnvelopeError, PrivateEnvelopeStore};
#[cfg(feature = "capability-service")]
use crate::publication::{
    CapabilityJobPublisher, CapabilityPublicationError, StartupReconciliationError,
};
#[cfg(feature = "capability-service")]
use crate::queue::{QueueConfig, QueueConfigError};

/// Runs the owner service from its explicit environment contract.
#[cfg(feature = "capability-service")]
pub async fn run_from_env() -> Result<(), ServiceRuntimeError> {
    let config = ServiceConfig::from_env()?;
    let snapshot_config = SnapshotServiceConfig::from_env()?;
    run_all_until_shutdown(config, snapshot_config, shutdown_signal()).await
}

/// Runs only the asynchronous capability lifecycle for the owner console.
#[cfg(feature = "capability-service")]
pub async fn run_capability_from_env() -> Result<(), ServiceRuntimeError> {
    let config = ServiceConfig::from_env()?;
    run_until_shutdown(config, shutdown_signal()).await
}

/// Runs only the snapshot lifecycle service for the owner console.
pub async fn run_snapshot_from_env() -> Result<(), ServiceRuntimeError> {
    let snapshot_config = SnapshotServiceConfig::from_env()?;
    let bind_address = std::env::var("RAZIEL_BIND_ADDRESS")
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .ok_or(ServiceRuntimeError::BindAddress)?;
    let snapshot = tokio::task::spawn_blocking(move || SnapshotCoordinator::open(snapshot_config))
        .await
        .map_err(|_| ServiceRuntimeError::BlockingTaskFailed)??;
    let service = bounded_snapshot_service(super::SnapshotServiceHandler::new(snapshot));
    Server::builder()
        .add_service(service)
        .serve_with_shutdown(bind_address, shutdown_signal())
        .await
        .map_err(|_| ServiceRuntimeError::Transport)
}

#[cfg(feature = "capability-service")]
async fn run_all_until_shutdown<F>(
    config: ServiceConfig,
    snapshot_config: SnapshotServiceConfig,
    shutdown: F,
) -> Result<(), ServiceRuntimeError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let store_root = config.private_envelope_root().to_owned();
    let store = tokio::task::spawn_blocking(move || PrivateEnvelopeStore::open(store_root))
        .await
        .map_err(|_| ServiceRuntimeError::BlockingTaskFailed)??;
    let queue_config = QueueConfig::new(config.redis_url(), config.queue_prefix())?;
    let publisher = CapabilityJobPublisher::connect(store, &queue_config).await?;
    publisher
        .reconcile_startup(SystemTime::now(), ORPHAN_GRACE_PERIOD)
        .await?;
    let snapshot = tokio::task::spawn_blocking(move || SnapshotCoordinator::open(snapshot_config))
        .await
        .map_err(|_| ServiceRuntimeError::BlockingTaskFailed)??;

    let capability_service =
        bounded_capability_service(super::CapabilityServiceHandler::new(publisher, SystemClock));
    let snapshot_service = bounded_snapshot_service(super::SnapshotServiceHandler::new(snapshot));
    Server::builder()
        .add_service(capability_service)
        .add_service(snapshot_service)
        .serve_with_shutdown(config.bind_address(), shutdown)
        .await
        .map_err(|_| ServiceRuntimeError::Transport)
}

/// Runs startup reconciliation before binding and serves until shutdown resolves.
#[cfg(feature = "capability-service")]
pub async fn run_until_shutdown<F>(
    config: ServiceConfig,
    shutdown: F,
) -> Result<(), ServiceRuntimeError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let store_root = config.private_envelope_root().to_owned();
    let store = tokio::task::spawn_blocking(move || PrivateEnvelopeStore::open(store_root))
        .await
        .map_err(|_| ServiceRuntimeError::BlockingTaskFailed)??;
    let queue_config = QueueConfig::new(config.redis_url(), config.queue_prefix())?;
    let publisher = CapabilityJobPublisher::connect(store, &queue_config).await?;
    publisher
        .reconcile_startup(SystemTime::now(), ORPHAN_GRACE_PERIOD)
        .await?;

    let service =
        bounded_capability_service(super::CapabilityServiceHandler::new(publisher, SystemClock));
    Server::builder()
        .add_service(service)
        .serve_with_shutdown(config.bind_address(), shutdown)
        .await
        .map_err(|_| ServiceRuntimeError::Transport)
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Redacted failures emitted by the service process boundary.
#[derive(Debug)]
pub enum ServiceRuntimeError {
    #[cfg(feature = "capability-service")]
    Configuration(ServiceConfigError),
    SnapshotConfiguration(SnapshotServiceConfigError),
    SnapshotOperation(SnapshotOperationError),
    #[cfg(feature = "capability-service")]
    QueueConfiguration(QueueConfigError),
    #[cfg(feature = "capability-service")]
    PrivateEnvelope(PrivateEnvelopeError),
    #[cfg(feature = "capability-service")]
    Publication(CapabilityPublicationError),
    #[cfg(feature = "capability-service")]
    Reconciliation(StartupReconciliationError),
    BlockingTaskFailed,
    BindAddress,
    Transport,
}

impl fmt::Display for ServiceRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            #[cfg(feature = "capability-service")]
            Self::Configuration(_) => "razield configuration is invalid",
            Self::SnapshotConfiguration(_) => "razield snapshot configuration is invalid",
            Self::SnapshotOperation(_) => "razield snapshot startup failed",
            #[cfg(feature = "capability-service")]
            Self::QueueConfiguration(_) => "razield queue configuration is invalid",
            #[cfg(feature = "capability-service")]
            Self::PrivateEnvelope(_) => "razield private storage failed",
            #[cfg(feature = "capability-service")]
            Self::Publication(_) => "razield queue startup failed",
            #[cfg(feature = "capability-service")]
            Self::Reconciliation(_) => "razield startup reconciliation failed",
            Self::BlockingTaskFailed => "razield blocking startup task failed",
            Self::BindAddress => "razield bind address is invalid",
            Self::Transport => "razield transport failed",
        })
    }
}

impl Error for ServiceRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            #[cfg(feature = "capability-service")]
            Self::Configuration(error) => Some(error),
            Self::SnapshotConfiguration(error) => Some(error),
            Self::SnapshotOperation(error) => Some(error),
            #[cfg(feature = "capability-service")]
            Self::QueueConfiguration(error) => Some(error),
            #[cfg(feature = "capability-service")]
            Self::PrivateEnvelope(error) => Some(error),
            #[cfg(feature = "capability-service")]
            Self::Publication(error) => Some(error),
            #[cfg(feature = "capability-service")]
            Self::Reconciliation(error) => Some(error),
            Self::BlockingTaskFailed | Self::BindAddress | Self::Transport => None,
        }
    }
}

#[cfg(feature = "capability-service")]
impl From<ServiceConfigError> for ServiceRuntimeError {
    fn from(error: ServiceConfigError) -> Self {
        Self::Configuration(error)
    }
}

impl From<SnapshotServiceConfigError> for ServiceRuntimeError {
    fn from(error: SnapshotServiceConfigError) -> Self {
        Self::SnapshotConfiguration(error)
    }
}

impl From<SnapshotOperationError> for ServiceRuntimeError {
    fn from(error: SnapshotOperationError) -> Self {
        Self::SnapshotOperation(error)
    }
}

#[cfg(feature = "capability-service")]
impl From<QueueConfigError> for ServiceRuntimeError {
    fn from(error: QueueConfigError) -> Self {
        Self::QueueConfiguration(error)
    }
}

#[cfg(feature = "capability-service")]
impl From<PrivateEnvelopeError> for ServiceRuntimeError {
    fn from(error: PrivateEnvelopeError) -> Self {
        Self::PrivateEnvelope(error)
    }
}

#[cfg(feature = "capability-service")]
impl From<CapabilityPublicationError> for ServiceRuntimeError {
    fn from(error: CapabilityPublicationError) -> Self {
        Self::Publication(error)
    }
}

#[cfg(feature = "capability-service")]
impl From<StartupReconciliationError> for ServiceRuntimeError {
    fn from(error: StartupReconciliationError) -> Self {
        Self::Reconciliation(error)
    }
}
