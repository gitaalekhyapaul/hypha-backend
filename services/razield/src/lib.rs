//! Reusable service policy for the Raziel issuer and proof-job supervisor.

#![forbid(unsafe_code)]

#[cfg(feature = "access-service")]
pub mod access_request;
pub mod admission;
#[cfg(feature = "capability-service")]
pub mod input_assembly;
#[cfg(feature = "capability-service")]
pub mod one_shot;
#[cfg(feature = "capability-service")]
pub mod private_envelope;
pub mod private_snapshot;
#[cfg(feature = "capability-service")]
pub mod publication;
#[cfg(feature = "capability-service")]
pub mod queue;
#[cfg(feature = "access-service")]
pub mod request_preparation;
pub mod service;
pub mod spicedb;
#[cfg(feature = "capability-service")]
pub mod worker;
