//! Bounded JSON bridge to the independent Tonic verifier.

use std::fmt;
use std::io::Read;

use prost::Message;
use prost_types::Timestamp;
use raziel_proto::raziel::v1::verification_service_client::VerificationServiceClient;
use raziel_proto::raziel::v1::{
    ProofCapability, PublicErrorCode, VerificationOutcome, VerifiedClaims, VerifyCapabilityRequest,
    VerifyCapabilityResponse,
};
use serde::{Deserialize, Serialize};

const MAX_INPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CAPABILITY_PROTO_BYTES: usize = 2 * 1024 * 1024;
const DIGEST_BYTES: usize = 32;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_METHOD_BYTES: usize = 16;
const MAX_TARGET_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierClientError {
    Configuration,
    InvalidRequest,
    Unavailable,
    Protocol,
}

impl fmt::Display for VerifierClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "configuration is invalid",
            Self::InvalidRequest => "request is invalid",
            Self::Unavailable => "verification service is unavailable",
            Self::Protocol => "verification service returned an invalid response",
        })
    }
}

impl std::error::Error for VerifierClientError {}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum VerifierCommand {
    Inspect(InspectInput),
    Verify(VerifyInput),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectInput {
    capability_proto_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyInput {
    capability_proto_hex: String,
    agent_public_key: String,
    request_method: String,
    canonical_request_target: String,
    request_body_hash: String,
    request_nonce: String,
    request_timestamp_unix_seconds: String,
    agent_request_signature: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct InspectOutput {
    capability_id: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct VerifierOutput {
    outcome: &'static str,
    failure_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    verified_claims: Option<VerifiedClaimsProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_record_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_digest: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct VerifiedClaimsProjection {
    format_version: u32,
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
    issued_at_unix_seconds: String,
    expires_at_unix_seconds: String,
}

pub async fn run_from_env() -> Result<(), VerifierClientError> {
    match parse_command(&read_input()?)? {
        VerifierCommand::Inspect(input) => write_output(&inspect_capability(input)?),
        VerifierCommand::Verify(input) => {
            let endpoint = verifier_endpoint()?;
            let request = verify_request(input)?;
            let mut client = VerificationServiceClient::connect(endpoint)
                .await
                .map_err(|_| VerifierClientError::Unavailable)?;
            let response = client
                .verify_capability(request)
                .await
                .map_err(|_| VerifierClientError::Unavailable)?
                .into_inner();
            write_output(&project_response(response)?)
        }
    }
}

fn read_input() -> Result<Vec<u8>, VerifierClientError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| VerifierClientError::InvalidRequest)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(VerifierClientError::InvalidRequest);
    }
    Ok(bytes)
}

fn parse_command(bytes: &[u8]) -> Result<VerifierCommand, VerifierClientError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(VerifierClientError::InvalidRequest);
    }
    serde_json::from_slice(bytes).map_err(|_| VerifierClientError::InvalidRequest)
}

fn inspect_capability(input: InspectInput) -> Result<InspectOutput, VerifierClientError> {
    let capability = decode_capability(&input.capability_proto_hex)?;
    Ok(InspectOutput {
        capability_id: exact_capability_id(capability.capability_id)?,
    })
}

fn verify_request(input: VerifyInput) -> Result<VerifyCapabilityRequest, VerifierClientError> {
    let proof_capability = decode_capability(&input.capability_proto_hex)?;
    let request_timestamp_unix_seconds = parse_decimal_i64(&input.request_timestamp_unix_seconds)?;

    Ok(VerifyCapabilityRequest {
        proof_capability: Some(proof_capability),
        agent_public_key: decode_exact_hex(&input.agent_public_key, ED25519_PUBLIC_KEY_BYTES)?,
        request_method: bounded_text(input.request_method, MAX_METHOD_BYTES)?,
        canonical_request_target: bounded_text(input.canonical_request_target, MAX_TARGET_BYTES)?,
        request_body_hash: decode_exact_hex(&input.request_body_hash, DIGEST_BYTES)?,
        request_nonce: decode_exact_hex(&input.request_nonce, DIGEST_BYTES)?,
        request_timestamp: Some(Timestamp {
            seconds: request_timestamp_unix_seconds,
            nanos: 0,
        }),
        agent_request_signature: decode_exact_hex(
            &input.agent_request_signature,
            ED25519_SIGNATURE_BYTES,
        )?,
    })
}

