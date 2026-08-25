use std::error::Error;
use std::fmt::{self, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;

use raziel_prover::TrustedProgramKey;
use raziel_verifier::{RegistryConfig, default_registry_config};

pub const VERIFIER_BIND_ADDRESS_ENV: &str = "RAZIEL_VERIFIER_BIND_ADDRESS";
pub const REGISTRY_PATH_ENV: &str = "RAZIEL_REGISTRY_PATH";
pub const TRUST_DOMAIN_ID_ENV: &str = "RAZIEL_TRUST_DOMAIN_ID";
pub const VERIFICATION_KEY_ID_ENV: &str = "RAZIEL_EXPECTED_VERIFICATION_KEY_ID";
pub const SP1_VKEY_HASH_ENV: &str = "RAZIEL_EXPECTED_SP1_VKEY_HASH";

pub struct VerifierdConfig {
    bind_address: SocketAddr,
    registry_path: PathBuf,
    registry_config: RegistryConfig,
    trusted_program_key: TrustedProgramKey,
}

impl VerifierdConfig {
    pub fn from_env() -> Result<Self, VerifierdConfigError> {
        let bind_address = std::env::var(VERIFIER_BIND_ADDRESS_ENV)
            .unwrap_or_else(|_| "0.0.0.0:50053".to_owned())
            .parse()
            .map_err(|_| VerifierdConfigError)?;
        let registry_path = required(REGISTRY_PATH_ENV).map(PathBuf::from)?;
        let trust_domain_id = required(TRUST_DOMAIN_ID_ENV)?;
        let registry_config =
            default_registry_config(&trust_domain_id).map_err(|_| VerifierdConfigError)?;
        let trusted_program_key = TrustedProgramKey::from_registry_record(
            &required(VERIFICATION_KEY_ID_ENV)?,
            &required(SP1_VKEY_HASH_ENV)?,
        )
        .map_err(|_| VerifierdConfigError)?;
        Ok(Self {
            bind_address,
            registry_path,
            registry_config,
            trusted_program_key,
        })
    }

    #[must_use]
    pub const fn bind_address(&self) -> SocketAddr {
        self.bind_address
    }

    #[must_use]
    pub fn registry_path(&self) -> &std::path::Path {
        &self.registry_path
    }

    #[must_use]
    pub const fn registry_config(&self) -> &RegistryConfig {
        &self.registry_config
    }

    #[must_use]
    pub const fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.trusted_program_key
    }
}

fn required(name: &str) -> Result<String, VerifierdConfigError> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(VerifierdConfigError)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifierdConfigError;

impl fmt::Display for VerifierdConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("verifierd configuration is invalid")
    }
}

impl Error for VerifierdConfigError {}
