use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use prost::Message;

use super::wire::{
    PolicyActivationFailureWire, PolicyActivationRequestWire, PolicyActivationResponseWire,
};
use super::{
    MAX_FRAME_BYTES, PROTOCOL_VERSION, PolicyActivationClientError, PolicyActivationProjection,
    PublicTimestamp, SOCKET_TIMEOUT,
};

pub(super) fn send_request(
    socket_path: &Path,
    request: PolicyActivationRequestWire,
) -> Result<PolicyActivationProjection, PolicyActivationClientError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| PolicyActivationClientError::Unavailable)?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| PolicyActivationClientError::Unavailable)?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| PolicyActivationClientError::Unavailable)?;
    write_message(&mut stream, &request).map_err(|_| PolicyActivationClientError::Protocol)?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|_| PolicyActivationClientError::Protocol)?;
    let response: PolicyActivationResponseWire =
        read_message(&mut stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    require_eof(&mut stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    response_result(response)
}

pub(super) fn read_request(
    stream: &mut UnixStream,
) -> Result<PolicyActivationRequestWire, PolicyActivationClientError> {
    let request = read_message(stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    require_eof(stream).map_err(|_| PolicyActivationClientError::Protocol)?;
    Ok(request)
}

pub(super) fn write_response(
    stream: &mut UnixStream,
    response: &PolicyActivationResponseWire,
) -> Result<(), PolicyActivationClientError> {
    write_message(stream, response).map_err(|_| PolicyActivationClientError::Protocol)
}

fn write_message<M: Message>(stream: &mut UnixStream, message: &M) -> Result<(), ()> {
    let bytes = message.encode_to_vec();
    if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES {
        return Err(());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| ())?;
    stream.write_all(&length.to_be_bytes()).map_err(|_| ())?;
    stream.write_all(&bytes).map_err(|_| ())?;
    stream.flush().map_err(|_| ())
}

fn read_message<M: Message + Default>(stream: &mut UnixStream) -> Result<M, ()> {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).map_err(|_| ())?;
    let length = usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| ())?;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(());
    }
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes).map_err(|_| ())?;
    let value = M::decode(bytes.as_slice()).map_err(|_| ())?;
    if value.encode_to_vec() != bytes {
        return Err(());
    }
    Ok(value)
}

fn require_eof(stream: &mut UnixStream) -> Result<(), ()> {
    let mut trailing = [0_u8; 1];
    match stream.read(&mut trailing) {
        Ok(0) => Ok(()),
        _ => Err(()),
    }
}

fn response_result(
    response: PolicyActivationResponseWire,
) -> Result<PolicyActivationProjection, PolicyActivationClientError> {
    if response.format_version != PROTOCOL_VERSION {
        return Err(PolicyActivationClientError::Protocol);
    }
    let failure = PolicyActivationFailureWire::try_from(response.failure)
        .map_err(|_| PolicyActivationClientError::Protocol)?;
    match failure {
        PolicyActivationFailureWire::Unspecified => {
            let value = response
                .projection
                .ok_or(PolicyActivationClientError::Protocol)?;
            if value.record_id.is_empty()
                || value.policy_name.is_empty()
                || value.policy_version.is_empty()
                || value.policy_hash.len() != 64
                || value.approval_epoch == 0
                || value.registered_at_unix_seconds <= 0
                || value.valid_from_unix_seconds <= 0
                || value.valid_until_unix_seconds <= value.valid_from_unix_seconds
                || value.registry_generation == 0
            {
                return Err(PolicyActivationClientError::Protocol);
            }
            Ok(PolicyActivationProjection {
                record_id: value.record_id,
                policy_name: value.policy_name,
                policy_version: value.policy_version,
                policy_hash: value.policy_hash,
                approval_epoch: value.approval_epoch.to_string(),
                status: "APPROVED".into(),
                registered_at: timestamp(value.registered_at_unix_seconds),
                valid_from: timestamp(value.valid_from_unix_seconds),
                valid_until: timestamp(value.valid_until_unix_seconds),
                registry_generation: value.registry_generation.to_string(),
            })
        }
        PolicyActivationFailureWire::InvalidRequest => {
            Err(PolicyActivationClientError::InvalidRequest)
        }
        PolicyActivationFailureWire::Conflict => Err(PolicyActivationClientError::Conflict),
        PolicyActivationFailureWire::CandidateRejected => {
            Err(PolicyActivationClientError::CandidateRejected)
        }
        PolicyActivationFailureWire::Unavailable => Err(PolicyActivationClientError::Unavailable),
    }
}

fn timestamp(seconds: i64) -> PublicTimestamp {
    PublicTimestamp {
        seconds: seconds.to_string(),
        nanos: 0,
    }
}
