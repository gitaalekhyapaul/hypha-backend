pub const SP1_SDK_VERSION: &str = "6.3.1";
pub(crate) const SP1_CIRCUIT_VERSION: &str = "v6.1.0";

#[cfg(all(test, feature = "proving"))]
mod tests {
    use super::*;

    #[test]
    fn pinned_circuit_version_matches_the_proving_sdk() {
        assert_eq!(SP1_CIRCUIT_VERSION, sp1_sdk::SP1_CIRCUIT_VERSION);
    }
}
