use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use prost::Message;

use super::wire::{
    RegistryWriterFailureWire, RegistryWriterRequestWire, RegistryWriterResponseWire,
};
use super::{
    MAX_FRAME_BYTES, PROTOCOL_VERSION, PolicyRegistryWriterResult, RegistryWriterClientError,
    RegistryWriterResult, SOCKET_TIMEOUT,
};

pub(super) fn send_root_request(
    socket_path: &Path,
    request: RegistryWriterRequestWire,
) -> Result<RegistryWriterResult, RegistryWriterClientError> {
    let response = send_request(socket_path, request)?;
    let root_record = response
        .root_record
        .ok_or(RegistryWriterClientError::Protocol)?;
    if response.policy_record.is_some()
        || root_record.registry_generation != response.registry_generation
    {
        return Err(RegistryWriterClientError::Protocol);
    }
    Ok(RegistryWriterResult {
        registry_generation: response.registry_generation,
        root_record,
    })
}

pub(super) fn send_policy_request(
    socket_path: &Path,
    request: RegistryWriterRequestWire,
) -> Result<PolicyRegistryWriterResult, RegistryWriterClientError> {
    let response = send_request(socket_path, request)?;
    let policy_record = response
        .policy_record
        .ok_or(RegistryWriterClientError::Protocol)?;
    if response.root_record.is_some()
        || policy_record.registry_generation != response.registry_generation
        || policy_record.status != raziel_proto::raziel::v1::PolicyStatus::Approved as i32
    {
        return Err(RegistryWriterClientError::Protocol);
    }
    Ok(PolicyRegistryWriterResult {
        registry_generation: response.registry_generation,
        policy_record,
    })
}

fn send_request(
    socket_path: &Path,
    request: RegistryWriterRequestWire,
) -> Result<RegistryWriterResponseWire, RegistryWriterClientError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| RegistryWriterClientError::Unavailable)?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| RegistryWriterClientError::Unavailable)?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| RegistryWriterClientError::Unavailable)?;
    write_message(&mut stream, &request).map_err(|_| RegistryWriterClientError::Protocol)?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|_| RegistryWriterClientError::Protocol)?;
    let response: RegistryWriterResponseWire =
        read_message(&mut stream).map_err(|_| RegistryWriterClientError::Protocol)?;
    require_eof(&mut stream).map_err(|_| RegistryWriterClientError::Protocol)?;
    response_result(response)
}

pub(super) fn read_request(
    stream: &mut UnixStream,
) -> Result<RegistryWriterRequestWire, RegistryWriterClientError> {
    let request = read_message(stream).map_err(|_| RegistryWriterClientError::Protocol)?;
    require_eof(stream).map_err(|_| RegistryWriterClientError::Protocol)?;
    Ok(request)
}

pub(super) fn write_response(
    stream: &mut UnixStream,
    response: &RegistryWriterResponseWire,
) -> Result<(), RegistryWriterClientError> {
    write_message(stream, response).map_err(|_| RegistryWriterClientError::Protocol)
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
    response: RegistryWriterResponseWire,
) -> Result<RegistryWriterResponseWire, RegistryWriterClientError> {
    if response.format_version != PROTOCOL_VERSION {
        return Err(RegistryWriterClientError::Protocol);
    }
    let failure = RegistryWriterFailureWire::try_from(response.failure)
        .map_err(|_| RegistryWriterClientError::Protocol)?;
    match failure {
        RegistryWriterFailureWire::Unspecified => {
            if response.registry_generation == 0
                || (response.root_record.is_some() == response.policy_record.is_some())
            {
                return Err(RegistryWriterClientError::Protocol);
            }
            Ok(response)
        }
        RegistryWriterFailureWire::StaleGeneration => {
            failure_response(&response, RegistryWriterClientError::StaleGeneration)
        }
        RegistryWriterFailureWire::Conflict => {
            failure_response(&response, RegistryWriterClientError::Conflict)
        }
        RegistryWriterFailureWire::InvalidRequest => {
            failure_response(&response, RegistryWriterClientError::InvalidRequest)
        }
        RegistryWriterFailureWire::RegistryRejected => {
            failure_response(&response, RegistryWriterClientError::RegistryRejected)
        }
        RegistryWriterFailureWire::PublicationFailed => {
            failure_response(&response, RegistryWriterClientError::PublicationFailed)
        }
    }
}

fn failure_response(
    response: &RegistryWriterResponseWire,
    error: RegistryWriterClientError,
) -> Result<RegistryWriterResponseWire, RegistryWriterClientError> {
    if response.registry_generation != 0
        || response.root_record.is_some()
        || response.policy_record.is_some()
    {
        Err(RegistryWriterClientError::Protocol)
    } else {
        Err(error)
    }
}
