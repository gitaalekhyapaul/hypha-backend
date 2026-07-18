//! PolicyIR boundary for Raziel.
//!
//! M0 reserves this crate for PolicyIR types, validation, and bounded evaluation.
//! It intentionally contains no policy representation or authorization behavior yet.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "raziel-policy-ir");
    }
}
