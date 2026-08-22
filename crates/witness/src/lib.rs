//! Private positive-path witness construction for Raziel.

#![forbid(unsafe_code)]

mod builder;
mod error;

pub use builder::build_positive_witness;
pub use error::WitnessBuildError;
