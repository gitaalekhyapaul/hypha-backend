//! SP1 authorization guest entrypoint for Raziel.
//!
//! M0 compiles this boundary without inputs, evaluation, commitments, or public
//! values.

#![no_main]

sp1_zkvm::entrypoint!(main);

pub fn main() {}
