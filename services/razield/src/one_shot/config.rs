use std::fmt::{self, Debug, Formatter};
use std::path::{Path, PathBuf};

use raziel_prover::TrustedProgramKey;
use raziel_verifier::default_registry_config;

use super::OneShotProtocolError;

pub(super) const CGROUP_ROOT: &str = "/sys/fs/cgroup";
pub(super) const MAX_GUEST_ELF_BYTES: u64 = 4 * 1024 * 1024;

const SOCKET_PATH_ENV: &str = "RAZIEL_ONE_SHOT_SOCKET_PATH";
const REGISTRY_PATH_ENV: &str = "RAZIEL_REGISTRY_PATH";
const PRIVATE_SNAPSHOT_ROOT_ENV: &str = "RAZIEL_PRIVATE_SNAPSHOT_ROOT";
pub(super) const GUEST_ELF_PATH_ENV: &str = "RAZIEL_GUEST_ELF_PATH";
const EXPECTED_VERIFICATION_KEY_ID_ENV: &str = "RAZIEL_EXPECTED_VERIFICATION_KEY_ID";
const EXPECTED_SP1_VKEY_HASH_ENV: &str = "RAZIEL_EXPECTED_SP1_VKEY_HASH";
const TRUST_DOMAIN_ID_ENV: &str = "RAZIEL_TRUST_DOMAIN_ID";
const PLONK_SETUP_PATH_ENV: &str = "SP1_PLONK_CIRCUIT_PATH";

/// Fixed owner configuration read before the child accepts private input.
pub struct OneShotProverConfig {
    socket_path: PathBuf,
    registry_path: PathBuf,
    private_snapshot_root: PathBuf,
    guest_elf_path: PathBuf,
    plonk_setup_path: PathBuf,
    expected_program_key: TrustedProgramKey,
    trust_domain_id: String,
}

impl OneShotProverConfig {
    pub fn from_env() -> Result<Self, OneShotProtocolError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, OneShotProtocolError> {
        let socket_path = required_absolute_path(&mut lookup, SOCKET_PATH_ENV, false)?;
        if socket_path.as_os_str().as_encoded_bytes().len() > 100 {
            return Err(OneShotProtocolError::Configuration);
        }
        let registry_path = required_absolute_path(&mut lookup, REGISTRY_PATH_ENV, true)?;
        let private_snapshot_root =
            required_absolute_path(&mut lookup, PRIVATE_SNAPSHOT_ROOT_ENV, true)?;
        let guest_elf_path = required_absolute_path(&mut lookup, GUEST_ELF_PATH_ENV, true)?;
        let plonk_setup_path = required_absolute_path(&mut lookup, PLONK_SETUP_PATH_ENV, true)?;
        let expected_program_key = required_program_key(&mut lookup)?;
        let trust_domain_id = lookup(TRUST_DOMAIN_ID_ENV)
            .filter(|value| default_registry_config(value).is_ok())
            .ok_or(OneShotProtocolError::Configuration)?;

        Ok(Self {
            socket_path,
            registry_path,
            private_snapshot_root,
            guest_elf_path,
            plonk_setup_path,
            expected_program_key,
            trust_domain_id,
        })
    }

    pub fn socket_path(&self) -> &Path {
        self.socket_path.as_path()
    }

    pub fn registry_path(&self) -> &Path {
        self.registry_path.as_path()
    }

    pub fn private_snapshot_root(&self) -> &Path {
        self.private_snapshot_root.as_path()
    }

    pub fn guest_elf_path(&self) -> &Path {
        self.guest_elf_path.as_path()
    }

    pub fn plonk_setup_path(&self) -> &Path {
        self.plonk_setup_path.as_path()
    }

    pub fn expected_verification_key_id(&self) -> &str {
        self.expected_program_key.verification_key_id()
    }

    pub(super) fn expected_program_key(&self) -> &TrustedProgramKey {
        &self.expected_program_key
    }

    pub const fn trust_domain_id(&self) -> &str {
        self.trust_domain_id.as_str()
    }
}

impl Debug for OneShotProverConfig {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OneShotProverConfig")
            .field("owner_configuration", &"<redacted>")
            .finish()
    }
}

