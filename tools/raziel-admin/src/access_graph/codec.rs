use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use prost::Message;

use super::wire::{AccessGraphFailureWire, AccessGraphRequestWire, AccessGraphResponseWire};
use super::{
    AccessGraphClientError, AccessGraphProjection, MAX_FRAME_BYTES, PROTOCOL_VERSION,
    PublicTimestamp, SOCKET_TIMEOUT,
};

pub(super) fn send_request(
    socket_path: &Path,
    request: AccessGraphRequestWire,
) -> Result<AccessGraphProjection, AccessGraphClientError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| AccessGraphClientError::Unavailable)?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| AccessGraphClientError::Unavailable)?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| AccessGraphClientError::Unavailable)?;
    write_message(&mut stream, &request).map_err(|_| AccessGraphClientError::Protocol)?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|_| AccessGraphClientError::Protocol)?;
    let response: AccessGraphResponseWire =
        read_message(&mut stream).map_err(|_| AccessGraphClientError::Protocol)?;
    require_eof(&mut stream).map_err(|_| AccessGraphClientError::Protocol)?;
    response_result(response)
}

pub(super) fn read_request(
    stream: &mut UnixStream,
) -> Result<AccessGraphRequestWire, AccessGraphClientError> {
    let request = read_message(stream).map_err(|_| AccessGraphClientError::Protocol)?;
    require_eof(stream).map_err(|_| AccessGraphClientError::Protocol)?;
    Ok(request)
}

pub(super) fn write_response(
    stream: &mut UnixStream,
    response: &AccessGraphResponseWire,
) -> Result<(), AccessGraphClientError> {
    write_message(stream, response).map_err(|_| AccessGraphClientError::Protocol)
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
    response: AccessGraphResponseWire,
) -> Result<AccessGraphProjection, AccessGraphClientError> {
    if response.format_version != PROTOCOL_VERSION {
        return Err(AccessGraphClientError::Protocol);
    }
    let failure = AccessGraphFailureWire::try_from(response.failure)
        .map_err(|_| AccessGraphClientError::Protocol)?;
    match failure {
        AccessGraphFailureWire::Unspecified => {
            let value = response
                .projection
                .ok_or(AccessGraphClientError::Protocol)?;
            if value.policy_record_id.is_empty()
                || value.policy_hash.len() != 64
                || value.snapshot_id.is_empty()
                || value.relationship_root.len() != 64
                || value.epoch == 0
                || value.issued_at_unix_seconds <= 0
                || value.expires_at_unix_seconds <= value.issued_at_unix_seconds
                || value.accepted_relationship_count == 0
                || value.registry_generation == 0
            {
                return Err(AccessGraphClientError::Protocol);
            }
            Ok(AccessGraphProjection {
                policy_record_id: value.policy_record_id,
                policy_hash: value.policy_hash,
                snapshot_id: value.snapshot_id,
                relationship_root: value.relationship_root,
                epoch: value.epoch.to_string(),
                issued_at: timestamp(value.issued_at_unix_seconds),
                expires_at: timestamp(value.expires_at_unix_seconds),
                accepted_relationship_count: value.accepted_relationship_count,
                registry_generation: value.registry_generation.to_string(),
                status: "ACCEPTED".into(),
            })
        }
        AccessGraphFailureWire::InvalidRequest => Err(AccessGraphClientError::InvalidRequest),
        AccessGraphFailureWire::Conflict => Err(AccessGraphClientError::Conflict),
        AccessGraphFailureWire::CandidateRejected => Err(AccessGraphClientError::CandidateRejected),
        AccessGraphFailureWire::Unavailable => Err(AccessGraphClientError::Unavailable),
    }
}

fn timestamp(seconds: i64) -> PublicTimestamp {
    PublicTimestamp {
        seconds: seconds.to_string(),
        nanos: 0,
    }
}
