fn main() {
    if let Err(error) = raziel_admin::policy_activation::run_client_from_env() {
        eprintln!("raziel-policy-activate: {error}");
        std::process::exit(2);
    }
}
