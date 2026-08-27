use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use prost::Message;

use super::wire::{
    PolicyAuthorityFailureWire, PolicyAuthorityRequestWire, PolicyAuthorityResponseWire,
};
use super::{
    MAX_FRAME_BYTES, PROTOCOL_VERSION, PolicyAuthorityClientError, PolicyAuthorityResult,
    SOCKET_TIMEOUT,
};

pub(super) fn send_request(
    socket_path: &Path,
    request: PolicyAuthorityRequestWire,
) -> Result<PolicyAuthorityResult, PolicyAuthorityClientError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| PolicyAuthorityClientError::Unavailable)?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| PolicyAuthorityClientError::Unavailable)?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| PolicyAuthorityClientError::Unavailable)?;
    write_message(&mut stream, &request).map_err(|_| PolicyAuthorityClientError::Protocol)?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|_| PolicyAuthorityClientError::Protocol)?;
    let response: PolicyAuthorityResponseWire =
        read_message(&mut stream).map_err(|_| PolicyAuthorityClientError::Protocol)?;
    require_eof(&mut stream).map_err(|_| PolicyAuthorityClientError::Protocol)?;
    response_result(response)
}

pub(super) fn read_request(
    stream: &mut UnixStream,
) -> Result<PolicyAuthorityRequestWire, PolicyAuthorityClientError> {
    let request = read_message(stream).map_err(|_| PolicyAuthorityClientError::Protocol)?;
    require_eof(stream).map_err(|_| PolicyAuthorityClientError::Protocol)?;
    Ok(request)
}

pub(super) fn write_response(
    stream: &mut UnixStream,
    response: &PolicyAuthorityResponseWire,
) -> Result<(), PolicyAuthorityClientError> {
    write_message(stream, response).map_err(|_| PolicyAuthorityClientError::Protocol)
}

pub(super) fn write_message<M: Message>(stream: &mut UnixStream, message: &M) -> Result<(), ()> {
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
    response: PolicyAuthorityResponseWire,
) -> Result<PolicyAuthorityResult, PolicyAuthorityClientError> {
    if response.format_version != PROTOCOL_VERSION {
        return Err(PolicyAuthorityClientError::Protocol);
    }
    let failure = PolicyAuthorityFailureWire::try_from(response.failure)
        .map_err(|_| PolicyAuthorityClientError::Protocol)?;
    match failure {
        PolicyAuthorityFailureWire::Unspecified => {
            let staged_policy_record = response
                .staged_policy_record
                .ok_or(PolicyAuthorityClientError::Protocol)?;
            if staged_policy_record.status != 0
                || staged_policy_record.registered_at.is_some()
                || staged_policy_record.registry_generation != 0
            {
                return Err(PolicyAuthorityClientError::Protocol);
            }
            Ok(PolicyAuthorityResult {
                staged_policy_record,
            })
        }
        PolicyAuthorityFailureWire::InvalidRequest => {
            Err(PolicyAuthorityClientError::InvalidRequest)
        }
        PolicyAuthorityFailureWire::StaleEpoch => Err(PolicyAuthorityClientError::StaleEpoch),
        PolicyAuthorityFailureWire::Conflict => Err(PolicyAuthorityClientError::Conflict),
        PolicyAuthorityFailureWire::CandidateRejected => {
            Err(PolicyAuthorityClientError::CandidateRejected)
        }
        PolicyAuthorityFailureWire::Unavailable => Err(PolicyAuthorityClientError::Unavailable),
    }
}
