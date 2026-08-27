fn main() {
    if let Err(error) = raziel_admin::policy_activation::run_from_env() {
        eprintln!("raziel-policy-activation: {error}");
        std::process::exit(2);
    }
}
