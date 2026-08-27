//! Bounded Tonic ingress for the asynchronous capability service.

mod clock;
#[cfg(feature = "capability-service")]
pub(crate) mod config;
#[cfg(feature = "capability-service")]
mod handler;
mod runtime;
mod snapshot_config;
mod snapshot_handler;
mod snapshot_runtime;

pub use clock::{ClockError, SystemClock, TrustedClock};
#[cfg(feature = "capability-service")]
pub use config::{ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError};
pub const MAX_SERVICE_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
#[cfg(feature = "capability-service")]
pub use handler::{CapabilityJobAccess, CapabilityServiceHandler, bounded_capability_service};
pub use runtime::{ServiceRuntimeError, run_snapshot_from_env};
#[cfg(feature = "capability-service")]
pub use runtime::{run_capability_from_env, run_from_env, run_until_shutdown};
pub use snapshot_config::{SnapshotServiceConfig, SnapshotServiceConfigError};
pub use snapshot_handler::{SnapshotOperations, SnapshotServiceHandler, bounded_snapshot_service};
pub use snapshot_runtime::{SnapshotCoordinator, SnapshotOperationError};

#[cfg(test)]
mod handler_tests;
#[cfg(test)]
mod snapshot_handler_tests;
#[cfg(test)]
mod tests;
