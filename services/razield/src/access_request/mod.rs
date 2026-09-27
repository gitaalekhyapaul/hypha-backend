//! Owner-only access inbox. Redis contains public metadata, never typed context.

mod approval;
mod approval_lifecycle;
mod dispatch;
mod dispatch_model;
mod issuance;
mod issuance_model;
mod model;
mod preparation;
mod private;
mod proof_handoff;
mod proof_result;
mod redis_store;
mod runtime;
mod service;

#[cfg(test)]
mod approval_tests;

pub use dispatch_model::DispatchReference;
pub use model::{AccessError, MAX_INTENT_BYTES, RETENTION_SECONDS, REVIEW_SECONDS};
pub use preparation::ApprovedPreparation;
#[cfg(feature = "access-dispatch")]
pub(crate) use proof_handoff::PendingProof;
pub use runtime::{AccessServiceConfig, run_from_env};
pub use service::{AccessCoordinator, AccessRequestHandler, bounded_access_service};
