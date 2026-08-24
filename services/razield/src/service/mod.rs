//! Bounded Tonic ingress for the asynchronous capability service.

mod clock;
pub(crate) mod config;
mod handler;
mod runtime;
mod snapshot_config;
mod snapshot_handler;
mod snapshot_runtime;

pub use clock::{ClockError, SystemClock, TrustedClock};
pub use config::{
    MAX_SERVICE_MESSAGE_BYTES, ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError,
};
pub use handler::{CapabilityPublisher, CapabilityServiceHandler, bounded_capability_service};
pub use runtime::{ServiceRuntimeError, run_from_env, run_until_shutdown};
pub use snapshot_config::{SnapshotServiceConfig, SnapshotServiceConfigError};
pub use snapshot_handler::{SnapshotOperations, SnapshotServiceHandler, bounded_snapshot_service};
pub use snapshot_runtime::{SnapshotCoordinator, SnapshotOperationError};

#[cfg(test)]
mod handler_tests;
#[cfg(test)]
mod snapshot_handler_tests;
#[cfg(test)]
mod tests;
