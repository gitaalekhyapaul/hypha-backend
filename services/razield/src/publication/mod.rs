//! Atomic handoff from admitted capability requests to BullMQ.

mod bullmq;
mod cancellation;
mod coordinator;
mod lifecycle;
mod payload;
mod reconciliation;

#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests;

pub use coordinator::{
    CapabilityJobPublisher, CapabilityPublicationError, PublicationDisposition,
    PublishedCapabilityJob,
};
pub use lifecycle::{CapabilityLifecycleError, PublicCapabilityJobState, RetainedCapabilityJob};
pub use payload::{CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION, CapabilityJobPayload};
pub use reconciliation::{StartupReconciliationError, StartupReconciliationSummary};

pub(crate) use cancellation::{
    CANCELLATION_JOB_NAME, CancellationCommandPayload, CancellationCommandResult,
};

/// Stable BullMQ job name consumed by the later proof supervisor.
pub const CAPABILITY_JOB_NAME: &str = "prove-capability";
