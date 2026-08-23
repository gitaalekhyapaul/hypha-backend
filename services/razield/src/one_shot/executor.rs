use std::fmt::{self, Debug, Formatter, Write as _};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use super::frame::{read_frame_to_eof_async, write_frame_async};
use super::{
    MAX_ONE_SHOT_REQUEST_BYTES, MAX_ONE_SHOT_RESULT_BYTES, OneShotFailureClass,
    OneShotProtocolError, OneShotRequest, OneShotResult, PROOF_LAUNCHER_SOCKET_PATH_ENV,
    decode_result, encode_request,
};
use crate::worker::{
    CapabilityExecutionRequest, CapabilityExecutor, ExecutionControl, ExecutionFuture,
    ExecutorFailureClass,
};

const CANCEL_CONTROL: u8 = 1;
const MAX_SOCKET_PATH_BYTES: usize = 100;

/// Supervisor-side client for the fixed proof-sandbox launcher socket.
pub struct UnixSocketExecutor {
    socket_path: PathBuf,
    expected_owner_uid: u32,
}

impl UnixSocketExecutor {
    pub fn from_env(expected_owner_uid: u32) -> Result<Self, OneShotExecutorConfigError> {
        let socket_path = std::env::var(PROOF_LAUNCHER_SOCKET_PATH_ENV)
            .map(PathBuf::from)
            .map_err(|_| OneShotExecutorConfigError::InvalidSocketPath)?;
        Self::new(socket_path, expected_owner_uid)
    }

    pub fn new(
        socket_path: PathBuf,
        expected_owner_uid: u32,
    ) -> Result<Self, OneShotExecutorConfigError> {
        if !valid_absolute_path(&socket_path) {
            return Err(OneShotExecutorConfigError::InvalidSocketPath);
        }
        Ok(Self {
            socket_path,
            expected_owner_uid,
        })
    }
}

impl CapabilityExecutor for UnixSocketExecutor {
    fn execute(
        &self,
        request: CapabilityExecutionRequest,
        control: ExecutionControl,
    ) -> ExecutionFuture {
        let socket_path = self.socket_path.clone();
        let expected_owner_uid = self.expected_owner_uid;
        let correlation_id = lowercase_hex(request.envelope().capability_request_commitment());
        let encoded = OneShotRequest::from_execution(request, correlation_id.clone())
            .and_then(|request| encode_request(&request));

        Box::pin(async move {
            let request_bytes = encoded.map_err(map_request_error)?;
            validate_socket(&socket_path, expected_owner_uid)?;

            let stream = tokio::select! {
                result = UnixStream::connect(&socket_path) => {
                    result.map_err(|_| ExecutorFailureClass::Unavailable)?
                }
                () = control.cancelled() => return Err(ExecutorFailureClass::Cancelled),
            };
            let (mut reader, mut writer) = stream.into_split();
            write_frame_async(&mut writer, &request_bytes, MAX_ONE_SHOT_REQUEST_BYTES)
                .await
                .map_err(|_| ExecutorFailureClass::Unavailable)?;

            let result = read_frame_to_eof_async(&mut reader, MAX_ONE_SHOT_RESULT_BYTES);
            tokio::pin!(result);
            tokio::select! {
                result = &mut result => {
                    drop(writer);
                    let result = result.map_err(map_async_result_error)?;
                    decode_execution_result(result, &correlation_id)
                }
                () = control.cancelled() => {
                    let _ = writer.write_all(&[CANCEL_CONTROL]).await;
                    let _ = writer.shutdown().await;
                    match result.await {
                        Ok(bytes) if valid_termination_acknowledgement(&bytes, &correlation_id) => {
                            Err(ExecutorFailureClass::Cancelled)
                        }
                        _ => std::future::pending().await,
                    }
                }
            }
        })
    }
}

fn valid_termination_acknowledgement(bytes: &[u8], expected_correlation_id: &str) -> bool {
    matches!(
        decode_result(bytes),
        Ok(OneShotResult::Failure(failure))
            if failure.class == OneShotFailureClass::Cancelled
                && failure.correlation_id == expected_correlation_id
    )
}

impl Debug for UnixSocketExecutor {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnixSocketExecutor")
            .field("socket_path", &"<redacted>")
            .field("expected_owner_uid", &self.expected_owner_uid)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OneShotExecutorConfigError {
    InvalidSocketPath,
}

impl fmt::Display for OneShotExecutorConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("one-shot executor configuration is invalid")
    }
}

impl std::error::Error for OneShotExecutorConfigError {}

fn decode_execution_result(
    bytes: Vec<u8>,
    expected_correlation_id: &str,
) -> Result<super::OneShotSuccess, ExecutorFailureClass> {
    match decode_result(&bytes).map_err(map_result_error)? {
        OneShotResult::Success(success) => Ok(*success),
        OneShotResult::Failure(failure) => {
            if failure.correlation_id != expected_correlation_id {
                return Err(ExecutorFailureClass::MalformedResult);
            }
            Err(match failure.class {
                OneShotFailureClass::Cancelled => ExecutorFailureClass::Cancelled,
                OneShotFailureClass::InvalidRequest
                | OneShotFailureClass::WitnessRejected
                | OneShotFailureClass::HostRejected
                | OneShotFailureClass::ProofFailed
                | OneShotFailureClass::JournalMismatch => ExecutorFailureClass::Rejected,
                OneShotFailureClass::TrustedState
                | OneShotFailureClass::ResourceProfile
                | OneShotFailureClass::Internal => ExecutorFailureClass::Unavailable,
            })
        }
    }
}

fn map_request_error(_error: OneShotProtocolError) -> ExecutorFailureClass {
    ExecutorFailureClass::Rejected
}

fn map_result_error(_error: OneShotProtocolError) -> ExecutorFailureClass {
    ExecutorFailureClass::MalformedResult
}

fn map_async_result_error(error: OneShotProtocolError) -> ExecutorFailureClass {
    match error {
        OneShotProtocolError::TrailingFrameData
        | OneShotProtocolError::ResultTooLarge
        | OneShotProtocolError::MalformedResult
        | OneShotProtocolError::UnsupportedVersion
        | OneShotProtocolError::InvalidResult => ExecutorFailureClass::MalformedResult,
        _ => ExecutorFailureClass::Unavailable,
    }
}

fn validate_socket(path: &Path, expected_owner_uid: u32) -> Result<(), ExecutorFailureClass> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or(ExecutorFailureClass::Unavailable)?;
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| ExecutorFailureClass::Unavailable)?;
    if parent_metadata.file_type().is_symlink()
        || !parent_metadata.is_dir()
        || parent_metadata.permissions().mode() & 0o7777 != 0o700
        || parent_metadata.uid() != expected_owner_uid
    {
        return Err(ExecutorFailureClass::Unavailable);
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| ExecutorFailureClass::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != expected_owner_uid
        || metadata.nlink() != 1
    {
        return Err(ExecutorFailureClass::Unavailable);
    }
    Ok(())
}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().as_encoded_bytes().len() <= MAX_SOCKET_PATH_BYTES
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}
