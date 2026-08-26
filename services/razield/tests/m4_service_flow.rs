use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_GUEST_INPUT_VERSION, CapabilityRequestBindingV1, agent_subject,
    capability_request_commitment,
};
use raziel_canonical::{
    PROTECTED_REQUEST_FORMAT_VERSION, ProtectedRequestSignatureInputV1,
    protected_request_signature_message,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};
use raziel_proto::raziel::v1::capability_service_client::CapabilityServiceClient;
use raziel_proto::raziel::v1::snapshot_service_client::SnapshotServiceClient;
use raziel_proto::raziel::v1::verification_service_client::VerificationServiceClient;
use raziel_proto::raziel::v1::{
    AuthorizationContextEntry, BuildSnapshotRequest, CreateCapabilityRequest, GetCapabilityRequest,
    ProofCapability, ProofJobState, PublicErrorCode, VerificationOutcome, VerifyCapabilityRequest,
    authorization_context_entry,
};
use spicedb_rs_proto::authzed::api::v1 as authzed;
use spicedb_rs_proto::authzed::api::v1::permissions_service_client::PermissionsServiceClient;
use spicedb_rs_proto::authzed::api::v1::schema_service_client::SchemaServiceClient;
use tonic::Request;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, Endpoint};

const SCHEMA: &str = include_str!("fixtures/m4-service-flow/authorization.zed");
const PRIVATE_CONTEXT_VALUE: &str = "m4-private-context-4829";
const AGENT_SIGNING_KEY: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const STOLEN_SIGNING_KEY: [u8; 32] = [9; 32];
const REQUEST_VALIDITY_SECONDS: i64 = 3 * 60 * 60;
const SERVICE_CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
const PROOF_COMPLETION_TIMEOUT: Duration = Duration::from_secs(70 * 60);
const PROOF_POLL_INTERVAL: Duration = Duration::from_secs(5);
const PROOF_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const MAX_PUBLIC_CAPABILITY_BYTES: u64 = 2 * 1024 * 1024;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
#[ignore = "requires the B-0032 SpiceDB acceptance service"]
async fn m4_seed_spicedb_writes_the_agent_relationship_and_revision() -> TestResult {
    let endpoint = required_env("RAZIEL_M4_SPICEDB_ENDPOINT")?;
    let token = required_env("RAZIEL_M4_SPICEDB_TOKEN")?;
    let revision_path = required_path("RAZIEL_M4_ZED_TOKEN_OUTPUT_PATH")?;
    let channel = connect_channel(&endpoint).await?;
    let authorization = authorization(&token)?;
    let mut schema = SchemaServiceClient::new(channel.clone());
    let mut permissions = PermissionsServiceClient::new(channel);

    schema
        .write_schema(authenticated(
            authzed::WriteSchemaRequest {
                schema: SCHEMA.into(),
            },
            &authorization,
        ))
        .await?;

    let signing_key = agent_signing_key();
    let thumbprint = derive_ed25519_key_thumbprint(&signing_key.verifying_key().to_bytes());
    let subject = agent_subject(&thumbprint);
    let written = permissions
        .write_relationships(authenticated(
            authzed::WriteRelationshipsRequest {
                updates: vec![authzed::RelationshipUpdate {
                    operation: authzed::relationship_update::Operation::Touch as i32,
                    relationship: Some(authzed::Relationship {
                        resource: Some(authzed::ObjectReference {
                            object_type: "dataset".into(),
                            object_id: "payments".into(),
                        }),
                        relation: "viewer".into(),
                        subject: Some(authzed::SubjectReference {
                            object: Some(authzed::ObjectReference {
                                object_type: subject.object_type,
                                object_id: subject.object_id,
                            }),
                            optional_relation: String::new(),
                        }),
                        optional_caveat: None,
                        optional_expires_at: None,
                    }),
                }],
                optional_preconditions: Vec::new(),
                optional_transaction_metadata: None,
            },
            &authorization,
        ))
        .await?
        .into_inner();
    let revision = written
        .written_at
        .filter(|token| !token.token.is_empty())
        .ok_or_else(|| io::Error::other("SpiceDB returned no written revision"))?
        .token;
    write_new(&revision_path, revision.as_bytes())?;

    println!(
        "b0032_seed: schema_bytes={}, relationships=1, subject_type=agent",
        SCHEMA.len()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires the live B-0032 razield API without the proof worker"]
async fn m4_submit_signed_capability_returns_queued_and_is_idempotent() -> TestResult {
    let endpoint = required_env("RAZIEL_M4_RAZIELD_ENDPOINT")?;
    let policy_hash = decode_digest(&required_env("RAZIEL_M4_POLICY_HASH")?)?;
    let policy_record_id = required_env("RAZIEL_M4_POLICY_RECORD_ID")?;
    let authority_key_id = required_env("RAZIEL_M4_SNAPSHOT_AUTHORITY_KEY_ID")?;
    let job_path = required_path("RAZIEL_M4_JOB_ID_OUTPUT_PATH")?;
    let snapshot_root_path = required_path("RAZIEL_M4_SNAPSHOT_ROOT_OUTPUT_PATH")?;
    let channel = connect_channel(&endpoint).await?;
    let mut snapshots = SnapshotServiceClient::new(channel.clone());
    let mut capabilities = CapabilityServiceClient::new(channel);
    let now = unix_now()?;

    let snapshot = snapshots
        .build_snapshot(BuildSnapshotRequest {
            spicedb_endpoint_ref: "owner-spicedb".into(),
            zed_token_secret_ref: "reconciliation-revision".into(),
            policy_record_id,
            policy_hash: policy_hash.to_vec(),
            expires_at: Some(timestamp(now + REQUEST_VALIDITY_SECONDS)),
            expected_previous_epoch: 0,
            authority_key_id,
        })
        .await?
        .into_inner()
        .snapshot
        .ok_or_else(|| io::Error::other("BuildSnapshot returned no snapshot"))?;
    if snapshot.relationship_snapshot_root.len() != 32 {
        return Err(io::Error::other("BuildSnapshot returned an invalid root").into());
    }
    write_new(
        &snapshot_root_path,
        lowercase_hex(&snapshot.relationship_snapshot_root).as_bytes(),
    )?;

    let request = signed_capability_request(&snapshot.snapshot_id, now)?;
    let expected_job_id = lowercase_hex(&capability_request_commitment(&capability_binding(
        &request,
    )?)?);
    write_new(&job_path, expected_job_id.as_bytes())?;
    let first = capabilities
        .create_capability(request.clone())
        .await?
        .into_inner()
        .job
        .ok_or_else(|| io::Error::other("CreateCapability returned no job"))?;
    if first.job_id != expected_job_id
        || first.state != ProofJobState::Queued as i32
        || first.capability.is_some()
        || first.metrics.is_some()
        || first.internal_error_ref.is_some()
    {
        return Err(
            io::Error::other("first capability response was not a clean queued job").into(),
        );
    }

    let duplicate = capabilities
        .create_capability(request)
        .await?
        .into_inner()
        .job
        .ok_or_else(|| io::Error::other("duplicate CreateCapability returned no job"))?;
    if duplicate.job_id != first.job_id || duplicate.state != ProofJobState::Queued as i32 {
        return Err(io::Error::other("exact duplicate did not return the queued job").into());
    }
    println!(
        "b0032_submit: job_id={}, snapshot_id={}, state=queued, duplicate=same_job",
        first.job_id, snapshot.snapshot_id
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires the live B-0032 proof worker and one real local Plonk proof"]
async fn m4_wait_for_terminal_capability_records_public_result() -> TestResult {
    let endpoint = required_env("RAZIEL_M4_RAZIELD_ENDPOINT")?;
    let job_path = required_path("RAZIEL_M4_JOB_ID_PATH")?;
    let capability_path = required_path("RAZIEL_M4_CAPABILITY_OUTPUT_PATH")?;
    let job_id = read_job_id(&job_path)?;
    let channel = connect_channel(&endpoint).await?;
    let mut client = CapabilityServiceClient::new(channel);
    let started = Instant::now();
    let mut last_state = None;
    let mut next_heartbeat = PROOF_HEARTBEAT_INTERVAL;

    let terminal = loop {
        let job = client
            .get_capability(GetCapabilityRequest {
                job_id: job_id.clone(),
            })
            .await?
            .into_inner()
            .job
            .ok_or_else(|| io::Error::other("GetCapability returned no job"))?;
        let state = ProofJobState::try_from(job.state)
            .map_err(|_| io::Error::other("GetCapability returned an unknown state"))?;
        if last_state != Some(state) {
            println!("b0032_progress: state={}", state.as_str_name());
            last_state = Some(state);
        }
        if state == ProofJobState::Succeeded {
            break job;
        }
        if matches!(state, ProofJobState::Failed | ProofJobState::Canceled) {
            return Err(io::Error::other(format!(
                "proof job closed as {} with public code {}",
                state.as_str_name(),
                PublicErrorCode::try_from(job.public_error_code)
                    .map(|code| code.as_str_name())
                    .unwrap_or("UNKNOWN")
            ))
            .into());
        }
        if started.elapsed() >= PROOF_COMPLETION_TIMEOUT {
            return Err(io::Error::other("proof job did not finish within 70 minutes").into());
        }
        if started.elapsed() >= next_heartbeat {
            println!(
                "b0032_heartbeat: elapsed_seconds={}, state={}",
                started.elapsed().as_secs(),
                state.as_str_name()
            );
            next_heartbeat += PROOF_HEARTBEAT_INTERVAL;
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    };

    let capability = terminal
        .capability
        .ok_or_else(|| io::Error::other("succeeded job returned no capability"))?;
    let metrics = terminal
        .metrics
        .ok_or_else(|| io::Error::other("succeeded job returned no metrics"))?;
    if capability.capability_id != job_id || capability.zk_proof.is_empty() {
        return Err(io::Error::other("terminal capability did not match its job").into());
    }
    let capability_bytes = capability.encode_to_vec();
    if capability_bytes.len() as u64 > MAX_PUBLIC_CAPABILITY_BYTES {
        return Err(io::Error::other("public capability exceeded the acceptance limit").into());
    }
    write_new(&capability_path, &capability_bytes)?;

    println!(
        "b0032_issue: job_id={}, snapshot_id={}, policy_hash={}, relationship_root={}, proof_bytes={}, execute_ms={}, prove_ms={}, verification_key_id={}",
        job_id,
        capability.snapshot_id,
        lowercase_hex(&capability.policy_hash),
        lowercase_hex(&capability.relationship_snapshot_root),
        capability.zk_proof.len(),
        duration_millis(metrics.execution_duration.as_ref()),
        duration_millis(metrics.proving_duration.as_ref()),
        capability.verification_key_id
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires the live B-0032 verifier with every issuer and prover role stopped"]
async fn m4_independent_verifier_accepts_and_bound_mutations_fail() -> TestResult {
    let endpoint = required_env("RAZIEL_M4_VERIFIERD_ENDPOINT")?;
    let capability_path = required_path("RAZIEL_M4_CAPABILITY_PATH")?;
    let capability = read_capability(&capability_path)?;
    let channel = connect_channel(&endpoint).await?;
    let mut client = VerificationServiceClient::new(channel);

    let positive = protected_request(capability.clone(), agent_signing_key(), unix_now()?)?;
    let accepted = client
        .verify_capability(positive.clone())
        .await?
        .into_inner();
    if accepted.outcome != VerificationOutcome::Verified as i32
        || accepted.verified_claims.is_none()
        || accepted.failure_code != PublicErrorCode::Unspecified as i32
        || accepted.policy_record_id.is_empty()
        || accepted.snapshot_id != capability.snapshot_id
        || accepted.proof_digest.len() != 32
    {
        return Err(io::Error::other("independent verifier did not promote valid claims").into());
    }

    let mut altered_capability = capability.clone();
    altered_capability.action.push_str("-altered");
    let altered = protected_request(altered_capability, agent_signing_key(), unix_now()?)?;
    assert_rejected(
        client.verify_capability(altered).await?,
        "altered capability",
    )?;

    let stolen = protected_request(
        capability,
        SigningKey::from_bytes(&STOLEN_SIGNING_KEY),
        unix_now()?,
    )?;
    assert_rejected(
        client.verify_capability(stolen).await?,
        "stolen key binding",
    )?;

    println!(
        "b0032_verify: outcome=verified, altered_capability=reject, stolen_key_binding=reject"
    );
    Ok(())
}

fn signed_capability_request(snapshot_id: &str, now: i64) -> TestResult<CreateCapabilityRequest> {
    let signing_key = agent_signing_key();
    let mut request = CreateCapabilityRequest {
        format_version: AUTHORIZATION_GUEST_INPUT_VERSION,
        agent_id: "m4-acceptance-agent".into(),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        task_id: "m4-synthetic-reconciliation".into(),
        purpose: "reconciliation".into(),
        action: "access".into(),
        resource_type: "dataset".into(),
        resource_scope: "payments".into(),
        requested_expires_at: Some(timestamp(now + REQUEST_VALIDITY_SECONDS)),
        issuance_nonce: vec![0x73; 32],
        snapshot_id: snapshot_id.into(),
        request_signature: Vec::new(),
        request_context: vec![AuthorizationContextEntry {
            name: "acceptance_run_label".into(),
            value: Some(authorization_context_entry::Value::StringValue(
                PRIVATE_CONTEXT_VALUE.into(),
            )),
        }],
    };
    let commitment = capability_request_commitment(&capability_binding(&request)?)?;
    request.request_signature = signing_key.sign(&commitment).to_bytes().to_vec();
    Ok(request)
}

fn capability_binding(request: &CreateCapabilityRequest) -> TestResult<CapabilityRequestBindingV1> {
    let public_key: [u8; 32] = request
        .agent_public_key
        .as_slice()
        .try_into()
        .map_err(|_| io::Error::other("agent public key has the wrong length"))?;
    let expires_at = request
        .requested_expires_at
        .as_ref()
        .ok_or_else(|| io::Error::other("requested expiry is missing"))?
        .seconds;
    let issuance_nonce = request
        .issuance_nonce
        .as_slice()
        .try_into()
        .map_err(|_| io::Error::other("issuance nonce has the wrong length"))?;
    let request_context = request
        .request_context
        .iter()
        .map(|entry| {
            let value = match entry.value.as_ref() {
                Some(authorization_context_entry::Value::BoolValue(value)) => {
                    CaveatValue::Bool(*value)
                }
                Some(authorization_context_entry::Value::IntValue(value)) => {
                    CaveatValue::Int(*value)
                }
                Some(authorization_context_entry::Value::StringValue(value)) => {
                    CaveatValue::String(value.clone())
                }
                None => return Err(io::Error::other("request context value is missing")),
            };
            Ok(ContextEntry {
                name: entry.name.clone(),
                value,
            })
        })
        .collect::<Result<Vec<_>, io::Error>>()?;
    Ok(CapabilityRequestBindingV1 {
        format_version: request.format_version,
        agent_key_thumbprint: derive_ed25519_key_thumbprint(&public_key),
        task_id: request.task_id.clone(),
        purpose: request.purpose.clone(),
        action: request.action.clone(),
        resource_type: request.resource_type.clone(),
        resource_scope: request.resource_scope.clone(),
        requested_expires_at_unix_seconds: expires_at,
        issuance_nonce,
        snapshot_id: request.snapshot_id.clone(),
        request_context,
    })
}

fn protected_request(
    capability: ProofCapability,
    signing_key: SigningKey,
    now: i64,
) -> TestResult<VerifyCapabilityRequest> {
    let method = "POST";
    let target = "/payments/reconcile?account=m4&status=pending";
    let body_hash = [0x81; 32];
    let request_nonce = [0x82; 32];
    let message = protected_request_signature_message(&ProtectedRequestSignatureInputV1 {
        format_version: PROTECTED_REQUEST_FORMAT_VERSION,
        capability_id: &capability.capability_id,
        request_method: method,
        canonical_request_target: target,
        request_body_hash: &body_hash,
        request_nonce: &request_nonce,
        request_timestamp_unix_seconds: now,
    })?;
    Ok(VerifyCapabilityRequest {
        proof_capability: Some(capability),
        agent_public_key: signing_key.verifying_key().to_bytes().to_vec(),
        request_method: method.into(),
        canonical_request_target: target.into(),
        request_body_hash: body_hash.to_vec(),
        request_nonce: request_nonce.to_vec(),
        request_timestamp: Some(timestamp(now)),
        agent_request_signature: signing_key.sign(&message).to_bytes().to_vec(),
    })
}

fn assert_rejected(
    response: tonic::Response<raziel_proto::raziel::v1::VerifyCapabilityResponse>,
    label: &str,
) -> TestResult {
    let response = response.into_inner();
    if response.outcome != VerificationOutcome::Rejected as i32
        || response.verified_claims.is_some()
        || response.failure_code == PublicErrorCode::Unspecified as i32
        || !response.policy_record_id.is_empty()
        || !response.snapshot_id.is_empty()
        || !response.proof_digest.is_empty()
    {
        return Err(io::Error::other(format!("{label} was not rejected cleanly")).into());
    }
    Ok(())
}

async fn connect_channel(endpoint: &str) -> TestResult<Channel> {
    let endpoint = Endpoint::from_shared(endpoint.to_owned())?;
    let deadline = Instant::now() + SERVICE_CONNECT_TIMEOUT;
    loop {
        match endpoint.clone().connect().await {
            Ok(channel) => return Ok(channel),
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn authorization(token: &str) -> TestResult<MetadataValue<Ascii>> {
    let mut value: MetadataValue<Ascii> = format!("Bearer {token}").try_into()?;
    value.set_sensitive(true);
    Ok(value)
}

fn authenticated<T>(value: T, authorization: &MetadataValue<Ascii>) -> Request<T> {
    let mut request = Request::new(value);
    request
        .metadata_mut()
        .insert("authorization", authorization.clone());
    request
}

fn read_capability(path: &Path) -> TestResult<ProofCapability> {
    let bytes = read_bounded_regular(path, MAX_PUBLIC_CAPABILITY_BYTES)?;
    let capability = ProofCapability::decode(bytes.as_slice())?;
    if capability.encode_to_vec() != bytes {
        return Err(io::Error::other("public capability is not canonical Protobuf").into());
    }
    Ok(capability)
}

fn read_job_id(path: &Path) -> TestResult<String> {
    let bytes = read_bounded_regular(path, 64)?;
    let value = String::from_utf8(bytes)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(io::Error::other("stored job ID is invalid").into());
    }
    Ok(value)
}

fn read_bounded_regular(path: &Path, max_bytes: u64) -> TestResult<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(io::Error::other("acceptance artifact has an invalid shape").into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(io::Error::other("acceptance artifact changed during its read").into());
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> TestResult {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("acceptance output has no parent"))?;
    let metadata = std::fs::symlink_metadata(parent)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other("acceptance output directory is unsafe").into());
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn decode_digest(value: &str) -> TestResult<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(io::Error::other("digest must be 64 lowercase hexadecimal characters").into());
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    Ok(output)
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn duration_millis(value: Option<&prost_types::Duration>) -> i128 {
    value.map_or(-1, |duration| {
        i128::from(duration.seconds) * 1_000 + i128::from(duration.nanos) / 1_000_000
    })
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other(format!("{name} is required")).into())
}

fn required_path(name: &str) -> TestResult<PathBuf> {
    let value = required_env(name)?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(io::Error::other(format!("{name} must be absolute")).into());
    }
    Ok(path)
}

fn agent_signing_key() -> SigningKey {
    SigningKey::from_bytes(&AGENT_SIGNING_KEY)
}

fn unix_now() -> TestResult<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(i64::try_from(seconds)?)
}

const fn timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}
