//! Owner-only access inbox. Redis contains public metadata, never typed context.

mod approval;
mod approval_lifecycle;
mod model;
mod preparation;
mod private;
mod redis_store;
mod runtime;
mod service;

#[cfg(test)]
mod approval_tests;

pub use model::{AccessError, MAX_INTENT_BYTES, RETENTION_SECONDS, REVIEW_SECONDS};
pub use preparation::ApprovedPreparation;
pub use runtime::{AccessServiceConfig, run_from_env};
pub use service::{AccessCoordinator, AccessRequestHandler, bounded_access_service};