fn decode_capability(value: &str) -> Result<ProofCapability, VerifierClientError> {
    let capability_bytes = decode_bounded_hex(value, MAX_CAPABILITY_PROTO_BYTES)?;
    let capability = ProofCapability::decode(capability_bytes.as_slice())
        .map_err(|_| VerifierClientError::InvalidRequest)?;
    if capability.encode_to_vec() != capability_bytes {
        return Err(VerifierClientError::InvalidRequest);
    }
    Ok(capability)
}

fn verifier_endpoint() -> Result<String, VerifierClientError> {
    let endpoint = std::env::var("RAZIEL_VERIFIERD_ENDPOINT")
        .map_err(|_| VerifierClientError::Configuration)?;
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err(VerifierClientError::Configuration);
    }
    Ok(endpoint)
}

fn write_output(output: &impl Serialize) -> Result<(), VerifierClientError> {
    let encoded = serde_json::to_vec(output).map_err(|_| VerifierClientError::Protocol)?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), &encoded)
        .map_err(|_| VerifierClientError::Unavailable)
}

fn project_response(
    response: VerifyCapabilityResponse,
) -> Result<VerifierOutput, VerifierClientError> {
    let outcome = VerificationOutcome::try_from(response.outcome)
        .map_err(|_| VerifierClientError::Protocol)?;
    let failure_code = PublicErrorCode::try_from(response.failure_code)
        .map_err(|_| VerifierClientError::Protocol)?;

    match outcome {
        VerificationOutcome::Verified => {
            if failure_code != PublicErrorCode::Unspecified {
                return Err(VerifierClientError::Protocol);
            }
            let claims = project_verified_claims(
                response
                    .verified_claims
                    .ok_or(VerifierClientError::Protocol)?,
            )?;
            let policy_record_id = bounded_response_text(response.policy_record_id)?;
            let snapshot_id = bounded_response_text(response.snapshot_id)?;
            if snapshot_id != claims.snapshot_id || response.proof_digest.len() != DIGEST_BYTES {
                return Err(VerifierClientError::Protocol);
            }
            Ok(VerifierOutput {
                outcome: "verified",
                failure_code: failure_code.as_str_name().to_owned(),
                verified_claims: Some(claims),
                policy_record_id: Some(policy_record_id),
                snapshot_id: Some(snapshot_id),
                proof_digest: Some(lowercase_hex(&response.proof_digest)),
            })
        }
        VerificationOutcome::Rejected => {
            if failure_code == PublicErrorCode::Unspecified
                || response.verified_claims.is_some()
                || !response.policy_record_id.is_empty()
                || !response.snapshot_id.is_empty()
                || !response.proof_digest.is_empty()
            {
                return Err(VerifierClientError::Protocol);
            }
            Ok(VerifierOutput {
                outcome: "rejected",
                failure_code: failure_code.as_str_name().to_owned(),
                verified_claims: None,
                policy_record_id: None,
                snapshot_id: None,
                proof_digest: None,
            })
        }
        VerificationOutcome::Unspecified => Err(VerifierClientError::Protocol),
    }
}

