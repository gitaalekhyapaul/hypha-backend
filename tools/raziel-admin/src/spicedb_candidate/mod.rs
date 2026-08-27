//! Owner-only publication of one approved policy candidate into SpiceDB.

mod client;
mod store;
mod wire;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use prost_types::{Struct, Value, value::Kind};
use raziel_policy_ir::CaveatValue;
use raziel_snapshot::RelationshipRecord;
use spicedb_rs_proto::authzed::api::v1 as authzed;

use client::SpiceDbCandidateWriter;
use store::{
    configured_path, load_existing_handoff, persist_relationship_batch, prepare_candidate,
    read_bearer_token, read_bound_handoff, trusted_now, write_handoff,
};

const RELATIONSHIP_BATCH_FORMAT_VERSION: u32 = 1;
const REVISION_HANDOFF_FORMAT_VERSION: u32 = 1;
const MAX_SPICEDB_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXACT_JSON_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone)]
struct CandidateConfig {
    candidate_root: PathBuf,
    registry_root: PathBuf,
    token_path: PathBuf,
    trust_domain_id: String,
    endpoint: String,
}

struct PreparedCandidate {
    candidate_id: String,
    policy_record_id: String,
    policy_hash: [u8; 32],
    resource_type: String,
    schema: String,
    source_schema_digest: [u8; 32],
    relationship_batch_digest: [u8; 32],
    relationships: Vec<RelationshipRecord>,
    accepted_relationship_count: u32,
    handoff_path: PathBuf,
    candidate_owner: u32,
}

impl fmt::Debug for PreparedCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCandidate")
            .field("candidate_id", &self.candidate_id)
            .field("policy_record_id", &self.policy_record_id)
            .field("resource_type", &self.resource_type)
            .field(
                "accepted_relationship_count",
                &self.accepted_relationship_count,
            )
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateWriteResult {
    pub candidate_id: String,
    pub policy_record_id: String,
    pub resource_type: String,
    pub accepted_relationship_count: u32,
    policy_hash: [u8; 32],
}

impl CandidateWriteResult {
    pub const fn policy_hash(&self) -> &[u8; 32] {
        &self.policy_hash
    }
}

#[derive(Clone)]
pub struct CandidateRevisionHandoffReader {
    candidate_root: PathBuf,
}

impl CandidateRevisionHandoffReader {
    pub fn new(candidate_root: impl Into<PathBuf>) -> Result<Self, CandidateRevisionReaderError> {
        let candidate_root = candidate_root.into();
        if !candidate_root.is_absolute()
            || candidate_root == std::path::Path::new("/")
            || candidate_root.as_os_str().as_encoded_bytes().contains(&0)
        {
            return Err(CandidateRevisionReaderError::InvalidConfiguration);
        }
        Ok(Self { candidate_root })
    }

    pub fn read_bound(
        &self,
        candidate_id: &str,
        expected_policy_record_id: &str,
        expected_policy_hash: &[u8; 32],
    ) -> Result<CandidateRevision, CandidateRevisionReaderError> {
        let zed_token = read_bound_handoff(
            &self.candidate_root,
            candidate_id,
            expected_policy_record_id,
            expected_policy_hash,
        )
        .map_err(|_| CandidateRevisionReaderError::Rejected)?;
        Ok(CandidateRevision { zed_token })
    }
}

impl fmt::Debug for CandidateRevisionHandoffReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateRevisionHandoffReader")
            .field("candidate_root", &"<redacted>")
            .finish()
    }
}

pub struct CandidateRevision {
    zed_token: String,
}

impl CandidateRevision {
    pub fn zed_token(&self) -> &str {
        &self.zed_token
    }
}

impl fmt::Debug for CandidateRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateRevision")
            .field("zed_token", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRevisionReaderError {
    InvalidConfiguration,
    Rejected,
}

impl fmt::Display for CandidateRevisionReaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("private candidate revision could not be loaded")
    }
}

impl Error for CandidateRevisionReaderError {}

