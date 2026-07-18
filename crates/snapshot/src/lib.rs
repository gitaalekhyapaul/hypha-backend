//! Relationship snapshot boundary for Raziel.
//!
//! M0 reserves this crate for relationship leaves, snapshot trees, and signed
//! descriptors. It intentionally contains no snapshot or cryptographic behavior yet.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "raziel-snapshot");
    }
}
