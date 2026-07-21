use std::fmt::{self, Display, Formatter};
use std::io;

use raziel_policy_ir::{EvaluationError, PolicyIngressError};

#[derive(Debug)]
pub enum HostError {
    NonUtf8Flag,
    DuplicateFlag(&'static str),
    MissingFlag(&'static str),
    MissingFlagValue(&'static str),
    UnknownFlag,
    Io {
        action: &'static str,
        artifact: &'static str,
        source: io::Error,
    },
    Json(serde_json::Error),
    CompilerConfigJson(serde_json::Error),
    UnsupportedFixtureVersion(u32),
    UnsupportedCompilerConfigVersion(u32),
    FixtureLimit(&'static str),
    InvalidFixture(&'static str),
    InvalidCaseId,
    CasesNotSorted,
    DuplicateCaseId,
    DuplicateRelationship,
    ContextNotSorted(String),
    PathNotConfined(&'static str),
    SchemaDigestEncoding,
    SchemaDigestMismatch(&'static str),
    CompilerConfigMismatch(&'static str),
    PolicyIngress(PolicyIngressError),
    InvalidRelationship,
    MissingPermission(String),
    UnknownCaveat(String),
    UnknownInput(String),
    AmbiguousInputProvenance(String),
    WrongInputProvenance(String),
    InputTypeMismatch(String),
    MissingRelationshipInput {
        caveat: String,
        parameter: String,
    },
    MissingEvaluationInput(String),
    Evaluation {
        case_id: String,
        source: EvaluationError,
    },
    ExpectationMismatch {
        case_id: String,
        expected: &'static str,
        actual: &'static str,
    },
}

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8Flag => formatter.write_str("command flag is not valid UTF-8"),
            Self::DuplicateFlag(flag) => write!(formatter, "duplicate {flag} flag"),
            Self::MissingFlag(flag) => write!(formatter, "required {flag} flag is missing"),
            Self::MissingFlagValue(flag) => write!(formatter, "{flag} needs a value"),
            Self::UnknownFlag => formatter.write_str("unknown command flag"),
            Self::Io {
                action,
                artifact,
                source,
            } => write!(formatter, "failed to {action} {artifact}: {source}"),
            Self::Json(source) => write!(
                formatter,
                "fixture JSON is invalid at line {}, column {}",
                source.line(),
                source.column()
            ),
            Self::CompilerConfigJson(source) => write!(
                formatter,
                "compiler config JSON is invalid at line {}, column {}",
                source.line(),
                source.column()
            ),
            Self::UnsupportedFixtureVersion(version) => {
                write!(formatter, "fixture version {version} is not supported")
            }
            Self::UnsupportedCompilerConfigVersion(version) => {
                write!(
                    formatter,
                    "compiler config version {version} is not supported"
                )
            }
            Self::FixtureLimit(limit) => write!(formatter, "fixture exceeds {limit}"),
            Self::InvalidFixture(reason) => write!(formatter, "fixture is invalid: {reason}"),
            Self::InvalidCaseId => formatter.write_str("fixture contains an invalid case ID"),
            Self::CasesNotSorted => formatter.write_str("fixture case IDs are not sorted"),
            Self::DuplicateCaseId => formatter.write_str("fixture contains a duplicate case ID"),
            Self::DuplicateRelationship => {
                formatter.write_str("fixture contains a duplicate relationship key")
            }
            Self::ContextNotSorted(scope) => {
                write!(formatter, "{scope} context names are not sorted and unique")
            }
            Self::PathNotConfined(field) => {
                write!(
                    formatter,
                    "{field} must resolve to a file inside the repository"
                )
            }
            Self::SchemaDigestEncoding => {
                formatter.write_str("schema_sha256 must contain 64 hexadecimal characters")
            }
            Self::SchemaDigestMismatch(layer) => {
                write!(formatter, "schema digest does not match {layer}")
            }
            Self::CompilerConfigMismatch(field) => {
                write!(formatter, "compiler config and PolicyIR {field} differ")
            }
            Self::PolicyIngress(source) => {
                write!(formatter, "PolicyIR ingress failed: {source}")
            }
            Self::InvalidRelationship => {
                formatter.write_str("fixture relationship is not permitted by PolicyIR")
            }
            Self::MissingPermission(case_id) => {
                write!(
                    formatter,
                    "case {case_id} names a permission absent from PolicyIR"
                )
            }
            Self::UnknownCaveat(name) => write!(formatter, "fixture names unknown caveat {name}"),
            Self::UnknownInput(name) => {
                write!(formatter, "fixture names unknown caveat input {name}")
            }
            Self::AmbiguousInputProvenance(name) => {
                write!(
                    formatter,
                    "PolicyIR has ambiguous provenance for caveat input {name}"
                )
            }
            Self::WrongInputProvenance(name) => {
                write!(
                    formatter,
                    "caveat input {name} is placed in the wrong source"
                )
            }
            Self::InputTypeMismatch(name) => {
                write!(formatter, "caveat input {name} has the wrong fixture type")
            }
            Self::MissingRelationshipInput { caveat, parameter } => write!(
                formatter,
                "relationship caveat {caveat} is missing required input {parameter}"
            ),
            Self::MissingEvaluationInput(case_id) => {
                write!(
                    formatter,
                    "case {case_id} is missing a required caveat input"
                )
            }
            Self::Evaluation { case_id, source } => {
                write!(
                    formatter,
                    "case {case_id} failed PolicyIR evaluation: {source}"
                )
            }
            Self::ExpectationMismatch {
                case_id,
                expected,
                actual,
            } => write!(
                formatter,
                "case {case_id} expected {expected} but PolicyIR returned {actual}"
            ),
        }
    }
}

impl std::error::Error for HostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(source) => Some(source),
            Self::CompilerConfigJson(source) => Some(source),
            Self::PolicyIngress(source) => Some(source),
            Self::Evaluation { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub type HostResult<T> = Result<T, HostError>;
