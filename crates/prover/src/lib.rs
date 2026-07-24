//! Local SP1 execution boundary for the Raziel authorization guest.

#![forbid(unsafe_code)]

mod error;
mod execute;
mod input;

pub use error::ProverError;
pub use execute::{MAX_EXECUTION_CYCLE_LIMIT, Sp1ExecuteReport, Sp1Executor};
pub use input::{PreparedGuestInput, prepare_guest_input};
