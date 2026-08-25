use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Debug, Formatter};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};

use super::frame::{read_frame_async, read_frame_to_eof_async, write_frame_async};
use super::{
    MAX_VERIFICATION_WORKER_REQUEST_BYTES, MAX_VERIFICATION_WORKER_RESULT_BYTES,
    VERIFICATION_DEADLINE_SECONDS, VerificationWorkerFailure, VerificationWorkerFailureClass,
    VerificationWorkerProtocolError, VerificationWorkerResult, decode_request, decode_result,
    encode_result,
};

pub const VERIFIER_LAUNCHER_SOCKET_PATH_ENV: &str = "RAZIEL_VERIFIER_LAUNCHER_SOCKET_PATH";
pub const VERIFIER_CHILD_SOCKET_PATH_ENV: &str = "RAZIEL_VERIFIER_CHILD_SOCKET_PATH";
pub const VERIFIER_CHILD_PATH_ENV: &str = "RAZIEL_VERIFIER_CHILD_PATH";

const CHILD_SOCKET_ENV: &str = "RAZIEL_VERIFIER_ONE_SHOT_SOCKET_PATH";
const CANCEL_CONTROL: u8 = 1;
const MAX_UNIX_SOCKET_PATH_BYTES: usize = 100;
const MAX_EXECUTABLE_PATH_BYTES: usize = 4_096;
const CLIENT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CHILD_START_TIMEOUT: Duration = Duration::from_secs(5);
const CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct VerifierSandboxLauncherConfig {
    launcher_socket_path: PathBuf,
    child_socket_path: PathBuf,
    child_executable: PathBuf,
    child_arguments: Vec<OsString>,
    child_environment: Vec<(OsString, OsString)>,
    verification_deadline: Duration,
}

impl VerifierSandboxLauncherConfig {
    pub fn from_env() -> Result<Self, VerifierSandboxLauncherError> {
        Self::new(
            required_path(VERIFIER_LAUNCHER_SOCKET_PATH_ENV)?,
            required_path(VERIFIER_CHILD_SOCKET_PATH_ENV)?,
            required_path(VERIFIER_CHILD_PATH_ENV)?,
            Vec::new(),
            Vec::new(),
            Duration::from_secs(u64::from(VERIFICATION_DEADLINE_SECONDS)),
        )
    }

    fn new(
        launcher_socket_path: PathBuf,
        child_socket_path: PathBuf,
        child_executable: PathBuf,
        child_arguments: Vec<OsString>,
        child_environment: Vec<(OsString, OsString)>,
        verification_deadline: Duration,
    ) -> Result<Self, VerifierSandboxLauncherError> {
        if launcher_socket_path == child_socket_path
            || !valid_absolute_path(&launcher_socket_path, MAX_UNIX_SOCKET_PATH_BYTES)
            || !valid_absolute_path(&child_socket_path, MAX_UNIX_SOCKET_PATH_BYTES)
            || !valid_absolute_path(&child_executable, MAX_EXECUTABLE_PATH_BYTES)
        {
            return Err(VerifierSandboxLauncherError::Configuration);
        }
        let owner = validate_private_parent(&launcher_socket_path)?;
        if validate_private_parent(&child_socket_path)? != owner {
            return Err(VerifierSandboxLauncherError::Configuration);
        }
        validate_child_executable(&child_executable)?;
        Ok(Self {
            launcher_socket_path,
            child_socket_path,
            child_executable,
            child_arguments,
            child_environment,
            verification_deadline,
        })
    }

    #[cfg(test)]
    fn for_test(
        launcher_socket_path: PathBuf,
        child_socket_path: PathBuf,
        child_executable: PathBuf,
        child_arguments: Vec<OsString>,
        child_environment: Vec<(OsString, OsString)>,
        verification_deadline: Duration,
    ) -> Result<Self, VerifierSandboxLauncherError> {
        Self::new(
            launcher_socket_path,
            child_socket_path,
            child_executable,
            child_arguments,
            child_environment,
            verification_deadline,
        )
    }
}

impl Debug for VerifierSandboxLauncherConfig {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifierSandboxLauncherConfig")
            .field("paths", &"<redacted>")
            .finish()
    }
}

pub async fn run_launcher_from_env() -> Result<(), VerifierSandboxLauncherError> {
    VerifierSandboxLauncher::bind(VerifierSandboxLauncherConfig::from_env()?)?
        .run()
        .await
}

