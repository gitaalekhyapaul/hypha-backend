//! Bounded, local JSON helper for ADR-0020 authentication.
//! The caller owns the two-second process deadline and all token/replay state.

use std::fmt;
use std::io::{Read, Write};

use raziel_authority::{validate_agent_api_key, verify_agent_api_request};
use raziel_canonical::{
    AGENT_API_REQUEST_FORMAT_VERSION, AgentApiRequestSignatureInputV1,
    MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS,
};
use serde::{Deserialize, Serialize};

pub const MAX_AGENT_AUTH_INPUT_BYTES: usize = 8_192;
pub const MAX_AGENT_AUTH_OUTPUT_BYTES: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentAuthError {
    InvalidRequest,
    Internal,
}

impl AgentAuthError {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidRequest => 2,
            Self::Internal => 1,
        }
    }
}

impl fmt::Display for AgentAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "request is invalid",
            Self::Internal => "authentication helper failed",
        })
    }
}

impl std::error::Error for AgentAuthError {}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum AgentAuthCommand {
    ValidateKey {
        agent_public_key: String,
    },
    Verify {
        audience: String,
        token_id: String,
        agent_public_key: String,
        request_method: String,
        canonical_request_target: String,
        request_body_hash: String,
        request_nonce: String,
        request_timestamp_unix_seconds: String,
        request_signature: String,
    },
}

#[derive(Serialize)]
struct AgentAuthOutput {
    agent_key_thumbprint: String,
}

pub fn run_from_env() -> Result<(), AgentAuthError> {
    if std::env::args_os().len() != 1 {
        return Err(AgentAuthError::InvalidRequest);
    }
    run(std::io::stdin().lock(), std::io::stdout().lock())
}

/// Reads at most 8 KiB plus one overflow byte, then writes one bounded success JSON.
pub fn run(input: impl Read, mut output: impl Write) -> Result<(), AgentAuthError> {
    let mut bytes = Vec::new();
    input
        .take((MAX_AGENT_AUTH_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AgentAuthError::Internal)?;
    if bytes.is_empty() || bytes.len() > MAX_AGENT_AUTH_INPUT_BYTES {
        return Err(AgentAuthError::InvalidRequest);
    }
    let command = serde_json::from_slice(&bytes).map_err(|_| AgentAuthError::InvalidRequest)?;
    let thumbprint = execute(command)?;
    let encoded = serde_json::to_vec(&AgentAuthOutput {
        agent_key_thumbprint: lowercase_hex(&thumbprint),
    })
    .map_err(|_| AgentAuthError::Internal)?;
    if encoded.len() > MAX_AGENT_AUTH_OUTPUT_BYTES {
        return Err(AgentAuthError::Internal);
    }
    output
        .write_all(&encoded)
        .and_then(|()| output.flush())
        .map_err(|_| AgentAuthError::Internal)
}

fn execute(command: AgentAuthCommand) -> Result<[u8; 32], AgentAuthError> {
    match command {
        AgentAuthCommand::ValidateKey { agent_public_key } => {
            validate_agent_api_key(&decode_hex::<32>(&agent_public_key)?)
                .map_err(|_| AgentAuthError::InvalidRequest)
        }
        AgentAuthCommand::Verify {
            audience,
            token_id,
            agent_public_key,
            request_method,
            canonical_request_target,
            request_body_hash,
            request_nonce,
            request_timestamp_unix_seconds,
            request_signature,
        } => {
            let public_key = decode_hex::<32>(&agent_public_key)?;
            let thumbprint =
                validate_agent_api_key(&public_key).map_err(|_| AgentAuthError::InvalidRequest)?;
            let body_hash = decode_hex::<32>(&request_body_hash)?;
            let nonce = decode_hex::<32>(&request_nonce)?;
            let signature = decode_hex::<64>(&request_signature)?;
            let input = AgentApiRequestSignatureInputV1 {
                format_version: AGENT_API_REQUEST_FORMAT_VERSION,
                audience: &audience,
                token_id: &token_id,
                agent_key_thumbprint: &thumbprint,
                request_method: &request_method,
                canonical_request_target: &canonical_request_target,
                request_body_hash: &body_hash,
                request_nonce: &nonce,
                request_timestamp_unix_seconds: parse_timestamp(&request_timestamp_unix_seconds)?,
            };
            verify_agent_api_request(&public_key, &input, &signature)
                .map_err(|_| AgentAuthError::InvalidRequest)
        }
    }
}

fn parse_timestamp(value: &str) -> Result<i64, AgentAuthError> {
    if value.is_empty()
        || value.len() > 12
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AgentAuthError::InvalidRequest);
    }
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value <= MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS)
        .ok_or(AgentAuthError::InvalidRequest)
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], AgentAuthError> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(AgentAuthError::InvalidRequest);
    }
    let mut decoded = [0; N];
    for (index, byte) in decoded.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| AgentAuthError::InvalidRequest)?;
    }
    Ok(decoded)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_decimal_is_canonical_and_bounded() {
        for (text, expected) in [
            ("0", 0),
            ("253402300799", MAX_AGENT_API_TIMESTAMP_UNIX_SECONDS),
        ] {
            assert_eq!(parse_timestamp(text), Ok(expected));
        }
        for text in [
            "",
            "00",
            "01",
            "-0",
            "-1",
            "+1",
            " 1",
            "1 ",
            "1.0",
            "1e2",
            "253402300800",
            "9223372036854775808",
            "\u{ff11}",
        ] {
            assert_eq!(parse_timestamp(text), Err(AgentAuthError::InvalidRequest));
        }
    }
}
