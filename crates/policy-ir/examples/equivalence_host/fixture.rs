use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use raziel_policy_ir::{
    CaveatInputSource, CaveatValue, CaveatValueType, ContextEntry, ObjectRef, PolicyLimits,
};
use serde::Deserialize;

use crate::error::{HostError, HostResult};
use crate::hash::decode_digest;

pub const MAX_FIXTURE_BYTES: u64 = 1_048_576;
pub const MAX_SCHEMA_BYTES: u64 = 1_048_576;
pub const MAX_CONFIG_BYTES: u64 = 1_048_576;
pub const MAX_POLICY_IR_BYTES: u64 = 4_194_304;

const MAX_RELATIONSHIPS: usize = 256;
const MAX_CASES: usize = 256;
const MAX_CONTEXT_ENTRIES: usize = 32;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_OBJECT_ID_BYTES: usize = 256;
const MAX_CASE_ID_BYTES: usize = 128;
const MAX_EXACT_JSON_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub fixture_version: u32,
    pub schema_path: String,
    pub compiler_config_path: String,
    pub policy_ir_path: String,
    pub schema_sha256: String,
    pub relationships: Vec<FixtureRelationship>,
    pub cases: Vec<FixtureCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureRelationship {
    pub resource: FixtureObject,
    pub relation: String,
    pub subject: FixtureObject,
    #[serde(default)]
    pub subject_relation: Option<String>,
    #[serde(default)]
    pub caveat: Option<FixtureCaveat>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureCaveat {
    pub name: String,
    pub context: Vec<FixtureContextEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureCase {
    pub id: String,
    pub resource: FixtureObject,
    pub permission: String,
    pub subject: FixtureObject,
    pub request_context: Vec<FixtureContextEntry>,
    pub capability_claims: Vec<FixtureContextEntry>,
    pub expected: ExpectedDecision,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct FixtureObject {
    pub object_type: String,
    pub object_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureContextEntry {
    pub name: String,
    pub value: FixtureValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FixtureValue {
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum ExpectedDecision {
    #[serde(rename = "ALLOW")]
    Allow,
    #[serde(rename = "DENY")]
    Deny,
    #[serde(rename = "CONDITIONAL")]
    Conditional,
}

pub struct ResolvedFixturePaths {
    pub schema: PathBuf,
    pub compiler_config: PathBuf,
    pub policy_ir: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilerConfig {
    pub config_version: u32,
    pub limits: CompilerLimits,
    pub caveats: BTreeMap<String, BTreeMap<String, CompilerInputSource>>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilerLimits {
    pub max_expression_nodes: u32,
    pub max_caveat_nodes: u32,
    pub max_path_depth: u32,
    pub max_relationship_witnesses: u32,
    pub max_merkle_depth: u32,
    pub max_string_bytes: u32,
    pub max_context_entries: u32,
    pub max_total_private_input_bytes: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompilerInputSource {
    RequestContext,
    RelationshipContext,
    CapabilityClaim,
}

impl Fixture {
    pub fn parse(bytes: &[u8]) -> HostResult<Self> {
        let fixture: Self = serde_json::from_slice(bytes).map_err(HostError::Json)?;
        fixture.validate_structure()?;
        Ok(fixture)
    }

    pub fn validate_structure(&self) -> HostResult<()> {
        if self.fixture_version != 1 {
            return Err(HostError::UnsupportedFixtureVersion(self.fixture_version));
        }
        if self.relationships.len() > MAX_RELATIONSHIPS {
            return Err(HostError::FixtureLimit("the relationship ceiling"));
        }
        if self.cases.is_empty() || self.cases.len() > MAX_CASES {
            return Err(HostError::FixtureLimit("the case ceiling"));
        }

        validate_relative_path(&self.schema_path, "schema_path")?;
        validate_relative_path(&self.compiler_config_path, "compiler_config_path")?;
        validate_relative_path(&self.policy_ir_path, "policy_ir_path")?;
        decode_digest(&self.schema_sha256)?;

        let mut relationship_keys = BTreeSet::new();
        for relationship in &self.relationships {
            validate_object(&relationship.resource)?;
            validate_identifier(&relationship.relation)?;
            validate_object(&relationship.subject)?;
            if let Some(subject_relation) = &relationship.subject_relation {
                validate_identifier(subject_relation)?;
            }
            if relationship.subject_relation.is_some() && relationship.caveat.is_some() {
                return Err(HostError::InvalidFixture(
                    "a relationship cannot combine a subject relation and caveat",
                ));
            }
            if let Some(caveat) = &relationship.caveat {
                validate_identifier(&caveat.name)?;
                validate_context(&caveat.context, &format!("caveat {}", caveat.name))?;
            }
            let key = (
                relationship.resource.clone(),
                relationship.relation.clone(),
                relationship.subject.clone(),
                relationship.subject_relation.clone(),
            );
            if !relationship_keys.insert(key) {
                return Err(HostError::DuplicateRelationship);
            }
        }

        for pair in self.cases.windows(2) {
            match pair[0].id.cmp(&pair[1].id) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => return Err(HostError::DuplicateCaseId),
                std::cmp::Ordering::Greater => return Err(HostError::CasesNotSorted),
            }
        }
        for case in &self.cases {
            validate_case_id(&case.id)?;
            validate_object(&case.resource)?;
            validate_object(&case.subject)?;
            validate_identifier(&case.permission)?;
            validate_context(&case.request_context, &format!("case {} request", case.id))?;
            validate_context(
                &case.capability_claims,
                &format!("case {} capability", case.id),
            )?;
        }
        Ok(())
    }
}

impl CompilerConfig {
    pub fn parse(bytes: &[u8]) -> HostResult<Self> {
        let config: Self = serde_json::from_slice(bytes).map_err(HostError::CompilerConfigJson)?;
        if config.config_version != 1 {
            return Err(HostError::UnsupportedCompilerConfigVersion(
                config.config_version,
            ));
        }
        Ok(config)
    }
}

impl CompilerLimits {
    pub const fn to_policy_limits(self) -> PolicyLimits {
        PolicyLimits {
            max_expression_nodes: self.max_expression_nodes,
            max_caveat_nodes: self.max_caveat_nodes,
            max_path_depth: self.max_path_depth,
            max_relationship_witnesses: self.max_relationship_witnesses,
            max_merkle_depth: self.max_merkle_depth,
            max_string_bytes: self.max_string_bytes,
            max_context_entries: self.max_context_entries,
            max_total_private_input_bytes: self.max_total_private_input_bytes,
        }
    }
}

impl CompilerInputSource {
    pub const fn to_policy_source(self) -> CaveatInputSource {
        match self {
            Self::RequestContext => CaveatInputSource::RequestContext,
            Self::RelationshipContext => CaveatInputSource::RelationshipContext,
            Self::CapabilityClaim => CaveatInputSource::CapabilityClaim,
        }
    }
}

impl FixtureObject {
    pub fn to_policy_object(&self) -> ObjectRef {
        ObjectRef {
            object_type: self.object_type.clone(),
            object_id: self.object_id.clone(),
        }
    }
}

impl FixtureContextEntry {
    pub fn to_policy_entry(&self) -> ContextEntry {
        ContextEntry {
            name: self.name.clone(),
            value: self.value.to_policy_value(),
        }
    }
}

impl FixtureValue {
    pub fn value_type(&self) -> CaveatValueType {
        match self {
            Self::Bool(_) => CaveatValueType::Bool,
            Self::Int(_) => CaveatValueType::Int,
            Self::String(_) => CaveatValueType::String,
        }
    }

    fn to_policy_value(&self) -> CaveatValue {
        match self {
            Self::Bool(value) => CaveatValue::Bool(*value),
            Self::Int(value) => CaveatValue::Int(*value),
            Self::String(value) => CaveatValue::String(value.clone()),
        }
    }
}

impl ExpectedDecision {
    pub const fn from_bool(value: bool) -> Self {
        if value { Self::Allow } else { Self::Deny }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
            Self::Conditional => "CONDITIONAL",
        }
    }
}

pub fn repository_root() -> HostResult<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or(HostError::PathNotConfined("repository root"))?;
    fs::canonicalize(root).map_err(|source| HostError::Io {
        action: "resolve",
        artifact: "repository root",
        source,
    })
}

pub fn read_fixture(root: &Path, relative_path: &Path) -> HostResult<Fixture> {
    let path = resolve_confined_file(root, relative_path, "--fixture")?;
    let bytes = read_bounded(&path, "fixture", MAX_FIXTURE_BYTES)?;
    Fixture::parse(&bytes)
}

pub fn resolve_fixture_paths(root: &Path, fixture: &Fixture) -> HostResult<ResolvedFixturePaths> {
    Ok(ResolvedFixturePaths {
        schema: resolve_confined_file(root, Path::new(&fixture.schema_path), "schema_path")?,
        compiler_config: resolve_confined_file(
            root,
            Path::new(&fixture.compiler_config_path),
            "compiler_config_path",
        )?,
        policy_ir: resolve_confined_file(
            root,
            Path::new(&fixture.policy_ir_path),
            "policy_ir_path",
        )?,
    })
}

pub fn read_bounded(path: &Path, artifact: &'static str, limit: u64) -> HostResult<Vec<u8>> {
    let file = File::open(path).map_err(|source| HostError::Io {
        action: "open",
        artifact,
        source,
    })?;
    let metadata = file.metadata().map_err(|source| HostError::Io {
        action: "inspect",
        artifact,
        source,
    })?;
    if !metadata.is_file() {
        return Err(HostError::PathNotConfined(artifact));
    }
    if metadata.len() > limit {
        return Err(HostError::FixtureLimit(artifact));
    }
    let mut bytes = Vec::with_capacity(metadata.len().min(limit) as usize);
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| HostError::Io {
            action: "read",
            artifact,
            source,
        })?;
    if bytes.len() as u64 > limit {
        return Err(HostError::FixtureLimit(artifact));
    }
    Ok(bytes)
}

fn resolve_confined_file(root: &Path, path: &Path, field: &'static str) -> HostResult<PathBuf> {
    validate_path_components(path, field)?;
    let resolved = fs::canonicalize(root.join(path)).map_err(|source| HostError::Io {
        action: "resolve",
        artifact: field,
        source,
    })?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(HostError::PathNotConfined(field));
    }
    Ok(resolved)
}

fn validate_relative_path(value: &str, field: &'static str) -> HostResult<()> {
    validate_path_components(Path::new(value), field)
}

fn validate_path_components(path: &Path, field: &'static str) -> HostResult<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(HostError::PathNotConfined(field));
    }
    Ok(())
}

fn validate_case_id(value: &str) -> HostResult<()> {
    if value.is_empty()
        || value.len() > MAX_CASE_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(HostError::InvalidCaseId);
    }
    Ok(())
}

fn validate_object(value: &FixtureObject) -> HostResult<()> {
    validate_identifier(&value.object_type)?;
    if value.object_id.is_empty() || value.object_id.len() > MAX_OBJECT_ID_BYTES {
        return Err(HostError::InvalidFixture("object ID is empty or too long"));
    }
    Ok(())
}

fn validate_identifier(value: &str) -> HostResult<()> {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(HostError::InvalidFixture("identifier is empty"));
    };
    if value.len() > MAX_IDENTIFIER_BYTES
        || !first.is_ascii_lowercase()
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(HostError::InvalidFixture("identifier syntax is invalid"));
    }
    Ok(())
}

