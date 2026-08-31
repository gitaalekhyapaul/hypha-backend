//! Browser-safe transport projection for the existing Tonic capability API.

use std::fmt;
use std::io::Read;

use prost::Message;
use prost_types::{Duration, Timestamp};
use raziel_proto::raziel::v1::capability_service_client::CapabilityServiceClient;
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, CancelCapabilityRequest, CreateCapabilityRequest,
    GetCapabilityRequest, ProofCapability, ProofJob, ProofJobState, ProofSystem, PublicErrorCode,
    authorization_context_entry,
};
use serde::{Deserialize, Serialize};
use tonic::{Code, Status};

const MAX_INPUT_BYTES: u64 = 128 * 1024;
const DIGEST_BYTES: usize = 32;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityClientError {
    Configuration,
    InvalidRequest,
    Busy,
    NotFound,
    Unavailable,
    Protocol,
}

impl fmt::Display for CapabilityClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "configuration is invalid",
            Self::InvalidRequest => "request is invalid",
            Self::Busy => "proof capacity is occupied",
            Self::NotFound => "capability job was not found",
            Self::Unavailable => "capability service is unavailable",
            Self::Protocol => "capability service returned an invalid response",
        })
    }
}

impl std::error::Error for CapabilityClientError {}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum CapabilityCommand {
    Create { request: Box<CreateInput> },
    Get { job_id: String },
    Cancel { job_id: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    format_version: u32,
    agent_id: String,
    agent_public_key: String,
    task_id: String,
    purpose: String,
    action: String,
    resource_type: String,
    resource_scope: String,
    requested_expires_at_unix_seconds: String,
    issuance_nonce: String,
    snapshot_id: String,
    request_signature: String,
    #[serde(default)]
    request_context: Vec<ContextEntryInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEntryInput {
    name: String,
    value: ContextValueInput,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ContextValueInput {
    Bool { value: bool },
    Int { value: String },
    String { value: String },
}

#[derive(Debug, Serialize)]
struct CapabilityClientOutput {
    projection: JobProjection,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability_proto_hex: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobProjection {
    job_id: String,
    state: String,
    submitted_at: Option<PublicTimestamp>,
    updated_at: Option<PublicTimestamp>,
    public_error_code: String,
    metrics: Option<MetricsProjection>,
    capability: Option<CapabilityProjection>,
    mode: &'static str,
}

#[derive(Debug, Serialize)]
struct PublicTimestamp {
    seconds: String,
    nanos: i32,
}

#[derive(Debug, Serialize)]
struct MetricsProjection {
    execution_milliseconds: String,
    proving_milliseconds: String,
    execution_cycle_count: String,
    proof_size_bytes: String,
}

#[derive(Debug, Serialize)]
struct CapabilityProjection {
    capability_id: String,
    agent_key_thumbprint: String,
    task_id: String,
    purpose: String,
    action: String,
    resource_type: String,
    resource_scope: String,
    policy_hash: String,
    relationship_snapshot_root: String,
    snapshot_id: String,
    issued_at: Option<PublicTimestamp>,
    expires_at: Option<PublicTimestamp>,
    proof_system: String,
    verification_key_id: String,
    proof_size_bytes: String,
}

pub async fn run_from_env() -> Result<(), CapabilityClientError> {
    let endpoint = std::env::var("RAZIEL_RAZIELD_ENDPOINT")
        .map_err(|_| CapabilityClientError::Configuration)?;
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err(CapabilityClientError::Configuration);
    }
    let command = read_command()?;
    let mut client = CapabilityServiceClient::connect(endpoint)
        .await
        .map_err(|_| CapabilityClientError::Unavailable)?;
    let job = match command {
        CapabilityCommand::Create { request } => {
            client
                .create_capability(create_request(*request)?)
                .await
                .map_err(map_status)?
                .into_inner()
                .job
        }
        CapabilityCommand::Get { job_id } => {
            client
                .get_capability(GetCapabilityRequest {
                    job_id: bounded_identifier(job_id)?,
                })
                .await
                .map_err(map_status)?
                .into_inner()
                .job
        }
        CapabilityCommand::Cancel { job_id } => {
            client
                .cancel_capability(CancelCapabilityRequest {
                    job_id: bounded_identifier(job_id)?,
                })
                .await
                .map_err(map_status)?
                .into_inner()
                .job
        }
    }
    .ok_or(CapabilityClientError::Protocol)?;
    let output = project_job(job)?;
    let encoded = serde_json::to_vec(&output).map_err(|_| CapabilityClientError::Protocol)?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), &encoded)
        .map_err(|_| CapabilityClientError::Unavailable)
}

pub const fn exit_code(error: CapabilityClientError) -> i32 {
    match error {
        CapabilityClientError::Busy => 3,
        CapabilityClientError::NotFound => 4,
        _ => 2,
    }
}

fn map_status(status: Status) -> CapabilityClientError {
    match status.code() {
        Code::ResourceExhausted => CapabilityClientError::Busy,
        Code::NotFound => CapabilityClientError::NotFound,
        _ => CapabilityClientError::Unavailable,
    }
}

fn read_command() -> Result<CapabilityCommand, CapabilityClientError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CapabilityClientError::InvalidRequest)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(CapabilityClientError::InvalidRequest);
    }
    serde_json::from_slice(&bytes).map_err(|_| CapabilityClientError::InvalidRequest)
}

