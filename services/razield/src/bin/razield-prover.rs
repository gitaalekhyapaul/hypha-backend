//! Isolated one-request local Plonk prover child.

#![forbid(unsafe_code)]

fn main() -> Result<(), razield::one_shot::OneShotProtocolError> {
    let mut arguments = std::env::args().skip(1);
    match (arguments.next().as_deref(), arguments.next()) {
        (None, None) => razield::one_shot::run_child_from_env(),
        (Some("--check-program-key"), None) => {
            razield::one_shot::check_program_key_from_env()?;
            println!("approved_program_key=pass");
            Ok(())
        }
        _ => Err(razield::one_shot::OneShotProtocolError::Configuration),
    }
}
