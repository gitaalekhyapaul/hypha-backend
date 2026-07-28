use crate::ProverError;

pub const FORBIDDEN_SP1_ENV_VARS: &[&str] = &[
    "SP1_DUMP",
    "SP1_RECORD_WRITE_DIR",
    "TRACE_FILE",
    "DUMP_ELF_OUTPUT",
    "SP1_RECORD_MAX_ARITY_INPUT",
    "SP1_RECORD_SHRINK_INPUT",
    "WITHOUT_VK_VERIFICATION",
    "SP1_CIRCUIT_MODE",
];

const FORBIDDEN_SP1_ENV_PREFIXES: &[&str] = &["SP1_WORKER_"];

pub fn require_safe_sp1_environment() -> Result<(), ProverError> {
    let forbidden_is_set = std::env::vars_os()
        .any(|(name, _)| name.to_str().is_some_and(is_forbidden_sp1_environment_name));
    if forbidden_is_set {
        Err(ProverError::UnsafeSp1Environment)
    } else {
        Ok(())
    }
}

fn is_forbidden_sp1_environment_name(name: &str) -> bool {
    FORBIDDEN_SP1_ENV_VARS.contains(&name)
        || FORBIDDEN_SP1_ENV_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_forbidden_sp1_setting_is_rejected() {
        for forbidden in FORBIDDEN_SP1_ENV_VARS {
            assert!(is_forbidden_sp1_environment_name(forbidden));
        }
        assert!(is_forbidden_sp1_environment_name(
            "SP1_WORKER_FUTURE_OPTION"
        ));
        assert!(!is_forbidden_sp1_environment_name("SP1_PLONK_CIRCUIT_PATH"));
    }
}
