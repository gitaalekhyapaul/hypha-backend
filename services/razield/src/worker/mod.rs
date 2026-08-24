//! BullMQ claim boundary for the proof supervisor.

mod cancellation;
mod config;
mod executor;
mod processor;
mod runtime;
mod terminal;

pub use crate::queue::{
    WORKER_DRAIN_DELAY, WORKER_LOCK_DURATION, WORKER_LOCK_RENEW_TIME, WORKER_STALLED_INTERVAL,
};
pub use config::{SupervisorConfig, SupervisorConfigError};
pub use executor::{
    CapabilityExecutionRequest, CapabilityExecutor, EXECUTOR_TERMINATION_GRACE, ExecutionControl,
    ExecutionFuture, ExecutorFailureClass, PROOF_EXECUTION_DEADLINE,
};
pub use processor::{PublicFailureClass, SupervisorFailureClass, SupervisorProgressPhase};
pub use runtime::{ProofSupervisor, SupervisorRuntimeError};
pub use terminal::{
    TERMINAL_REGISTRY_PATH_ENV, TERMINAL_SP1_VKEY_HASH_ENV, TERMINAL_TRUST_DOMAIN_ID_ENV,
    TERMINAL_VERIFICATION_KEY_ID_ENV, TerminalCapabilityResult, TerminalResultValidator,
    TerminalValidationError, TerminalValidatorConfig,
};

pub(crate) use cancellation::{CancellationCommandProcessor, CancellationIntentRegistry};
pub(crate) use processor::{ClaimedJobProcessor, ProgressPublisher};
pub(crate) use terminal::{CapabilityResultValidator, TerminalIssuanceContext};

#[cfg(test)]
pub(crate) use processor::{EnvelopeAccess, execute_with_limits};
#[cfg(test)]
pub(crate) use runtime::ProofSupervisorActivation;

#[cfg(test)]
mod tests;