struct VerifierSandboxLauncher {
    config: VerifierSandboxLauncherConfig,
    listener: UnixListener,
    _socket: SocketGuard,
}

impl VerifierSandboxLauncher {
    fn bind(config: VerifierSandboxLauncherConfig) -> Result<Self, VerifierSandboxLauncherError> {
        let (listener, socket) = bind_private_listener(&config.launcher_socket_path)?;
        Ok(Self {
            config,
            listener,
            _socket: socket,
        })
    }

    async fn run(self) -> Result<(), VerifierSandboxLauncherError> {
        loop {
            tokio::select! {
                accepted = self.listener.accept() => {
                    let (client, _) = accepted.map_err(|_| VerifierSandboxLauncherError::Transport)?;
                    if matches!(
                        self.serve_client(client).await,
                        Err(VerifierSandboxLauncherError::TerminationUnconfirmed)
                    ) {
                        return Err(VerifierSandboxLauncherError::TerminationUnconfirmed);
                    }
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|_| VerifierSandboxLauncherError::Transport)?;
                    return Ok(());
                }
            }
        }
    }

    async fn serve_client(&self, client: UnixStream) -> Result<(), VerifierSandboxLauncherError> {
        let (mut client_reader, mut client_writer) = client.into_split();
        let request_bytes = tokio::time::timeout(
            CLIENT_REQUEST_TIMEOUT,
            read_frame_async(&mut client_reader, MAX_VERIFICATION_WORKER_REQUEST_BYTES),
        )
        .await
        .map_err(|_| VerifierSandboxLauncherError::Protocol)?
        .map_err(|_| VerifierSandboxLauncherError::Protocol)?;
        let request =
            decode_request(&request_bytes).map_err(|_| VerifierSandboxLauncherError::Protocol)?;
        let correlation_id = request.correlation_id().to_owned();
        drop(request);

        let (child_listener, _child_socket) =
            bind_private_listener(&self.config.child_socket_path)?;
        let mut child = self.spawn_child()?;
        let child_stream = tokio::select! {
            accepted = child_listener.accept() => {
                accepted
                    .map(|(stream, _)| stream)
                    .map_err(|_| VerifierSandboxLauncherError::Transport)?
            }
            status = child.wait() => {
                status.map_err(|_| VerifierSandboxLauncherError::ChildProcess)?;
                return send_failure(
                    VerificationWorkerFailureClass::Internal,
                    &correlation_id,
                    &mut client_writer,
                ).await;
            }
            _ = tokio::time::sleep(CHILD_START_TIMEOUT) => {
                terminate_child(&mut child).await?;
                return send_failure(
                    VerificationWorkerFailureClass::Internal,
                    &correlation_id,
                    &mut client_writer,
                ).await;
            }
        };

        let (mut child_reader, mut child_writer) = child_stream.into_split();
        write_frame_async(
            &mut child_writer,
            &request_bytes,
            MAX_VERIFICATION_WORKER_REQUEST_BYTES,
        )
        .await
        .map_err(|_| VerifierSandboxLauncherError::Transport)?;
        child_writer
            .shutdown()
            .await
            .map_err(|_| VerifierSandboxLauncherError::Transport)?;

        let child_result =
            read_frame_to_eof_async(&mut child_reader, MAX_VERIFICATION_WORKER_RESULT_BYTES);
        tokio::pin!(child_result);
        let client_control = client_reader.read_u8();
        tokio::pin!(client_control);
        tokio::select! {
            result = &mut child_result => {
                let status = wait_for_child_exit(&mut child).await?;
                forward_child_result(
                    result,
                    status,
                    &correlation_id,
                    &mut client_writer,
                ).await
            }
            control = &mut client_control => {
                terminate_child(&mut child).await?;
                match control {
                    Ok(CANCEL_CONTROL) => send_failure(
                        VerificationWorkerFailureClass::Cancelled,
                        &correlation_id,
                        &mut client_writer,
                    ).await,
                    Ok(_) => send_failure(
                        VerificationWorkerFailureClass::InvalidRequest,
                        &correlation_id,
                        &mut client_writer,
                    ).await,
                    Err(_) => Ok(()),
                }
            }
            _ = tokio::time::sleep(self.config.verification_deadline) => {
                terminate_child(&mut child).await?;
                send_failure(
                    VerificationWorkerFailureClass::Internal,
                    &correlation_id,
                    &mut client_writer,
                ).await
            }
        }
    }

    fn spawn_child(&self) -> Result<Child, VerifierSandboxLauncherError> {
        let mut command = Command::new(&self.config.child_executable);
        command
            .args(&self.config.child_arguments)
            .envs(self.config.child_environment.iter().cloned())
            .env(CHILD_SOCKET_ENV, &self.config.child_socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        command
            .spawn()
            .map_err(|_| VerifierSandboxLauncherError::ChildProcess)
    }
}