pub(super) fn required_program_key(
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Result<TrustedProgramKey, OneShotProtocolError> {
    let key_id =
        lookup(EXPECTED_VERIFICATION_KEY_ID_ENV).ok_or(OneShotProtocolError::Configuration)?;
    let key_hash = lookup(EXPECTED_SP1_VKEY_HASH_ENV).ok_or(OneShotProtocolError::Configuration)?;
    TrustedProgramKey::from_registry_record(&key_id, &key_hash)
        .map_err(|_| OneShotProtocolError::Configuration)
}

pub(super) fn required_absolute_path(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    name: &str,
    must_exist: bool,
) -> Result<PathBuf, OneShotProtocolError> {
    let value = lookup(name).ok_or(OneShotProtocolError::Configuration)?;
    if value.is_empty() || value.contains('\0') {
        return Err(OneShotProtocolError::Configuration);
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() || (must_exist && std::fs::symlink_metadata(&path).is_err()) {
        return Err(OneShotProtocolError::Configuration);
    }
    Ok(path)
}

#[cfg(test)]
pub(super) fn config_from_pairs(
    pairs: &[(&str, String)],
) -> Result<OneShotProverConfig, OneShotProtocolError> {
    OneShotProverConfig::from_lookup(|name| {
        pairs
            .iter()
            .find_map(|(key, value)| (*key == name).then(|| value.clone()))
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn fixed_configuration_requires_complete_absolute_owner_values() {
        let root = test_root("valid");
        std::fs::create_dir_all(root.join("registry")).unwrap();
        std::fs::create_dir_all(root.join("snapshots")).unwrap();
        std::fs::create_dir_all(root.join("plonk")).unwrap();
        std::fs::write(root.join("guest.elf"), b"guest").unwrap();
        let values = values(&root);

        let config = config_from_pairs(&values).unwrap();
        assert_eq!(config.registry_path(), root.join("registry"));
        assert_eq!(config.private_snapshot_root(), root.join("snapshots"));
        assert_eq!(config.guest_elf_path(), root.join("guest.elf"));
        assert_eq!(config.plonk_setup_path(), root.join("plonk"));
        assert_eq!(config.trust_domain_id(), "raziel-test");
        assert!(!format!("{config:?}").contains(root.to_str().unwrap()));

        for hash in [None, Some("0x00"), Some("0xINVALID")] {
            let mut invalid = values.clone();
            invalid.retain(|(name, _)| *name != EXPECTED_SP1_VKEY_HASH_ENV);
            if let Some(hash) = hash {
                invalid.push((EXPECTED_SP1_VKEY_HASH_ENV, hash.into()));
            }
            assert!(matches!(
                config_from_pairs(&invalid),
                Err(OneShotProtocolError::Configuration)
            ));
        }

        let mut missing = values.clone();
        missing.retain(|(name, _)| *name != REGISTRY_PATH_ENV);
        assert!(matches!(
            config_from_pairs(&missing),
            Err(OneShotProtocolError::Configuration)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn values(root: &Path) -> Vec<(&'static str, String)> {
        vec![
            (
                SOCKET_PATH_ENV,
                root.join("proof.sock").display().to_string(),
            ),
            (
                REGISTRY_PATH_ENV,
                root.join("registry").display().to_string(),
            ),
            (
                PRIVATE_SNAPSHOT_ROOT_ENV,
                root.join("snapshots").display().to_string(),
            ),
            (
                GUEST_ELF_PATH_ENV,
                root.join("guest.elf").display().to_string(),
            ),
            (
                EXPECTED_VERIFICATION_KEY_ID_ENV,
                format!("sp1-vk-v1:{}", "ab".repeat(32)),
            ),
            (
                EXPECTED_SP1_VKEY_HASH_ENV,
                "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".into(),
            ),
            (TRUST_DOMAIN_ID_ENV, "raziel-test".into()),
            (
                PLONK_SETUP_PATH_ENV,
                root.join("plonk").display().to_string(),
            ),
        ]
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "razield-one-shot-config-{label}-{}",
            std::process::id()
        ))
    }
}
