fn main() {
    if let Err(error) = raziel_admin::writer::run_from_env() {
        eprintln!("raziel-registry-writer: {error}");
        std::process::exit(2);
    }
}