async fn forward_child_result(
    result: Result<Vec<u8>, VerificationWorkerProtocolError>,
    status: ExitStatus,
    correlation_id: &str,
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
) -> Result<(), VerifierSandboxLauncherError> {
    let Ok(bytes) = result else {
        return send_failure(
            VerificationWorkerFailureClass::Internal,
            correlation_id,
            writer,
        )
        .await;
    };
    let decoded = decode_result(&bytes);
    let correlation_matches = match &decoded {
        Ok(VerificationWorkerResult::Success(success)) => {
            success.correlation_id() == correlation_id
        }
        Ok(VerificationWorkerResult::Failure(failure)) => {
            failure.correlation_id() == correlation_id
        }
        Err(_) => false,
    };
    if !status.success() || !correlation_matches {
        return send_failure(
            VerificationWorkerFailureClass::Internal,
            correlation_id,
            writer,
        )
        .await;
    }
    write_frame_async(writer, &bytes, MAX_VERIFICATION_WORKER_RESULT_BYTES)
        .await
        .map_err(|_| VerifierSandboxLauncherError::Transport)?;
    writer
        .shutdown()
        .await
        .map_err(|_| VerifierSandboxLauncherError::Transport)
}

async fn send_failure(
    class: VerificationWorkerFailureClass,
    correlation_id: &str,
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
) -> Result<(), VerifierSandboxLauncherError> {
    let failure = VerificationWorkerFailure::new(class, correlation_id.to_owned())
        .map_err(|_| VerifierSandboxLauncherError::Protocol)?;
    let bytes = encode_result(&VerificationWorkerResult::Failure(failure))
        .map_err(|_| VerifierSandboxLauncherError::Protocol)?;
    write_frame_async(writer, &bytes, MAX_VERIFICATION_WORKER_RESULT_BYTES)
        .await
        .map_err(|_| VerifierSandboxLauncherError::Transport)?;
    writer
        .shutdown()
        .await
        .map_err(|_| VerifierSandboxLauncherError::Transport)
}

async fn terminate_child(child: &mut Child) -> Result<ExitStatus, VerifierSandboxLauncherError> {
    if let Some(status) = child
        .try_wait()
        .map_err(|_| VerifierSandboxLauncherError::ChildProcess)?
    {
        return Ok(status);
    }
    child
        .start_kill()
        .map_err(|_| VerifierSandboxLauncherError::ChildProcess)?;
    wait_for_child_exit(child).await
}

async fn wait_for_child_exit(
    child: &mut Child,
) -> Result<ExitStatus, VerifierSandboxLauncherError> {
    tokio::time::timeout(CHILD_EXIT_TIMEOUT, child.wait())
        .await
        .map_err(|_| VerifierSandboxLauncherError::TerminationUnconfirmed)?
        .map_err(|_| VerifierSandboxLauncherError::ChildProcess)
}

fn bind_private_listener(
    path: &Path,
) -> Result<(UnixListener, SocketGuard), VerifierSandboxLauncherError> {
    let owner_uid = validate_private_parent(path)?;
    remove_stale_socket(path, owner_uid)?;
    let listener = UnixListener::bind(path).map_err(|_| VerifierSandboxLauncherError::Transport)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| VerifierSandboxLauncherError::Transport)?;
    validate_socket(path, owner_uid)?;
    Ok((
        listener,
        SocketGuard {
            path: path.to_owned(),
            owner_uid,
        },
    ))
}

fn remove_stale_socket(path: &Path, owner_uid: u32) -> Result<(), VerifierSandboxLauncherError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(VerifierSandboxLauncherError::Configuration),
    };
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != owner_uid
        || metadata.nlink() != 1
    {
        return Err(VerifierSandboxLauncherError::Configuration);
    }
    std::fs::remove_file(path).map_err(|_| VerifierSandboxLauncherError::Transport)
}

