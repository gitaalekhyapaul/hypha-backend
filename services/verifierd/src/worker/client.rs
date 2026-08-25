use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use super::frame::{read_frame_to_eof_async, write_frame_async};
use super::{
    IsolatedVerificationRequest, MAX_VERIFICATION_WORKER_REQUEST_BYTES,
    MAX_VERIFICATION_WORKER_RESULT_BYTES, VERIFICATION_DEADLINE_SECONDS,
    VERIFIER_LAUNCHER_SOCKET_PATH_ENV, VerificationWorkerFailureClass, VerificationWorkerResult,
    VerificationWorkerSuccess, decode_result, encode_request,
};

const CANCEL_CONTROL: u8 = 1;
const MAX_SOCKET_PATH_BYTES: usize = 100;
const CANCELLATION_ACK_TIMEOUT: Duration = Duration::from_secs(5);

pub type VerificationFuture = Pin<
    Box<dyn Future<Output = Result<VerificationWorkerSuccess, VerificationExecutionError>> + Send>,
>;

pub trait VerificationExecutor: Send + Sync + 'static {
    fn verify(&self, request: IsolatedVerificationRequest) -> VerificationFuture;
}

pub struct UnixSocketVerificationExecutor {
    socket_path: PathBuf,
    expected_owner_uid: u32,
}

impl UnixSocketVerificationExecutor {
    pub fn from_env(expected_owner_uid: u32) -> Result<Self, VerificationExecutionError> {
        let socket_path = std::env::var_os(VERIFIER_LAUNCHER_SOCKET_PATH_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or(VerificationExecutionError::Configuration)?;
        Self::new(socket_path, expected_owner_uid)
    }

    pub fn new(
        socket_path: PathBuf,
        expected_owner_uid: u32,
    ) -> Result<Self, VerificationExecutionError> {
        if !valid_absolute_path(&socket_path) {
            return Err(VerificationExecutionError::Configuration);
        }
        Ok(Self {
            socket_path,
            expected_owner_uid,
        })
    }
}

impl VerificationExecutor for UnixSocketVerificationExecutor {
    fn verify(&self, request: IsolatedVerificationRequest) -> VerificationFuture {
        let socket_path = self.socket_path.clone();
        let expected_owner_uid = self.expected_owner_uid;
        let correlation_id = request.correlation_id().to_owned();
        let encoded = encode_request(&request);

        Box::pin(async move {
            let request_bytes = encoded.map_err(|_| VerificationExecutionError::Rejected)?;
            validate_socket(&socket_path, expected_owner_uid)?;
            let stream = UnixStream::connect(&socket_path)
                .await
                .map_err(|_| VerificationExecutionError::Unavailable)?;
            let (mut reader, mut writer) = stream.into_split();
            write_frame_async(
                &mut writer,
                &request_bytes,
                MAX_VERIFICATION_WORKER_REQUEST_BYTES,
            )
            .await
            .map_err(|_| VerificationExecutionError::Unavailable)?;

            let response =
                read_frame_to_eof_async(&mut reader, MAX_VERIFICATION_WORKER_RESULT_BYTES);
            match tokio::time::timeout(
                Duration::from_secs(u64::from(VERIFICATION_DEADLINE_SECONDS)),
                response,
            )
            .await
            {
                Ok(Ok(bytes)) => decode_execution_result(&bytes, &correlation_id),
                Ok(Err(_)) => Err(VerificationExecutionError::Unavailable),
                Err(_) => {
                    let _ = writer.write_all(&[CANCEL_CONTROL]).await;
                    let _ = writer.shutdown().await;
                    let _ = tokio::time::timeout(
                        CANCELLATION_ACK_TIMEOUT,
                        read_frame_to_eof_async(&mut reader, MAX_VERIFICATION_WORKER_RESULT_BYTES),
                    )
                    .await;
                    Err(VerificationExecutionError::TimedOut)
                }
            }
        })
    }
}

fn decode_execution_result(
    bytes: &[u8],
    expected_correlation_id: &str,
) -> Result<VerificationWorkerSuccess, VerificationExecutionError> {
    match decode_result(bytes).map_err(|_| VerificationExecutionError::MalformedResult)? {
        VerificationWorkerResult::Success(success) => {
            if success.correlation_id() != expected_correlation_id {
                return Err(VerificationExecutionError::MalformedResult);
            }
            Ok(success)
        }
        VerificationWorkerResult::Failure(failure) => {
            if failure.correlation_id() != expected_correlation_id {
                return Err(VerificationExecutionError::MalformedResult);
            }
            Err(match failure.class() {
                VerificationWorkerFailureClass::InvalidRequest
                | VerificationWorkerFailureClass::ProofRejected => {
                    VerificationExecutionError::Rejected
                }
                VerificationWorkerFailureClass::Cancelled
                | VerificationWorkerFailureClass::ResourceProfile
                | VerificationWorkerFailureClass::Internal => {
                    VerificationExecutionError::Unavailable
                }
            })
        }
    }
}

fn validate_socket(path: &Path, expected_owner_uid: u32) -> Result<(), VerificationExecutionError> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or(VerificationExecutionError::Unavailable)?;
    let parent_metadata =
        std::fs::symlink_metadata(parent).map_err(|_| VerificationExecutionError::Unavailable)?;
    if parent_metadata.file_type().is_symlink()
        || !parent_metadata.is_dir()
        || parent_metadata.permissions().mode() & 0o7777 != 0o700
        || parent_metadata.uid() != expected_owner_uid
    {
        return Err(VerificationExecutionError::Unavailable);
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| VerificationExecutionError::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != expected_owner_uid
        || metadata.nlink() != 1
    {
        return Err(VerificationExecutionError::Unavailable);
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

impl Debug for UnixSocketVerificationExecutor {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnixSocketVerificationExecutor")
            .field("socket_path", &"<redacted>")
            .field("expected_owner_uid", &self.expected_owner_uid)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationExecutionError {
    Configuration,
    Rejected,
    Unavailable,
    TimedOut,
    MalformedResult,
}

impl fmt::Display for VerificationExecutionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("isolated proof verification failed")
    }
}

impl std::error::Error for VerificationExecutionError {}
