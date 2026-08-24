fn main() {
    if let Err(error) = raziel_admin::run_from_env() {
        eprintln!("raziel-admin: {error}");
        std::process::exit(2);
    }
}
