//! Isolated one-request local Plonk prover child.

#![forbid(unsafe_code)]

fn main() -> Result<(), razield::one_shot::OneShotProtocolError> {
    razield::one_shot::run_child_from_env()
}