pub async fn run_from_env() -> Result<(), CandidateWriteError> {
    let candidate_id = parse_candidate_id(std::env::args())?;
    let result = publish_candidate_from_env(&candidate_id).await?;
    print_result(&result);
    Ok(())
}

pub fn write_relationship_batch(
    candidate_root: &std::path::Path,
    candidate_id: &str,
    relationships: Vec<RelationshipRecord>,
) -> Result<(), CandidateWriteError> {
    persist_relationship_batch(candidate_root, candidate_id, relationships)
}

pub async fn publish_candidate_from_env(
    candidate_id: &str,
) -> Result<CandidateWriteResult, CandidateWriteError> {
    let config = CandidateConfig {
        candidate_root: configured_path("RAZIEL_POLICY_CANDIDATE_ROOT")?,
        registry_root: configured_path("RAZIEL_REGISTRY_PATH")?,
        token_path: configured_path("RAZIEL_SPICEDB_TOKEN_PATH")?,
        trust_domain_id: required_env("RAZIEL_TRUST_DOMAIN_ID")?,
        endpoint: required_env("RAZIEL_SPICEDB_ENDPOINT")?,
    };
    let now = trusted_now()?;
    let prepared = prepare_candidate(&config, candidate_id, now)?;
    let result = if load_existing_handoff(&prepared)?.is_some() {
        result_from(&prepared)
    } else {
        let token = read_bearer_token(&config.token_path)?;
        let mut writer = SpiceDbCandidateWriter::connect(&config.endpoint, &token).await?;
        write_prepared_candidate(&mut writer, &prepared).await?
    };
    Ok(result)
}

#[tonic::async_trait]
trait SpiceDbWriteApi {
    async fn write_schema(
        &mut self,
        request: authzed::WriteSchemaRequest,
    ) -> Result<authzed::WriteSchemaResponse, CandidateWriteError>;

    async fn write_relationships(
        &mut self,
        request: authzed::WriteRelationshipsRequest,
    ) -> Result<authzed::WriteRelationshipsResponse, CandidateWriteError>;
}

async fn write_prepared_candidate(
    writer: &mut impl SpiceDbWriteApi,
    prepared: &PreparedCandidate,
) -> Result<CandidateWriteResult, CandidateWriteError> {
    if let Some(_stored) = load_existing_handoff(prepared)? {
        return Ok(result_from(prepared));
    }
    let schema_response = writer
        .write_schema(authzed::WriteSchemaRequest {
            schema: prepared.schema.clone(),
        })
        .await?;
    require_revision(schema_response.written_at)?;

    let updates = prepared
        .relationships
        .iter()
        .map(relationship_update)
        .collect::<Result<Vec<_>, _>>()?;
    let relationship_response = writer
        .write_relationships(authzed::WriteRelationshipsRequest {
            updates,
            optional_preconditions: Vec::new(),
            optional_transaction_metadata: None,
        })
        .await?;
    let final_revision = require_revision(relationship_response.written_at)?;
    write_handoff(prepared, final_revision)?;
    Ok(result_from(prepared))
}