fn validate_socket(path: &Path, owner_uid: u32) -> Result<(), VerifierSandboxLauncherError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| VerifierSandboxLauncherError::Configuration)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != owner_uid
        || metadata.nlink() != 1
    {
        return Err(VerifierSandboxLauncherError::Configuration);
    }
    Ok(())
}

fn validate_private_parent(path: &Path) -> Result<u32, VerifierSandboxLauncherError> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or(VerifierSandboxLauncherError::Configuration)?;
    let metadata = std::fs::symlink_metadata(parent)
        .map_err(|_| VerifierSandboxLauncherError::Configuration)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(VerifierSandboxLauncherError::Configuration);
    }
    Ok(metadata.uid())
}

fn validate_child_executable(path: &Path) -> Result<(), VerifierSandboxLauncherError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| VerifierSandboxLauncherError::Configuration)?;
    let mode = metadata.permissions().mode();
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || mode & 0o022 != 0
        || mode & 0o111 == 0
        || metadata.nlink() != 1
    {
        return Err(VerifierSandboxLauncherError::Configuration);
    }
    Ok(())
}

fn required_path(name: &str) -> Result<PathBuf, VerifierSandboxLauncherError> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(VerifierSandboxLauncherError::Configuration)
}

fn valid_absolute_path(path: &Path, maximum_bytes: usize) -> bool {
    path.is_absolute()
        && path.as_os_str().as_encoded_bytes().len() <= maximum_bytes
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
}

struct SocketGuard {
    path: PathBuf,
    owner_uid: u32,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if validate_socket(&self.path, self.owner_uid).is_ok() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifierSandboxLauncherError {
    Configuration,
    Transport,
    Protocol,
    ChildProcess,
    TerminationUnconfirmed,
}

impl fmt::Display for VerifierSandboxLauncherError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "verifier sandbox launcher configuration is invalid",
            Self::Transport => "verifier sandbox launcher transport failed",
            Self::Protocol => "verifier sandbox launcher protocol failed",
            Self::ChildProcess => "verifier sandbox child process failed",
            Self::TerminationUnconfirmed => "verifier sandbox child termination is unconfirmed",
        })
    }
}

