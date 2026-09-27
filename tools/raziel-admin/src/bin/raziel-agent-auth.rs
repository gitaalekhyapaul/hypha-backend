//! Local key validation and strict agent API signature verification.

#![forbid(unsafe_code)]

use raziel_admin::agent_api::AgentAuthError;

fn main() -> std::process::ExitCode {
    match raziel_admin::agent_api::run_from_env() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if error == AgentAuthError::Internal {
                eprintln!("raziel-agent-auth: {error}");
            }
            std::process::ExitCode::from(error.exit_code())
        }
    }
}