fn project_verified_claims(
    claims: VerifiedClaims,
) -> Result<VerifiedClaimsProjection, VerifierClientError> {
    if claims.format_version != 1
        || claims.agent_key_thumbprint.len() != DIGEST_BYTES
        || claims.policy_hash.len() != DIGEST_BYTES
        || claims.relationship_snapshot_root.len() != DIGEST_BYTES
    {
        return Err(VerifierClientError::Protocol);
    }
    let issued_at_unix_seconds = whole_second(claims.issued_at)?;
    let expires_at_unix_seconds = whole_second(claims.expires_at)?;
    if expires_at_unix_seconds <= issued_at_unix_seconds {
        return Err(VerifierClientError::Protocol);
    }

    Ok(VerifiedClaimsProjection {
        format_version: claims.format_version,
        capability_id: exact_response_capability_id(claims.capability_id)?,
        agent_key_thumbprint: lowercase_hex(&claims.agent_key_thumbprint),
        task_id: bounded_response_text(claims.task_id)?,
        purpose: bounded_response_text(claims.purpose)?,
        action: bounded_response_text(claims.action)?,
        resource_type: bounded_response_text(claims.resource_type)?,
        resource_scope: bounded_response_text(claims.resource_scope)?,
        policy_hash: lowercase_hex(&claims.policy_hash),
        relationship_snapshot_root: lowercase_hex(&claims.relationship_snapshot_root),
        snapshot_id: bounded_response_text(claims.snapshot_id)?,
        issued_at_unix_seconds: issued_at_unix_seconds.to_string(),
        expires_at_unix_seconds: expires_at_unix_seconds.to_string(),
    })
}

fn whole_second(timestamp: Option<Timestamp>) -> Result<i64, VerifierClientError> {
    let timestamp = timestamp.ok_or(VerifierClientError::Protocol)?;
    if timestamp.nanos != 0 {
        return Err(VerifierClientError::Protocol);
    }
    Ok(timestamp.seconds)
}

fn bounded_text(value: String, max_bytes: usize) -> Result<String, VerifierClientError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() != value.len()
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        return Err(VerifierClientError::InvalidRequest);
    }
    Ok(value)
}

fn bounded_response_text(value: String) -> Result<String, VerifierClientError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() != value.len()
        || value.len() > MAX_IDENTIFIER_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(VerifierClientError::Protocol);
    }
    Ok(value)
}

fn exact_capability_id(value: String) -> Result<String, VerifierClientError> {
    if value.len() != 64 || !value.bytes().all(is_lowercase_hex_digit) {
        return Err(VerifierClientError::InvalidRequest);
    }
    Ok(value)
}

fn exact_response_capability_id(value: String) -> Result<String, VerifierClientError> {
    if value.len() != 64 || !value.bytes().all(is_lowercase_hex_digit) {
        return Err(VerifierClientError::Protocol);
    }
    Ok(value)
}

fn parse_decimal_i64(value: &str) -> Result<i64, VerifierClientError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(VerifierClientError::InvalidRequest);
    }
    value
        .parse::<i64>()
        .map_err(|_| VerifierClientError::InvalidRequest)
}

fn decode_exact_hex(value: &str, expected_bytes: usize) -> Result<Vec<u8>, VerifierClientError> {
    if value.len() != expected_bytes * 2 {
        return Err(VerifierClientError::InvalidRequest);
    }
    decode_bounded_hex(value, expected_bytes)
}

fn decode_bounded_hex(value: &str, max_bytes: usize) -> Result<Vec<u8>, VerifierClientError> {
    if value.is_empty()
        || value.len() > max_bytes.saturating_mul(2)
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(is_lowercase_hex_digit)
    {
        return Err(VerifierClientError::InvalidRequest);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text =
                std::str::from_utf8(pair).map_err(|_| VerifierClientError::InvalidRequest)?;
            u8::from_str_radix(text, 16).map_err(|_| VerifierClientError::InvalidRequest)
        })
        .collect()
}

