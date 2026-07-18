//! Independent Raziel verifier service shell.
//!
//! M0 defines a standalone process boundary with no `razield` dependency or client.
//! Proof verification, registry access, request validation, and serving are absent.

#![forbid(unsafe_code)]

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "verifierd");
    }
}
