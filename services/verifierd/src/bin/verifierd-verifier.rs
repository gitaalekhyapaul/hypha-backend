//! Isolated one-request SP1 Plonk verifier child.

#![forbid(unsafe_code)]

fn main() -> Result<(), verifierd::worker::VerificationWorkerProtocolError> {
    verifierd::worker::run_child_from_env()
}
