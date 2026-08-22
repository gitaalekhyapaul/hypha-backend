//! Bounded Tonic ingress for the asynchronous capability service.

mod clock;
pub(crate) mod config;
mod handler;
mod runtime;

pub use clock::{ClockError, SystemClock, TrustedClock};
pub use config::{
    MAX_SERVICE_MESSAGE_BYTES, ORPHAN_GRACE_PERIOD, ServiceConfig, ServiceConfigError,
};
pub use handler::{CapabilityPublisher, CapabilityServiceHandler, bounded_capability_service};
pub use runtime::{ServiceRuntimeError, run_from_env, run_until_shutdown};

#[cfg(test)]
mod handler_tests;
#[cfg(test)]
mod tests;
