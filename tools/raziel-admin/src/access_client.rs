//! Bounded owner-only RPC transport. This helper has no policy or signing authority.

use std::io::{Read, Write};
use std::time::Duration;

use prost::Message;
use raziel_proto::raziel::v1::access_request_service_client::AccessRequestServiceClient;
use raziel_proto::raziel::v1::{
    ApproveAccessRequestRequest, CancelAccessRequestRequest, CreateAccessRequestRequest,
    GetAccessRequestRequest, ListAccessRequestsRequest, RejectAccessRequestRequest,
    ResumeAccessRequestRequest, SubmitAccessIssuanceRequest,
};
use serde::Deserialize;
use tonic::{Code, Status, transport::Endpoint};

const MAX_COMMAND: u64 = 128 * 1024;
const MAX_REQUEST: usize = 32 * 1024;
const MAX_RESPONSE: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessClientError {
    Unavailable,
    InvalidRequest,
    NotFound,
    Conflict,
    Expired,
    Capacity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    operation: Operation,
    request_proto_hex: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Create,
    Get,
    List,
    Approve,
    Reject,
    Cancel,
    Resume,
    SubmitIssuance,
}

pub async fn run_from_env() -> Result<(), AccessClientError> {
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_COMMAND + 1)
        .read_to_end(&mut input)
        .map_err(|_| AccessClientError::InvalidRequest)?;
    if input.is_empty() || input.len() as u64 > MAX_COMMAND {
        return Err(AccessClientError::InvalidRequest);
    }
    let command: Command =
        serde_json::from_slice(&input).map_err(|_| AccessClientError::InvalidRequest)?;
    let bytes = decode_hex(&command.request_proto_hex)?;
    let address =
        std::env::var("RAZIEL_ACCESS_ENDPOINT").map_err(|_| AccessClientError::Unavailable)?;
    if !(address.starts_with("http://") || address.starts_with("https://")) {
        return Err(AccessClientError::Unavailable);
    }
    let endpoint = Endpoint::from_shared(address)
        .map_err(|_| AccessClientError::Unavailable)?
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(7));
    let channel = endpoint
        .connect()
        .await
        .map_err(|_| AccessClientError::Unavailable)?;
    let mut client = AccessRequestServiceClient::new(channel)
        .max_decoding_message_size(MAX_RESPONSE)
        .max_encoding_message_size(MAX_REQUEST);
    macro_rules! invoke {
        ($method:ident, $message:ty) => {{
            let request = <$message>::decode(bytes.as_slice())
                .map_err(|_| AccessClientError::InvalidRequest)?;
            client
                .$method(request)
                .await
                .map_err(map_status)?
                .into_inner()
                .encode_to_vec()
        }};
    }
    let output = match command.operation {
        Operation::Create => invoke!(create_access_request, CreateAccessRequestRequest),
        Operation::Get => invoke!(get_access_request, GetAccessRequestRequest),
        Operation::List => invoke!(list_access_requests, ListAccessRequestsRequest),
        Operation::Approve => invoke!(approve_access_request, ApproveAccessRequestRequest),
        Operation::Reject => invoke!(reject_access_request, RejectAccessRequestRequest),
        Operation::Cancel => invoke!(cancel_access_request, CancelAccessRequestRequest),
        Operation::Resume => invoke!(resume_access_request, ResumeAccessRequestRequest),
        Operation::SubmitIssuance => invoke!(submit_access_issuance, SubmitAccessIssuanceRequest),
    };
    if output.len() > MAX_RESPONSE {
        return Err(AccessClientError::Unavailable);
    }
    let mut encoded = String::with_capacity(output.len() * 2);
    for byte in output {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").map_err(|_| AccessClientError::Unavailable)?;
    }
    std::io::stdout()
        .lock()
        .write_all(encoded.as_bytes())
        .map_err(|_| AccessClientError::Unavailable)
}

pub const fn exit_code(error: AccessClientError) -> i32 {
    match error {
        AccessClientError::Unavailable => 2,
        AccessClientError::InvalidRequest => 3,
        AccessClientError::NotFound => 4,
        AccessClientError::Conflict => 5,
        AccessClientError::Expired => 6,
        AccessClientError::Capacity => 7,
    }
}

fn map_status(status: Status) -> AccessClientError {
    match status.code() {
        Code::InvalidArgument => AccessClientError::InvalidRequest,
        Code::NotFound => AccessClientError::NotFound,
        Code::AlreadyExists | Code::Aborted => AccessClientError::Conflict,
        Code::OutOfRange => AccessClientError::Expired,
        Code::ResourceExhausted => AccessClientError::Capacity,
        _ => AccessClientError::Unavailable,
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, AccessClientError> {
    if value.len() > MAX_REQUEST * 2
        || !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AccessClientError::InvalidRequest);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| AccessClientError::InvalidRequest)?;
            u8::from_str_radix(text, 16).map_err(|_| AccessClientError::InvalidRequest)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_errors_have_stable_coarse_exit_codes() {
        for (code, expected) in [
            (Code::InvalidArgument, 3),
            (Code::NotFound, 4),
            (Code::Aborted, 5),
            (Code::OutOfRange, 6),
            (Code::ResourceExhausted, 7),
            (Code::Internal, 2),
        ] {
            assert_eq!(
                exit_code(map_status(Status::new(code, "private detail"))),
                expected
            );
        }
    }
    #[test]
    fn only_bounded_canonical_hex_is_accepted() {
        assert_eq!(decode_hex("00ff"), Ok(vec![0, 255]));
        for value in ["0", "FF", "gg", &"aa".repeat(MAX_REQUEST + 1)] {
            assert_eq!(decode_hex(value), Err(AccessClientError::InvalidRequest));
        }
    }
}
