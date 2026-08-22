use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use bullmq::worker::CancellationToken;

use crate::private_envelope::PrivateCapabilityEnvelope;

/// Approved maximum wall time for one local proof execution.
pub const PROOF_EXECUTION_DEADLINE: Duration = Duration::from_secs(60 * 60);

/// Maximum time for an executor to confirm exit after cancellation.
pub const EXECUTOR_TERMINATION_GRACE: Duration = Duration::from_secs(30);

/// Boxed one-shot executor future used without an async-trait dependency.
pub type ExecutionFuture =
    Pin<Box<dyn Future<Output = Result<(), ExecutorFailureClass>> + Send + 'static>>;

/// Narrow handoff accepted by a future isolated one-shot prover.
pub struct CapabilityExecutionRequest {
    envelope: PrivateCapabilityEnvelope,
    issued_at_unix_seconds: i64,
}

impl CapabilityExecutionRequest {
    pub(crate) const fn new(
        envelope: PrivateCapabilityEnvelope,
        issued_at_unix_seconds: i64,
    ) -> Self {
        Self {
            envelope,
            issued_at_unix_seconds,
        }
    }

    /// Returns the commitment-checked private request loaded after the queue claim.
    pub const fn envelope(&self) -> &PrivateCapabilityEnvelope {
        &self.envelope
    }

    /// Returns the owner-trusted whole-second start time for later proof binding.
    pub const fn issued_at_unix_seconds(&self) -> i64 {
        self.issued_at_unix_seconds
    }
}

impl Debug for CapabilityExecutionRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityExecutionRequest")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Fixed execution controls that request data cannot select.
#[derive(Clone)]
pub struct ExecutionControl {
    deadline: Duration,
    cancellation: CancellationToken,
}

impl ExecutionControl {
    pub(crate) const fn new(deadline: Duration, cancellation: CancellationToken) -> Self {
        Self {
            deadline,
            cancellation,
        }
    }

    /// Returns the ADR-0013 proof deadline.
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }

    /// Checks whether BullMQ or the supervisor requested termination.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Waits for BullMQ or the supervisor to request termination.
    pub async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }
}

impl Debug for ExecutionControl {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutionControl")
            .field("deadline", &self.deadline)
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Injectable one-shot execution boundary. Concrete proving remains a later slice.
///
/// Implementations must stop private work after cancellation and must terminate any
/// owned child process when the returned future is dropped.
pub trait CapabilityExecutor: Send + Sync + 'static {
    fn execute(
        &self,
        request: CapabilityExecutionRequest,
        control: ExecutionControl,
    ) -> ExecutionFuture;
}

/// Redacted private failure classes returned by a one-shot executor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutorFailureClass {
    Cancelled,
    Rejected,
    Unavailable,
    TimedOut,
    TerminationUnconfirmed,
    MalformedResult,
}
