//! Raziel snapshot and prover service shell.
//!
//! M0 defines the process boundary only. Snapshot orchestration, witness construction,
//! proof generation, capability issuance, and network serving are not implemented.

#![forbid(unsafe_code)]

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "razield");
    }
}
