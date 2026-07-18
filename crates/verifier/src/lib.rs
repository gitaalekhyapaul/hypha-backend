//! Independent proof-verification boundary for Raziel.
//!
//! M0 reserves this crate for local registry checks, proof verification, and public
//! claim validation. It intentionally contains no verification behavior yet.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "raziel-verifier");
    }
}