fn relationship_update(
    value: &RelationshipRecord,
) -> Result<authzed::RelationshipUpdate, CandidateWriteError> {
    let optional_caveat = value
        .stored_caveat
        .as_ref()
        .map(|caveat| {
            let fields = caveat
                .context
                .iter()
                .map(|entry| {
                    let kind = match &entry.value {
                        CaveatValue::Bool(value) => Kind::BoolValue(*value),
                        CaveatValue::Int(value)
                            if value.unsigned_abs() <= MAX_EXACT_JSON_INTEGER as u64 =>
                        {
                            Kind::NumberValue(*value as f64)
                        }
                        CaveatValue::Int(_) => return Err(CandidateWriteError::CandidateRejected),
                        CaveatValue::String(value) => Kind::StringValue(value.clone()),
                    };
                    Ok((entry.name.clone(), Value { kind: Some(kind) }))
                })
                .collect::<Result<_, _>>()?;
            Ok(authzed::ContextualizedCaveat {
                caveat_name: caveat.name.clone(),
                context: Some(Struct { fields }),
            })
        })
        .transpose()?;
    Ok(authzed::RelationshipUpdate {
        operation: authzed::relationship_update::Operation::Touch as i32,
        relationship: Some(authzed::Relationship {
            resource: Some(authzed::ObjectReference {
                object_type: value.resource_type.clone(),
                object_id: value.resource_id.clone(),
            }),
            relation: value.relation.clone(),
            subject: Some(authzed::SubjectReference {
                object: Some(authzed::ObjectReference {
                    object_type: value.subject_type.clone(),
                    object_id: value.subject_id.clone(),
                }),
                optional_relation: value.subject_relation.clone().unwrap_or_default(),
            }),
            optional_caveat,
            optional_expires_at: None,
        }),
    })
}

fn require_revision(token: Option<authzed::ZedToken>) -> Result<String, CandidateWriteError> {
    token
        .map(|token| token.token)
        .filter(|token| !token.is_empty() && token.len() <= 4096)
        .ok_or(CandidateWriteError::MissingRevision)
}

fn parse_candidate_id(
    mut values: impl Iterator<Item = String>,
) -> Result<String, CandidateWriteError> {
    let _program = values.next();
    if values.next().as_deref() != Some("--candidate-id") {
        return Err(CandidateWriteError::Usage);
    }
    let candidate_id = values.next().ok_or(CandidateWriteError::Usage)?;
    if values.next().is_some() {
        return Err(CandidateWriteError::Usage);
    }
    Ok(candidate_id)
}

fn required_env(name: &str) -> Result<String, CandidateWriteError> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty() && value.trim() == value && !value.contains('\0'))
        .ok_or(CandidateWriteError::InvalidConfiguration)
}

fn result_from(prepared: &PreparedCandidate) -> CandidateWriteResult {
    CandidateWriteResult {
        candidate_id: prepared.candidate_id.clone(),
        policy_record_id: prepared.policy_record_id.clone(),
        resource_type: prepared.resource_type.clone(),
        accepted_relationship_count: prepared.accepted_relationship_count,
        policy_hash: prepared.policy_hash,
    }
}

fn print_result(result: &CandidateWriteResult) {
    println!("candidate_id={}", result.candidate_id);
    println!("policy_record_id={}", result.policy_record_id);
    println!("resource_type={}", result.resource_type);
    println!(
        "accepted_relationships={}",
        result.accepted_relationship_count
    );
    println!("status=spicedb_written");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateWriteError {
    Usage,
    InvalidConfiguration,
    InvalidCandidateId,
    CandidateRejected,
    RegistryRejected,
    InvalidCredential,
    ConnectionFailed,
    SchemaWriteFailed,
    RelationshipWriteFailed,
    MissingRevision,
    HandoffFailed,
    Conflict,
    Unavailable,
}

impl fmt::Display for CandidateWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Usage => "expected --candidate-id <id>",
            Self::InvalidConfiguration => "SpiceDB candidate configuration is invalid",
            Self::InvalidCandidateId => "candidate identifier is invalid",
            Self::CandidateRejected => "SpiceDB candidate input was rejected",
            Self::RegistryRejected => "approved policy registry rejected the candidate",
            Self::InvalidCredential => "SpiceDB owner credential is invalid",
            Self::ConnectionFailed => "SpiceDB connection failed",
            Self::SchemaWriteFailed => "SpiceDB schema write failed",
            Self::RelationshipWriteFailed => "SpiceDB relationship write failed",
            Self::MissingRevision => "SpiceDB mutation returned no revision",
            Self::HandoffFailed => "private SpiceDB revision handoff failed",
            Self::Conflict => "relationship batch conflicts with the stored candidate",
            Self::Unavailable => "trusted local state is unavailable",
        })
    }
}

impl Error for CandidateWriteError {}
