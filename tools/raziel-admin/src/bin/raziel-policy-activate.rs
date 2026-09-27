fn main() {
    // Legacy activation flags and the read-only `--list-approved` operation share
    // the existing private owner socket. Neither operation accepts a socket path argument.
    if let Err(error) = raziel_admin::policy_activation::run_client_from_env() {
        eprintln!("raziel-policy-activate: {error}");
        std::process::exit(2);
    }
}