fn create_request(input: CreateInput) -> Result<CreateCapabilityRequest, CapabilityClientError> {
    let requested_expires_at = input
        .requested_expires_at_unix_seconds
        .parse::<i64>()
        .map_err(|_| CapabilityClientError::InvalidRequest)?;
    let request_context = input
        .request_context
        .into_iter()
        .map(|entry| {
            let value = match entry.value {
                ContextValueInput::Bool { value } => {
                    authorization_context_entry::Value::BoolValue(value)
                }
                ContextValueInput::Int { value } => authorization_context_entry::Value::IntValue(
                    value
                        .parse::<i64>()
                        .map_err(|_| CapabilityClientError::InvalidRequest)?,
                ),
                ContextValueInput::String { value } => {
                    authorization_context_entry::Value::StringValue(value)
                }
            };
            Ok(AuthorizationContextEntry {
                name: bounded_identifier(entry.name)?,
                value: Some(value),
            })
        })
        .collect::<Result<Vec<_>, CapabilityClientError>>()?;
    Ok(CreateCapabilityRequest {
        format_version: input.format_version,
        agent_id: bounded_identifier(input.agent_id)?,
        agent_public_key: decode_hex(&input.agent_public_key, ED25519_PUBLIC_KEY_BYTES)?,
        task_id: bounded_identifier(input.task_id)?,
        purpose: bounded_identifier(input.purpose)?,
        action: bounded_identifier(input.action)?,
        resource_type: bounded_identifier(input.resource_type)?,
        resource_scope: bounded_identifier(input.resource_scope)?,
        requested_expires_at: Some(Timestamp {
            seconds: requested_expires_at,
            nanos: 0,
        }),
        issuance_nonce: decode_hex(&input.issuance_nonce, DIGEST_BYTES)?,
        snapshot_id: bounded_identifier(input.snapshot_id)?,
        request_signature: decode_hex(&input.request_signature, ED25519_SIGNATURE_BYTES)?,
        request_context,
    })
}

fn project_job(job: ProofJob) -> Result<CapabilityClientOutput, CapabilityClientError> {
    let state = ProofJobState::try_from(job.state).map_err(|_| CapabilityClientError::Protocol)?;
    if state == ProofJobState::Unspecified || job.job_id.is_empty() {
        return Err(CapabilityClientError::Protocol);
    }
    let public_error_code = PublicErrorCode::try_from(job.public_error_code)
        .map_err(|_| CapabilityClientError::Protocol)?;
    let capability_proto_hex = job
        .capability
        .as_ref()
        .map(|capability| lowercase_hex(&capability.encode_to_vec()));
    let capability = job.capability.map(project_capability).transpose()?;
    Ok(CapabilityClientOutput {
        projection: JobProjection {
            job_id: job.job_id,
            state: state.as_str_name().to_owned(),
            submitted_at: job.submitted_at.map(public_timestamp),
            updated_at: job.updated_at.map(public_timestamp),
            public_error_code: public_error_code.as_str_name().to_owned(),
            metrics: job.metrics.map(project_metrics),
            capability,
            mode: "live",
        },
        capability_proto_hex,
    })
}

