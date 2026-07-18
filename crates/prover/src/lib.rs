//! Proving host boundary for Raziel.
//!
//! M0 reserves this crate for guest input assembly, local execution, and proof
//! packaging. It intentionally contains no proof or authorization behavior yet.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "raziel-prover");
    }
}
