//! Atomic handoff from admitted capability requests to BullMQ.

mod bullmq;
mod coordinator;
mod payload;

#[cfg(test)]
mod tests;

pub use coordinator::{
    CapabilityJobPublisher, CapabilityPublicationError, PublicationDisposition,
    PublishedCapabilityJob,
};
pub use payload::{CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION, CapabilityJobPayload};

/// Stable BullMQ job name consumed by the later proof supervisor.
pub const CAPABILITY_JOB_NAME: &str = "prove-capability";