fn project_capability(
    capability: ProofCapability,
) -> Result<CapabilityProjection, CapabilityClientError> {
    let proof_system = ProofSystem::try_from(capability.proof_system)
        .map_err(|_| CapabilityClientError::Protocol)?;
    Ok(CapabilityProjection {
        capability_id: capability.capability_id,
        agent_key_thumbprint: lowercase_hex(&capability.agent_key_thumbprint),
        task_id: capability.task_id,
        purpose: capability.purpose,
        action: capability.action,
        resource_type: capability.resource_type,
        resource_scope: capability.resource_scope,
        policy_hash: lowercase_hex(&capability.policy_hash),
        relationship_snapshot_root: lowercase_hex(&capability.relationship_snapshot_root),
        snapshot_id: capability.snapshot_id,
        issued_at: capability.issued_at.map(public_timestamp),
        expires_at: capability.expires_at.map(public_timestamp),
        proof_system: proof_system.as_str_name().to_owned(),
        verification_key_id: capability.verification_key_id,
        proof_size_bytes: capability.zk_proof.len().to_string(),
    })
}

fn project_metrics(metrics: raziel_proto::raziel::v1::ProofMetrics) -> MetricsProjection {
    MetricsProjection {
        execution_milliseconds: duration_milliseconds(metrics.execution_duration.as_ref()),
        proving_milliseconds: duration_milliseconds(metrics.proving_duration.as_ref()),
        execution_cycle_count: metrics.execution_cycle_count.to_string(),
        proof_size_bytes: metrics.proof_size_bytes.to_string(),
    }
}

fn public_timestamp(timestamp: Timestamp) -> PublicTimestamp {
    PublicTimestamp {
        seconds: timestamp.seconds.to_string(),
        nanos: timestamp.nanos,
    }
}

fn duration_milliseconds(duration: Option<&Duration>) -> String {
    duration
        .map(|value| value.seconds.saturating_mul(1_000) + i64::from(value.nanos) / 1_000_000)
        .unwrap_or_default()
        .to_string()
}

fn bounded_identifier(value: String) -> Result<String, CapabilityClientError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 256 || trimmed.chars().any(char::is_control) {
        return Err(CapabilityClientError::InvalidRequest);
    }
    Ok(trimmed.to_owned())
}

fn decode_hex(value: &str, expected_bytes: usize) -> Result<Vec<u8>, CapabilityClientError> {
    if value.len() != expected_bytes * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CapabilityClientError::InvalidRequest);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text =
                std::str::from_utf8(pair).map_err(|_| CapabilityClientError::InvalidRequest)?;
            u8::from_str_radix(text, 16).map_err(|_| CapabilityClientError::InvalidRequest)
        })
        .collect()
}

fn lowercase_hex(bytes: &[u8]) -> String {
    use fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_capacity_has_a_distinct_process_result() {
        let busy = map_status(Status::resource_exhausted("redacted"));
        assert_eq!(busy, CapabilityClientError::Busy);
        assert_eq!(exit_code(busy), 3);
        let not_found = map_status(Status::not_found("redacted"));
        assert_eq!(not_found, CapabilityClientError::NotFound);
        assert_eq!(exit_code(not_found), 4);
        assert_eq!(
            map_status(Status::unavailable("redacted")),
            CapabilityClientError::Unavailable
        );
        assert_eq!(exit_code(CapabilityClientError::Unavailable), 2);
    }
}
