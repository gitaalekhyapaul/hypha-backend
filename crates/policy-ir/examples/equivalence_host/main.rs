mod cli;
mod error;
mod fixture;
mod hash;
mod runner;

use std::env;
use std::process::ExitCode;

use error::HostResult;

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("equivalence host failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> HostResult<()> {
    let cli = cli::parse(env::args_os().skip(1))?;
    let root = fixture::repository_root()?;
    let outcomes = runner::run(&root, &cli.fixture)?;
    runner::write_outcomes(&outcomes, cli.output.as_deref())
}