fn is_lowercase_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
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

    fn minimal_capability() -> ProofCapability {
        ProofCapability {
            capability_id: "ab".repeat(32),
            ..ProofCapability::default()
        }
    }

    fn verify_input_json(capability: &ProofCapability) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "operation": "verify",
            "capability_proto_hex": lowercase_hex(&capability.encode_to_vec()),
            "agent_public_key": "11".repeat(ED25519_PUBLIC_KEY_BYTES),
            "request_method": "POST",
            "canonical_request_target": "/api/datasets/payments/protected-query",
            "request_body_hash": "22".repeat(DIGEST_BYTES),
            "request_nonce": "33".repeat(DIGEST_BYTES),
            "request_timestamp_unix_seconds": "1800000000",
            "agent_request_signature": "44".repeat(ED25519_SIGNATURE_BYTES),
        }))
        .unwrap()
    }

    fn verify_request_from_json(
        bytes: &[u8],
    ) -> Result<VerifyCapabilityRequest, VerifierClientError> {
        match parse_command(bytes)? {
            VerifierCommand::Verify(input) => verify_request(input),
            VerifierCommand::Inspect(_) => Err(VerifierClientError::InvalidRequest),
        }
    }

    fn verified_response() -> VerifyCapabilityResponse {
        VerifyCapabilityResponse {
            outcome: VerificationOutcome::Verified as i32,
            verified_claims: Some(VerifiedClaims {
                format_version: 1,
                capability_id: "ab".repeat(32),
                agent_key_thumbprint: vec![0x11; DIGEST_BYTES],
                task_id: "task-1".into(),
                purpose: "reconciliation".into(),
                action: "query".into(),
                resource_type: "dataset".into(),
                resource_scope: "payments".into(),
                policy_hash: vec![0x22; DIGEST_BYTES],
                relationship_snapshot_root: vec![0x33; DIGEST_BYTES],
                snapshot_id: "snapshot-1".into(),
                capability_request_commitment: vec![0x44; DIGEST_BYTES],
                issuance_nullifier: vec![0x55; DIGEST_BYTES],
                issued_at: Some(Timestamp {
                    seconds: 1_800_000_000,
                    nanos: 0,
                }),
                expires_at: Some(Timestamp {
                    seconds: 1_800_003_600,
                    nanos: 0,
                }),
            }),
            failure_code: PublicErrorCode::Unspecified as i32,
            policy_record_id: "policy-record-1".into(),
            snapshot_id: "snapshot-1".into(),
            proof_digest: vec![0x66; DIGEST_BYTES],
        }
    }

    #[test]
    fn parses_bounded_request_and_decodes_capability() {
        let capability = minimal_capability();
        let request = verify_request_from_json(&verify_input_json(&capability)).unwrap();

        assert_eq!(request.proof_capability.unwrap(), capability);
        assert_eq!(request.agent_public_key, vec![0x11; 32]);
        assert_eq!(request.request_timestamp.unwrap().nanos, 0);
    }

    #[test]
    fn rejects_unknown_json_fields_and_wrong_crypto_lengths() {
        let capability = minimal_capability();
        let mut value: serde_json::Value =
            serde_json::from_slice(&verify_input_json(&capability)).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );

        value.as_object_mut().unwrap().remove("unknown");
        value["request_nonce"] = serde_json::json!("00");
        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );
    }

    #[test]
    fn rejects_non_string_timestamps_and_uppercase_hex() {
        let capability = minimal_capability();
        let mut value: serde_json::Value =
            serde_json::from_slice(&verify_input_json(&capability)).unwrap();
        value["request_timestamp_unix_seconds"] = serde_json::json!(1_800_000_000_i64);
        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );

        value["request_body_hash"] = serde_json::json!("22".repeat(DIGEST_BYTES));
        value["request_timestamp_unix_seconds"] = serde_json::json!("01800000000");
        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );

        value["request_timestamp_unix_seconds"] = serde_json::json!("1800000000");
        value["request_body_hash"] = serde_json::json!("AA".repeat(DIGEST_BYTES));
        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );
    }

    #[test]
    fn rejects_whitespace_normalization_of_signed_request_fields() {
        let capability = minimal_capability();
        let mut value: serde_json::Value =
            serde_json::from_slice(&verify_input_json(&capability)).unwrap();
        value["canonical_request_target"] =
            serde_json::json!(" /api/datasets/payments/protected-query");

        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );
    }

    #[test]
    fn rejects_capability_bytes_that_do_not_round_trip_exactly() {
        let capability = minimal_capability();
        let mut noncanonical = capability.encode_to_vec();
        noncanonical.extend_from_slice(&[0xa0, 0x06, 0x01]);
        let mut value: serde_json::Value =
            serde_json::from_slice(&verify_input_json(&capability)).unwrap();
        value["capability_proto_hex"] = serde_json::json!(lowercase_hex(&noncanonical));

        assert_eq!(
            verify_request_from_json(&serde_json::to_vec(&value).unwrap()),
            Err(VerifierClientError::InvalidRequest)
        );
    }

    #[test]
    fn inspects_only_an_exact_canonical_capability_id() {
        let capability = minimal_capability();
        let bytes = serde_json::to_vec(&serde_json::json!({
            "operation": "inspect",
            "capability_proto_hex": lowercase_hex(&capability.encode_to_vec()),
        }))
        .unwrap();
        let output = match parse_command(&bytes).unwrap() {
            VerifierCommand::Inspect(input) => inspect_capability(input).unwrap(),
            VerifierCommand::Verify(_) => panic!("expected inspect command"),
        };

        assert_eq!(output.capability_id, "ab".repeat(32));
    }

    #[test]
    fn inspect_rejects_extra_fields_and_noncanonical_ids() {
        let capability = minimal_capability();
        let extra = serde_json::to_vec(&serde_json::json!({
            "operation": "inspect",
            "capability_proto_hex": lowercase_hex(&capability.encode_to_vec()),
            "request_nonce": "00".repeat(DIGEST_BYTES),
        }))
        .unwrap();
        assert!(matches!(
            parse_command(&extra),
            Err(VerifierClientError::InvalidRequest)
        ));

        let invalid = InspectInput {
            capability_proto_hex: lowercase_hex(
                &ProofCapability {
                    capability_id: "AB".repeat(32),
                    ..ProofCapability::default()
                }
                .encode_to_vec(),
            ),
        };
        assert_eq!(
            inspect_capability(invalid),
            Err(VerifierClientError::InvalidRequest)
        );
    }

    #[test]
    fn projects_only_approved_public_verified_fields() {
        let output = project_response(verified_response()).unwrap();
        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["outcome"], "verified");
        assert_eq!(json["verified_claims"]["action"], "query");
        assert_eq!(
            json["verified_claims"]["issued_at_unix_seconds"],
            "1800000000"
        );
        assert_eq!(
            json["verified_claims"]["expires_at_unix_seconds"],
            "1800003600"
        );
        assert_eq!(json["proof_digest"], "66".repeat(DIGEST_BYTES));
        assert!(
            json["verified_claims"]
                .get("capability_request_commitment")
                .is_none()
        );
        assert!(json["verified_claims"].get("issuance_nullifier").is_none());
        assert!(json.get("zk_proof").is_none());
    }

    #[test]
    fn clean_rejection_is_a_valid_projection() {
        let output = project_response(VerifyCapabilityResponse {
            outcome: VerificationOutcome::Rejected as i32,
            verified_claims: None,
            failure_code: PublicErrorCode::NotAuthorized as i32,
            policy_record_id: String::new(),
            snapshot_id: String::new(),
            proof_digest: Vec::new(),
        })
        .unwrap();

        assert_eq!(output.outcome, "rejected");
        assert_eq!(output.failure_code, "PUBLIC_ERROR_CODE_NOT_AUTHORIZED");
        assert!(output.verified_claims.is_none());
        assert!(output.proof_digest.is_none());
    }

    #[test]
    fn rejects_inconsistent_verified_response() {
        let mut response = verified_response();
        response.snapshot_id = "different-snapshot".into();
        assert_eq!(
            project_response(response),
            Err(VerifierClientError::Protocol)
        );
    }
}
