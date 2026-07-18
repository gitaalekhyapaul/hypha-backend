//! Canonical encoding boundary for Raziel.
//!
//! M0 reserves this crate for versioned canonical encodings and commitments. It
//! intentionally contains no encoding or hashing behavior yet.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "raziel-canonical");
    }
}
