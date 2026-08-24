//! Reusable service policy for the Raziel issuer and proof-job supervisor.

#![forbid(unsafe_code)]

pub mod admission;
pub mod input_assembly;
pub mod one_shot;
pub mod private_envelope;
pub mod private_snapshot;
pub mod publication;
pub mod queue;
pub mod service;
pub mod spicedb;
pub mod worker;
