//! Independent Raziel verifier service.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), verifierd::service::VerifierdRuntimeError> {
    verifierd::service::run_from_env().await
}

#[cfg(test)]
mod tests {
    #[test]
    fn package_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "verifierd");
    }
}
