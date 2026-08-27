fn main() {
    if let Err(error) = raziel_admin::policy_authority::run_from_env() {
        eprintln!("raziel-policy-authority: {error}");
        std::process::exit(2);
    }
}
