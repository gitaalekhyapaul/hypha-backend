use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::{HostError, HostResult};

#[derive(Debug, PartialEq, Eq)]
pub struct Cli {
    pub fixture: PathBuf,
    pub output: Option<PathBuf>,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> HostResult<Cli> {
    let mut args = args.into_iter();
    let mut fixture = None;
    let mut output = None;
    let mut output_seen = false;

    while let Some(flag) = args.next() {
        let flag = flag.to_str().ok_or(HostError::NonUtf8Flag)?;
        match flag {
            "--fixture" => {
                if fixture.is_some() {
                    return Err(HostError::DuplicateFlag("--fixture"));
                }
                fixture = Some(next_value(&mut args, "--fixture")?);
            }
            "--output" => {
                if output_seen {
                    return Err(HostError::DuplicateFlag("--output"));
                }
                output_seen = true;
                let value = next_value(&mut args, "--output")?;
                if value.as_os_str() != "-" {
                    output = Some(value);
                }
            }
            _ => return Err(HostError::UnknownFlag),
        }
    }

    Ok(Cli {
        fixture: fixture.ok_or(HostError::MissingFlag("--fixture"))?,
        output,
    })
}

fn next_value(
    args: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
) -> HostResult<PathBuf> {
    let value = args.next().ok_or(HostError::MissingFlagValue(flag))?;
    if value.is_empty() {
        return Err(HostError::MissingFlagValue(flag));
    }
    Ok(PathBuf::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_exact_fixture_and_output_flags() {
        assert_eq!(
            parse(args(&["--fixture", "tests/equivalence/fixtures/v1.json"])).unwrap(),
            Cli {
                fixture: "tests/equivalence/fixtures/v1.json".into(),
                output: None,
            }
        );
        assert_eq!(
            parse(args(&[
                "--output",
                "/tmp/results.tsv",
                "--fixture",
                "tests/equivalence/fixtures/v1.json",
            ]))
            .unwrap()
            .output,
            Some("/tmp/results.tsv".into())
        );
        assert_eq!(
            parse(args(&[
                "--fixture",
                "tests/equivalence/fixtures/v1.json",
                "--output",
                "-",
            ]))
            .unwrap()
            .output,
            None
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_unknown_flags() {
        assert!(matches!(
            parse(args(&[])),
            Err(HostError::MissingFlag("--fixture"))
        ));
        assert!(matches!(
            parse(args(&["--fixture"])),
            Err(HostError::MissingFlagValue("--fixture"))
        ));
        assert!(matches!(
            parse(args(&["--fixture", "a", "--fixture", "b"])),
            Err(HostError::DuplicateFlag("--fixture"))
        ));
        assert!(matches!(
            parse(args(&["fixture.json"])),
            Err(HostError::UnknownFlag)
        ));
    }
}