impl Error for VerifierSandboxLauncherError {}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixStream as StdUnixStream;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use raziel_authorization_core::{AuthorizationPublicValuesV1, encode_public_values};
    use raziel_prover::TrustedProgramKey;

    use super::*;
    use crate::worker::{
        IsolatedVerificationRequest, UnixSocketVerificationExecutor, VerificationExecutionError,
        VerificationExecutor, VerificationWorkerSuccess,
    };

    const KEY_ID: &str =
        "sp1-vk-v1:e96d8ab7a16a315c10b7adb0ddc15007c59cf5cf36d671a36bb75f1b0ce698b9";
    const SP1_KEY_HASH: &str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    const CORRELATION_ID: &str = "abababababababababababababababababababababababababababababababab";
    const CONTROLLED_CHILD_ENV: &str = "RAZIEL_CONTROLLED_VERIFIER_BEHAVIOR";
    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[tokio::test]
    async fn launcher_relays_success_and_proof_rejection() {
        let success = LauncherFixture::new("success");
        let result = success.executor.verify(request()).await.unwrap();
        assert_eq!(result.verification_key_id(), KEY_ID);
        success.server.await.unwrap().unwrap();

        let rejection = LauncherFixture::new("proof-rejected");
        assert_eq!(
            rejection.executor.verify(request()).await,
            Err(VerificationExecutionError::Rejected)
        );
        rejection.server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn launcher_contains_crash_malformed_output_and_timeout() {
        for behavior in ["crash", "malformed", "hang"] {
            let fixture = LauncherFixture::new(behavior);
            assert_eq!(
                fixture.executor.verify(request()).await,
                Err(VerificationExecutionError::Unavailable)
            );
            fixture.server.await.unwrap().unwrap();
        }
    }

    #[test]
    #[ignore = "spawned only by verifier launcher process tests"]
    fn controlled_child_entry() {
        let behavior = std::env::var(CONTROLLED_CHILD_ENV).unwrap();
        let socket_path = std::env::var_os(CHILD_SOCKET_ENV).unwrap();
        let mut stream = StdUnixStream::connect(socket_path).unwrap();
        let request_bytes = crate::worker::read_frame_to_eof(
            &mut stream,
            crate::worker::MAX_VERIFICATION_WORKER_REQUEST_BYTES,
        )
        .unwrap();
        let request = crate::worker::decode_request(&request_bytes).unwrap();

        match behavior.as_str() {
            "success" => {
                let success = VerificationWorkerSuccess::new(
                    request.correlation_id().to_owned(),
                    KEY_ID.to_owned(),
                )
                .unwrap();
                write_result(&mut stream, VerificationWorkerResult::Success(success));
            }
            "proof-rejected" => {
                let failure = VerificationWorkerFailure::new(
                    VerificationWorkerFailureClass::ProofRejected,
                    request.correlation_id().to_owned(),
                )
                .unwrap();
                write_result(&mut stream, VerificationWorkerResult::Failure(failure));
            }
            "malformed" => {
                stream.write_all(&1_u32.to_be_bytes()).unwrap();
                stream.write_all(&[0xff]).unwrap();
            }
            "hang" => std::thread::sleep(Duration::from_secs(60)),
            "crash" => panic!("controlled child crash"),
            _ => panic!("unknown controlled child behavior"),
        }
    }

    fn write_result(stream: &mut StdUnixStream, result: VerificationWorkerResult) {
        let bytes = crate::worker::encode_result(&result).unwrap();
        crate::worker::write_frame(
            stream,
            &bytes,
            crate::worker::MAX_VERIFICATION_WORKER_RESULT_BYTES,
        )
        .unwrap();
    }

    struct LauncherFixture {
        _root: TestRoot,
        executor: UnixSocketVerificationExecutor,
        server: tokio::task::JoinHandle<Result<(), VerifierSandboxLauncherError>>,
    }

    impl LauncherFixture {
        fn new(behavior: &str) -> Self {
            let root = TestRoot::new();
            std::fs::create_dir(root.path()).unwrap();
            std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
            let launcher_socket = root.path().join("launcher.sock");
            let child_socket = root.path().join("child.sock");
            let executable = std::env::current_exe().unwrap();
            let arguments = [
                "--exact",
                "worker::launcher::tests::controlled_child_entry",
                "--ignored",
                "--nocapture",
            ]
            .into_iter()
            .map(OsString::from)
            .collect();
            let config = VerifierSandboxLauncherConfig::for_test(
                launcher_socket.clone(),
                child_socket,
                executable,
                arguments,
                vec![(
                    OsString::from(CONTROLLED_CHILD_ENV),
                    OsString::from(behavior),
                )],
                Duration::from_millis(250),
            )
            .unwrap();
            let launcher = VerifierSandboxLauncher::bind(config).unwrap();
            let owner_uid = std::fs::symlink_metadata(&launcher_socket).unwrap().uid();
            let executor = UnixSocketVerificationExecutor::new(launcher_socket, owner_uid).unwrap();
            let server = tokio::spawn(async move {
                let (client, _) = launcher
                    .listener
                    .accept()
                    .await
                    .map_err(|_| VerifierSandboxLauncherError::Transport)?;
                launcher.serve_client(client).await
            });
            Self {
                _root: root,
                executor,
                server,
            }
        }
    }

    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new() -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self {
                path: PathBuf::from("/tmp").join(format!(
                    "rz-vfy-{}-{timestamp}-{sequence}",
                    std::process::id()
                )),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn request() -> IsolatedVerificationRequest {
        IsolatedVerificationRequest::new(
            vec![0xaa],
            encode_public_values(&AuthorizationPublicValuesV1 {
                format_version: 1,
                policy_hash: [0x11; 32],
                relationship_snapshot_root: [0x22; 32],
                capability_request_commitment: [0x33; 32],
                capability_scope_hash: [0x44; 32],
                issued_at_unix_seconds: 10,
                expires_at_unix_seconds: 20,
                issuance_nullifier: [0x55; 32],
            })
            .unwrap(),
            TrustedProgramKey::from_registry_record(KEY_ID, SP1_KEY_HASH).unwrap(),
            CORRELATION_ID.to_owned(),
        )
        .unwrap()
    }
}
