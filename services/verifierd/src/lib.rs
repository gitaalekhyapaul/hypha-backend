//! Independent capability admission and isolated SP1 verification.

#![forbid(unsafe_code)]

pub mod admission;
#[cfg(test)]
mod retained_fixture;
pub mod service;
pub mod worker;
