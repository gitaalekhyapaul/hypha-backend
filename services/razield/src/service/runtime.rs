use std::error::Error;
use std::fmt;
use std::future::Future;
use std::time::SystemTime;

use tonic::transport::Server;

use super::{
    ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError, SystemClock, bounded_capability_service,
};
use crate::private_envelope::{PrivateEnvelopeError, PrivateEnvelopeStore};
use crate::publication::{
    CapabilityJobPublisher, CapabilityPublicationError, StartupReconciliationError,
};
use crate::queue::{QueueConfig, QueueConfigError};

/// Runs the owner service from its explicit environment contract.
pub async fn run_from_env() -> Result<(), ServiceRuntimeError> {
    let config = ServiceConfig::from_env()?;
    run_until_shutdown(config, shutdown_signal()).await
}

/// Runs startup reconciliation before binding and serves until shutdown resolves.
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
    Configuration(ServiceConfigError),
    QueueConfiguration(QueueConfigError),
    PrivateEnvelope(PrivateEnvelopeError),
    Publication(CapabilityPublicationError),
    Reconciliation(StartupReconciliationError),
    BlockingTaskFailed,
    Transport,
}

impl fmt::Display for ServiceRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration(_) => "razield configuration is invalid",
            Self::QueueConfiguration(_) => "razield queue configuration is invalid",
            Self::PrivateEnvelope(_) => "razield private storage failed",
            Self::Publication(_) => "razield queue startup failed",
            Self::Reconciliation(_) => "razield startup reconciliation failed",
            Self::BlockingTaskFailed => "razield blocking startup task failed",
            Self::Transport => "razield transport failed",
        })
    }
}

impl Error for ServiceRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::QueueConfiguration(error) => Some(error),
            Self::PrivateEnvelope(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::Reconciliation(error) => Some(error),
            Self::BlockingTaskFailed | Self::Transport => None,
        }
    }
}

impl From<ServiceConfigError> for ServiceRuntimeError {
    fn from(error: ServiceConfigError) -> Self {
        Self::Configuration(error)
    }
}

impl From<QueueConfigError> for ServiceRuntimeError {
    fn from(error: QueueConfigError) -> Self {
        Self::QueueConfiguration(error)
    }
}

impl From<PrivateEnvelopeError> for ServiceRuntimeError {
    fn from(error: PrivateEnvelopeError) -> Self {
        Self::PrivateEnvelope(error)
    }
}

impl From<CapabilityPublicationError> for ServiceRuntimeError {
    fn from(error: CapabilityPublicationError) -> Self {
        Self::Publication(error)
    }
}

impl From<StartupReconciliationError> for ServiceRuntimeError {
    fn from(error: StartupReconciliationError) -> Self {
        Self::Reconciliation(error)
    }
}
