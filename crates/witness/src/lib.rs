//! Private witness construction boundary for Raziel.
//!
//! M0 reserves this crate for positive-path search and membership-witness assembly.
//! It intentionally contains no graph search or witness behavior yet.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "raziel-witness");
    }
}
