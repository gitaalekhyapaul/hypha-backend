//! BullMQ claim boundary for the proof supervisor.

mod config;
mod executor;
mod processor;
mod runtime;

pub use crate::queue::{
    WORKER_DRAIN_DELAY, WORKER_LOCK_DURATION, WORKER_LOCK_RENEW_TIME, WORKER_STALLED_INTERVAL,
};
pub use config::{SupervisorConfig, SupervisorConfigError};
pub use executor::{
    CapabilityExecutionRequest, CapabilityExecutor, EXECUTOR_TERMINATION_GRACE, ExecutionControl,
    ExecutionFuture, ExecutorFailureClass, PROOF_EXECUTION_DEADLINE,
};
pub use processor::{PublicFailureClass, SupervisorFailureClass, SupervisorProgressPhase};
pub use runtime::{ProofSupervisor, ProofSupervisorActivation};

pub(crate) use processor::{ClaimedJobProcessor, ProgressPublisher};

#[cfg(test)]
pub(crate) use processor::EnvelopeAccess;

#[cfg(test)]
mod tests;
