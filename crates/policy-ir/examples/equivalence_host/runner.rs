use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use prost::Message;
use raziel_policy_ir::{
    CaveatDefinition, CaveatInputSource, CaveatValueType, ContextEntry, EvaluationError,
    EvaluationRequest, ObjectRef, PolicyLimits, PolicyProgram, RelationshipCaveat,
    RelationshipResolver, ResolvedRelationship, ResolverError, ValidatedPolicyProgram,
    ValidationCeilings, evaluate,
};
use raziel_proto::raziel::v1 as proto;

use crate::error::{HostError, HostResult};
use crate::fixture::{
    CompilerConfig, ExpectedDecision, Fixture, FixtureContextEntry, FixtureRelationship,
    MAX_CONFIG_BYTES, MAX_POLICY_IR_BYTES, MAX_SCHEMA_BYTES, read_bounded, read_fixture,
    resolve_fixture_paths,
};
use crate::hash::{decode_digest, sha256};

const TRUSTED_POLICY_CEILINGS: ValidationCeilings = ValidationCeilings {
    policy_limits: PolicyLimits {
        max_expression_nodes: 64,
        max_caveat_nodes: 64,
        max_path_depth: 4,
        max_relationship_witnesses: 32,
        max_merkle_depth: 32,
        max_string_bytes: 256,
        max_context_entries: 16,
        max_total_private_input_bytes: 4096,
    },
    max_definitions: 16,
    max_relations_per_definition: 16,
    max_allowed_subject_types_per_relation: 16,
    max_permissions: 32,
    max_caveats: 16,
    max_parameters_per_caveat: 16,
    max_source_schema_digest_bytes: 32,
    max_syntax_depth: 16,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseOutcome {
    pub id: String,
    pub decision: ExpectedDecision,
}

#[derive(Default)]
struct FixtureResolver {
    relationships: BTreeMap<(ObjectRef, String), Vec<ResolvedRelationship>>,
}

impl RelationshipResolver for FixtureResolver {
    fn resolve<'a>(
        &'a self,
        resource: &ObjectRef,
        relation: &str,
    ) -> Result<&'a [ResolvedRelationship], ResolverError> {
        Ok(self
            .relationships
            .get(&(resource.clone(), relation.to_owned()))
            .map(Vec::as_slice)
            .unwrap_or(&[]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputSpec {
    source: CaveatInputSource,
    value_type: CaveatValueType,
}

pub fn run(root: &Path, fixture_path: &Path) -> HostResult<Vec<CaseOutcome>> {
    let fixture = read_fixture(root, fixture_path)?;
    evaluate_fixture(root, &fixture)
}

pub fn evaluate_fixture(root: &Path, fixture: &Fixture) -> HostResult<Vec<CaseOutcome>> {
    fixture.validate_structure()?;
    let paths = resolve_fixture_paths(root, fixture)?;
    let schema = read_bounded(&paths.schema, "schema", MAX_SCHEMA_BYTES)?;
    let compiler_config = CompilerConfig::parse(&read_bounded(
        &paths.compiler_config,
        "compiler config",
        MAX_CONFIG_BYTES,
    )?)?;
    let policy_ir = read_bounded(&paths.policy_ir, "PolicyIR", MAX_POLICY_IR_BYTES)?;

    let expected_digest = decode_digest(&fixture.schema_sha256)?;
    if sha256(&schema) != expected_digest {
        return Err(HostError::SchemaDigestMismatch("schema bytes"));
    }

    let wire =
        proto::PolicyProgram::decode(policy_ir.as_slice()).map_err(HostError::PolicyDecode)?;
    let program = ValidatedPolicyProgram::from_proto(wire, &TRUSTED_POLICY_CEILINGS)
        .map_err(HostError::PolicyValidation)?;
    if program.program().source_schema_digest.as_slice() != expected_digest {
        return Err(HostError::SchemaDigestMismatch(
            "PolicyIR source_schema_digest",
        ));
    }

    validate_compiler_config(&compiler_config, program.program())?;
    validate_fixture_against_policy(fixture, program.program())?;
    let resolver = FixtureResolver::from_fixture(fixture);
    evaluate_cases(fixture, &program, &resolver)
}

fn validate_compiler_config(config: &CompilerConfig, program: &PolicyProgram) -> HostResult<()> {
    if config.limits.to_policy_limits() != program.limits {
        return Err(HostError::CompilerConfigMismatch("limits"));
    }
    if config.caveats.len() != program.caveats.len() {
        return Err(HostError::CompilerConfigMismatch("caveat sets"));
    }
    for caveat in &program.caveats {
        let configured = config
            .caveats
            .get(&caveat.name)
            .ok_or(HostError::CompilerConfigMismatch("caveat sets"))?;
        if configured.len() != caveat.parameters.len() {
            return Err(HostError::CompilerConfigMismatch("caveat parameters"));
        }
        for parameter in &caveat.parameters {
            let source = configured
                .get(&parameter.name)
                .ok_or(HostError::CompilerConfigMismatch("caveat parameters"))?;
            if source.to_policy_source() != parameter.source {
                return Err(HostError::CompilerConfigMismatch("parameter provenance"));
            }
        }
    }
    Ok(())
}

pub fn write_outcomes(outcomes: &[CaseOutcome], output: Option<&Path>) -> HostResult<()> {
    let rendered = render_outcomes(outcomes);
    if let Some(path) = output {
        fs::write(path, rendered).map_err(|source| HostError::Io {
            action: "write",
            artifact: "equivalence output",
            source,
        })
    } else {
        io::stdout()
            .lock()
            .write_all(rendered.as_bytes())
            .map_err(|source| HostError::Io {
                action: "write",
                artifact: "standard output",
                source,
            })
    }
}

fn render_outcomes(outcomes: &[CaseOutcome]) -> String {
    let mut rendered = String::new();
    for outcome in outcomes {
        rendered.push_str(&outcome.id);
        rendered.push('\t');
        rendered.push_str(outcome.decision.as_str());
        rendered.push('\n');
    }
    rendered
}

impl FixtureResolver {
    fn from_fixture(fixture: &Fixture) -> Self {
        let mut resolver = Self::default();
        for relationship in &fixture.relationships {
            resolver
                .relationships
                .entry((
                    relationship.resource.to_policy_object(),
                    relationship.relation.clone(),
                ))
                .or_default()
                .push(to_resolved_relationship(relationship));
        }
        for relationships in resolver.relationships.values_mut() {
            relationships.sort_by(compare_resolved_relationships);
        }
        resolver
    }
}

fn to_resolved_relationship(value: &FixtureRelationship) -> ResolvedRelationship {
    ResolvedRelationship {
        subject: value.subject.to_policy_object(),
        subject_relation: value.subject_relation.clone(),
        caveat: value.caveat.as_ref().map(|caveat| RelationshipCaveat {
            name: caveat.name.clone(),
            context: to_context(&caveat.context),
        }),
        merkle_depth: 0,
        private_input_bytes: 0,
    }
}

fn compare_resolved_relationships(
    left: &ResolvedRelationship,
    right: &ResolvedRelationship,
) -> Ordering {
    left.subject
        .cmp(&right.subject)
        .then_with(|| left.subject_relation.cmp(&right.subject_relation))
        .then_with(|| {
            left.caveat
                .as_ref()
                .map(|caveat| &caveat.name)
                .cmp(&right.caveat.as_ref().map(|caveat| &caveat.name))
        })
}

fn evaluate_cases(
    fixture: &Fixture,
    program: &ValidatedPolicyProgram,
    resolver: &FixtureResolver,
) -> HostResult<Vec<CaseOutcome>> {
    let mut outcomes = Vec::with_capacity(fixture.cases.len());
    for case in &fixture.cases {
        let request = EvaluationRequest {
            resource: case.resource.to_policy_object(),
            subject: case.subject.to_policy_object(),
            permission: case.permission.clone(),
            request_context: to_context(&case.request_context),
            capability_claims: to_context(&case.capability_claims),
        };
        let actual = match evaluate(program, &request, resolver) {
            Ok(value) => ExpectedDecision::from_bool(value),
            Err(EvaluationError::MissingCaveatInput)
                if case.expected == ExpectedDecision::Conditional =>
            {
                ExpectedDecision::Conditional
            }
            Err(EvaluationError::MissingCaveatInput) => {
                return Err(HostError::MissingEvaluationInput(case.id.clone()));
            }
            Err(source) => {
                return Err(HostError::Evaluation {
                    case_id: case.id.clone(),
                    source,
                });
            }
        };
        if actual != case.expected {
            return Err(HostError::ExpectationMismatch {
                case_id: case.id.clone(),
                expected: case.expected.as_str(),
                actual: actual.as_str(),
            });
        }
        outcomes.push(CaseOutcome {
            id: case.id.clone(),
            decision: actual,
        });
    }
    Ok(outcomes)
}

fn validate_fixture_against_policy(fixture: &Fixture, program: &PolicyProgram) -> HostResult<()> {
    let mut inputs = BTreeMap::new();
    for caveat in &program.caveats {
        for parameter in &caveat.parameters {
            let specification = InputSpec {
                source: parameter.source,
                value_type: parameter.value_type,
            };
            if inputs
                .insert(parameter.name.clone(), specification)
                .is_some_and(|previous| previous != specification)
            {
                return Err(HostError::AmbiguousInputProvenance(parameter.name.clone()));
            }
        }
    }

    for relationship in &fixture.relationships {
        validate_relationship_shape(relationship, program)?;
        if let Some(caveat) = &relationship.caveat {
            let definition = program
                .caveats
                .iter()
                .find(|definition| definition.name == caveat.name)
                .ok_or_else(|| HostError::UnknownCaveat(caveat.name.clone()))?;
            validate_relationship_context(&caveat.context, definition)?;
        }
    }

    for case in &fixture.cases {
        if !program.permissions.iter().any(|permission| {
            permission.object_type == case.resource.object_type
                && permission.name == case.permission
        }) {
            return Err(HostError::MissingPermission(case.id.clone()));
        }
        validate_case_context(
            &case.request_context,
            CaveatInputSource::RequestContext,
            &inputs,
        )?;
        validate_case_context(
            &case.capability_claims,
            CaveatInputSource::CapabilityClaim,
            &inputs,
        )?;
    }
    Ok(())
}

fn validate_relationship_shape(
    relationship: &FixtureRelationship,
    program: &PolicyProgram,
) -> HostResult<()> {
    let relation = program
        .definitions
        .iter()
        .find(|definition| definition.name == relationship.resource.object_type)
        .and_then(|definition| {
            definition
                .relations
                .iter()
                .find(|relation| relation.name == relationship.relation)
        })
        .ok_or(HostError::InvalidRelationship)?;
    let caveat_name = relationship
        .caveat
        .as_ref()
        .map(|caveat| caveat.name.as_str());
    if !relation.allowed_subject_types.iter().any(|allowed| {
        allowed.object_type == relationship.subject.object_type
            && allowed.subject_relation.as_deref() == relationship.subject_relation.as_deref()
            && allowed.caveat_name.as_deref() == caveat_name
    }) {
        return Err(HostError::InvalidRelationship);
    }
    Ok(())
}

fn validate_relationship_context(
    entries: &[FixtureContextEntry],
    caveat: &CaveatDefinition,
) -> HostResult<()> {
    for entry in entries {
        let parameter = caveat
            .parameters
            .iter()
            .find(|parameter| parameter.name == entry.name)
            .ok_or_else(|| HostError::UnknownInput(entry.name.clone()))?;
        if parameter.source != CaveatInputSource::RelationshipContext {
            return Err(HostError::WrongInputProvenance(entry.name.clone()));
        }
        if parameter.value_type != entry.value.value_type() {
            return Err(HostError::InputTypeMismatch(entry.name.clone()));
        }
    }
    for parameter in caveat
        .parameters
        .iter()
        .filter(|parameter| parameter.source == CaveatInputSource::RelationshipContext)
    {
        if !entries.iter().any(|entry| entry.name == parameter.name) {
            return Err(HostError::MissingRelationshipInput {
                caveat: caveat.name.clone(),
                parameter: parameter.name.clone(),
            });
        }
    }
    Ok(())
}

fn validate_case_context(
    entries: &[FixtureContextEntry],
    source: CaveatInputSource,
    inputs: &BTreeMap<String, InputSpec>,
) -> HostResult<()> {
    for entry in entries {
        let specification = inputs
            .get(&entry.name)
            .ok_or_else(|| HostError::UnknownInput(entry.name.clone()))?;
        if specification.source != source {
            return Err(HostError::WrongInputProvenance(entry.name.clone()));
        }
        if specification.value_type != entry.value.value_type() {
            return Err(HostError::InputTypeMismatch(entry.name.clone()));
        }
    }
    Ok(())
}

fn to_context(entries: &[FixtureContextEntry]) -> Vec<ContextEntry> {
    entries
        .iter()
        .map(FixtureContextEntry::to_policy_entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{FixtureContextEntry, FixtureValue, repository_root};

    const FIXTURE_JSON: &[u8] = include_bytes!("../../../../tests/equivalence/fixtures/v1.json");

    fn fixture() -> Fixture {
        Fixture::parse(FIXTURE_JSON).unwrap()
    }

    fn evaluate(value: &Fixture) -> HostResult<Vec<CaseOutcome>> {
        evaluate_fixture(&repository_root().unwrap(), value)
    }

    #[test]
    fn evaluates_every_shared_case_and_renders_stable_lines() {
        let outcomes = evaluate(&fixture()).unwrap();
        assert_eq!(outcomes.len(), 21);
        let rendered = render_outcomes(&outcomes);
        assert!(rendered.starts_with("all-ops-allow-equal\tALLOW\n"));
        assert!(rendered.ends_with("view-subject-set-deny\tDENY\n"));
        assert!(rendered.contains("and-absorption-missing\tCONDITIONAL\n"));
        assert!(rendered.contains("or-absorption-missing\tCONDITIONAL\n"));
        assert!(!rendered.contains("approved"));
        assert!(!rendered.contains("1000"));
    }

    #[test]
    fn writes_stable_lines_to_requested_output_path() {
        let outcomes = evaluate(&fixture()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "raziel-equivalence-host-{}.tsv",
            std::process::id()
        ));

        write_outcomes(&outcomes, Some(&path)).unwrap();
        let rendered = fs::read_to_string(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert_eq!(rendered, render_outcomes(&outcomes));
    }

    #[test]
    fn rejects_schema_digest_drift_and_malformed_policy_ir() {
        let mut value = fixture();
        value.schema_sha256 = "0".repeat(64);
        assert!(matches!(
            evaluate(&value),
            Err(HostError::SchemaDigestMismatch("schema bytes"))
        ));

        let mut value = fixture();
        value.policy_ir_path = value.schema_path.clone();
        assert!(matches!(evaluate(&value), Err(HostError::PolicyDecode(_))));
    }

    #[test]
    fn rejects_compiler_limit_and_provenance_drift() {
        const CONFIG_JSON: &[u8] = include_bytes!(
            "../../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.json"
        );
        const POLICY_IR: &[u8] = include_bytes!(
            "../../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
        );
        let wire = proto::PolicyProgram::decode(POLICY_IR).unwrap();
        let program = ValidatedPolicyProgram::from_proto(wire, &TRUSTED_POLICY_CEILINGS).unwrap();

        let mut config = CompilerConfig::parse(CONFIG_JSON).unwrap();
        config.limits.max_path_depth += 1;
        assert!(matches!(
            validate_compiler_config(&config, program.program()),
            Err(HostError::CompilerConfigMismatch("limits"))
        ));

        let mut config = CompilerConfig::parse(CONFIG_JSON).unwrap();
        *config
            .caveats
            .get_mut("all_ops")
            .unwrap()
            .get_mut("a")
            .unwrap() = crate::fixture::CompilerInputSource::CapabilityClaim;
        assert!(matches!(
            validate_compiler_config(&config, program.program()),
            Err(HostError::CompilerConfigMismatch("parameter provenance"))
        ));
    }

    #[test]
    fn rejects_wrong_provenance_and_missing_relationship_input() {
        let mut value = fixture();
        let case = value
            .cases
            .iter_mut()
            .find(|case| case.id == "all-ops-allow-equal")
            .unwrap();
        let input = case.request_context.remove(0);
        case.capability_claims.insert(0, input);
        assert!(matches!(
            evaluate(&value),
            Err(HostError::WrongInputProvenance(name)) if name == "a"
        ));

        let mut value = fixture();
        value
            .relationships
            .iter_mut()
            .find_map(|relationship| relationship.caveat.as_mut())
            .unwrap()
            .context
            .clear();
        assert!(matches!(
            evaluate(&value),
            Err(HostError::MissingRelationshipInput { .. })
        ));
    }

    #[test]
    fn reports_missing_runtime_input_and_expectation_mismatch() {
        let mut value = fixture();
        value
            .cases
            .iter_mut()
            .find(|case| case.id == "view-caveat-allow")
            .unwrap()
            .request_context
            .clear();
        assert!(matches!(
            evaluate(&value),
            Err(HostError::MissingEvaluationInput(case_id)) if case_id == "view-caveat-allow"
        ));

        let mut value = fixture();
        value
            .cases
            .iter_mut()
            .find(|case| case.id == "view-caveat-allow")
            .unwrap()
            .capability_claims
            .clear();
        assert!(matches!(
            evaluate(&value),
            Err(HostError::MissingEvaluationInput(case_id)) if case_id == "view-caveat-allow"
        ));

        let mut value = fixture();
        value
            .cases
            .iter_mut()
            .find(|case| case.id == "view-direct-allow")
            .unwrap()
            .expected = ExpectedDecision::Deny;
        assert!(matches!(
            evaluate(&value),
            Err(HostError::ExpectationMismatch { case_id, .. }) if case_id == "view-direct-allow"
        ));
    }

    #[test]
    fn rejects_fixture_value_type_drift() {
        let mut value = fixture();
        let entry = value
            .cases
            .iter_mut()
            .find(|case| case.id == "all-ops-allow-equal")
            .unwrap()
            .request_context
            .iter_mut()
            .find(|entry| entry.name == "a")
            .unwrap();
        *entry = FixtureContextEntry {
            name: "a".into(),
            value: FixtureValue::String("ten".into()),
        };
        assert!(matches!(
            evaluate(&value),
            Err(HostError::InputTypeMismatch(name)) if name == "a"
        ));
    }
}