fn validate_context(entries: &[FixtureContextEntry], scope: &str) -> HostResult<()> {
    if entries.len() > MAX_CONTEXT_ENTRIES {
        return Err(HostError::FixtureLimit("the context-entry ceiling"));
    }
    if entries.windows(2).any(|pair| pair[0].name >= pair[1].name) {
        return Err(HostError::ContextNotSorted(scope.to_owned()));
    }
    for entry in entries {
        validate_identifier(&entry.name)?;
        if matches!(
            entry.value,
            FixtureValue::Int(value)
                if !(-MAX_EXACT_JSON_INTEGER..=MAX_EXACT_JSON_INTEGER).contains(&value)
        ) {
            return Err(HostError::FixtureLimit("the exact JSON integer range"));
        }
        if matches!(&entry.value, FixtureValue::String(value) if value.len() > MAX_OBJECT_ID_BYTES)
        {
            return Err(HostError::FixtureLimit("the string-value ceiling"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_JSON: &[u8] = include_bytes!("../../../../tests/equivalence/fixtures/v1.json");

    fn fixture() -> Fixture {
        Fixture::parse(FIXTURE_JSON).unwrap()
    }

    #[test]
    fn parses_strict_typed_fixture() {
        let fixture = fixture();
        assert_eq!(fixture.fixture_version, 1);
        assert_eq!(fixture.cases.len(), 21);

        let mut json: serde_json::Value = serde_json::from_slice(FIXTURE_JSON).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("unknown".into(), true.into());
        assert!(matches!(
            Fixture::parse(&serde_json::to_vec(&json).unwrap()),
            Err(HostError::Json(_))
        ));

        json.as_object_mut().unwrap().remove("unknown");
        json["cases"][0]["request_context"][0]["value"] = serde_json::json!({"float": 1.5});
        assert!(matches!(
            Fixture::parse(&serde_json::to_vec(&json).unwrap()),
            Err(HostError::Json(_))
        ));

        json["cases"][0]["request_context"][0]["value"] =
            serde_json::json!({"bool": "private-value"});
        let error = Fixture::parse(&serde_json::to_vec(&json).unwrap()).unwrap_err();
        assert!(!error.to_string().contains("private-value"));
    }

    #[test]
    fn rejects_version_case_order_and_duplicate_relationships() {
        let mut value = fixture();
        value.fixture_version = 2;
        assert!(matches!(
            value.validate_structure(),
            Err(HostError::UnsupportedFixtureVersion(2))
        ));

        let mut value = fixture();
        value.cases.swap(0, 1);
        assert!(matches!(
            value.validate_structure(),
            Err(HostError::CasesNotSorted)
        ));

        let mut value = fixture();
        value.relationships.push(value.relationships[0].clone());
        assert!(matches!(
            value.validate_structure(),
            Err(HostError::DuplicateRelationship)
        ));
    }

    #[test]
    fn rejects_unsorted_context_and_unconfined_paths() {
        let mut value = fixture();
        value.cases[0].request_context.swap(0, 1);
        assert!(matches!(
            value.validate_structure(),
            Err(HostError::ContextNotSorted(_))
        ));

        let mut value = fixture();
        value.schema_path = "../supported.zed".into();
        assert!(matches!(
            value.validate_structure(),
            Err(HostError::PathNotConfined("schema_path"))
        ));
    }

    #[test]
    fn rejects_integers_that_spicedb_cannot_represent_exactly() {
        let mut value = fixture();
        value.cases[0].request_context[0].value = FixtureValue::Int(MAX_EXACT_JSON_INTEGER + 1);
        assert!(matches!(
            value.validate_structure(),
            Err(HostError::FixtureLimit("the exact JSON integer range"))
        ));
    }

    #[test]
    fn bounded_read_rejects_content_over_the_limit() {
        let path = std::env::temp_dir().join(format!(
            "raziel-equivalence-bounded-read-{}",
            std::process::id()
        ));
        fs::write(&path, vec![0_u8; 33]).unwrap();
        let result = read_bounded(&path, "test input", 32);
        fs::remove_file(path).unwrap();
        assert!(matches!(result, Err(HostError::FixtureLimit("test input"))));
    }
}
