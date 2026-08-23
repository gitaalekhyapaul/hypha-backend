use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};

use super::frame::{read_frame_async, read_frame_to_eof_async, write_frame_async};
use super::{
    MAX_ONE_SHOT_REQUEST_BYTES, MAX_ONE_SHOT_RESULT_BYTES, OneShotFailure, OneShotFailureClass,
    OneShotProtocolError, OneShotResult, decode_request, decode_result, encode_result,
};

pub const PROOF_LAUNCHER_SOCKET_PATH_ENV: &str = "RAZIEL_PROOF_LAUNCHER_SOCKET_PATH";
pub const PROVER_CHILD_SOCKET_PATH_ENV: &str = "RAZIEL_PROVER_CHILD_SOCKET_PATH";
pub const PROVER_CHILD_PATH_ENV: &str = "RAZIEL_PROVER_CHILD_PATH";

const CHILD_SOCKET_ENV: &str = "RAZIEL_ONE_SHOT_SOCKET_PATH";
const CANCEL_CONTROL: u8 = 1;
const MAX_UNIX_SOCKET_PATH_BYTES: usize = 100;
const MAX_EXECUTABLE_PATH_BYTES: usize = 4_096;
const CHILD_START_TIMEOUT: Duration = Duration::from_secs(30);
const CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(30);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(25);
const LAUNCHER_PROOF_DEADLINE: Duration = Duration::from_secs(60 * 60);

/// Fixed owner configuration for the persistent proof-sandbox launcher.
pub struct ProofSandboxLauncherConfig {
    launcher_socket_path: PathBuf,
    child_socket_path: PathBuf,
    child_executable: PathBuf,
    child_arguments: Vec<OsString>,
    child_environment: Vec<(OsString, OsString)>,
}

impl ProofSandboxLauncherConfig {
    pub fn from_env() -> Result<Self, ProofSandboxLauncherError> {
        let launcher_socket_path = required_path(PROOF_LAUNCHER_SOCKET_PATH_ENV)?;
        let child_socket_path = required_path(PROVER_CHILD_SOCKET_PATH_ENV)?;
        let child_executable = required_path(PROVER_CHILD_PATH_ENV)?;
        Self::new(
            launcher_socket_path,
            child_socket_path,
            child_executable,
            Vec::new(),
            Vec::new(),
        )
    }

    fn new(
        launcher_socket_path: PathBuf,
        child_socket_path: PathBuf,
        child_executable: PathBuf,
        child_arguments: Vec<OsString>,
        child_environment: Vec<(OsString, OsString)>,
    ) -> Result<Self, ProofSandboxLauncherError> {
        if launcher_socket_path == child_socket_path
            || !valid_absolute_path(&launcher_socket_path, MAX_UNIX_SOCKET_PATH_BYTES)
            || !valid_absolute_path(&child_socket_path, MAX_UNIX_SOCKET_PATH_BYTES)
            || !valid_absolute_path(&child_executable, MAX_EXECUTABLE_PATH_BYTES)
        {
            return Err(ProofSandboxLauncherError::Configuration);
        }
        let launcher_owner = validate_private_parent(&launcher_socket_path)?;
        if validate_private_parent(&child_socket_path)? != launcher_owner {
            return Err(ProofSandboxLauncherError::Configuration);
        }
        validate_child_executable(&child_executable)?;
        Ok(Self {
            launcher_socket_path,
            child_socket_path,
            child_executable,
            child_arguments,
            child_environment,
        })
    }

    #[cfg(test)]
    fn for_test(
        launcher_socket_path: PathBuf,
        child_socket_path: PathBuf,
        child_executable: PathBuf,
        child_arguments: Vec<OsString>,
        child_environment: Vec<(OsString, OsString)>,
    ) -> Result<Self, ProofSandboxLauncherError> {
        Self::new(
            launcher_socket_path,
            child_socket_path,
            child_executable,
            child_arguments,
            child_environment,
        )
    }
}

impl Debug for ProofSandboxLauncherConfig {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProofSandboxLauncherConfig")
            .field("paths", &"<redacted>")
            .finish()
    }
}

/// Runs the persistent one-job-at-a-time launcher from fixed environment settings.
pub async fn run_launcher_from_env() -> Result<(), ProofSandboxLauncherError> {
    let config = ProofSandboxLauncherConfig::from_env()?;
    ProofSandboxLauncher::bind(config)?.run().await
}

struct ProofSandboxLauncher {
    config: ProofSandboxLauncherConfig,
    listener: UnixListener,
    _socket: SocketGuard,
}

impl ProofSandboxLauncher {
    fn bind(config: ProofSandboxLauncherConfig) -> Result<Self, ProofSandboxLauncherError> {
        let (listener, socket) = bind_private_listener(&config.launcher_socket_path)?;
        Ok(Self {
            config,
            listener,
            _socket: socket,
        })
    }

    async fn run(self) -> Result<(), ProofSandboxLauncherError> {
        loop {
            tokio::select! {
                accepted = self.listener.accept() => {
                    let (client, _) = accepted.map_err(|_| ProofSandboxLauncherError::Transport)?;
                    if matches!(
                        self.serve_client(client).await,
                        Err(ProofSandboxLauncherError::TerminationUnconfirmed)
                    ) {
                        return Err(ProofSandboxLauncherError::TerminationUnconfirmed);
                    }
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|_| ProofSandboxLauncherError::Transport)?;
                    return Ok(());
                }
            }
        }
    }

    async fn serve_client(&self, client: UnixStream) -> Result<(), ProofSandboxLauncherError> {
        let (mut client_reader, mut client_writer) = client.into_split();
        let request_bytes = read_frame_async(&mut client_reader, MAX_ONE_SHOT_REQUEST_BYTES)
            .await
            .map_err(|_| ProofSandboxLauncherError::Protocol)?;
        let request =
            decode_request(&request_bytes).map_err(|_| ProofSandboxLauncherError::Protocol)?;
        let correlation_id = request.correlation_id().to_owned();
        drop(request);

        let control = read_client_control(&mut client_reader);
        tokio::pin!(control);
        let (child_listener, _child_socket) =
            bind_private_listener(&self.config.child_socket_path)?;
        let mut child = self.spawn_child()?;

        let child_stream = match accept_child(
            &child_listener,
            &mut child,
            control.as_mut(),
            CHILD_START_TIMEOUT,
        )
        .await?
        {
            ChildAccept::Connected(stream) => stream,
            ChildAccept::Cancelled(control) => {
                terminate_child(&mut child).await?;
                return acknowledge_control(control, &correlation_id, &mut client_writer).await;
            }
            ChildAccept::Exited => {
                return send_failure(
                    OneShotFailureClass::Internal,
                    &correlation_id,
                    &mut client_writer,
                )
                .await;
            }
        };

        let (mut child_reader, mut child_writer) = child_stream.into_split();
        write_frame_async(
            &mut child_writer,
            &request_bytes,
            MAX_ONE_SHOT_REQUEST_BYTES,
        )
        .await
        .map_err(|_| ProofSandboxLauncherError::Transport)?;
        child_writer
            .shutdown()
            .await
            .map_err(|_| ProofSandboxLauncherError::Transport)?;

        let child_result = read_frame_to_eof_async(&mut child_reader, MAX_ONE_SHOT_RESULT_BYTES);
        tokio::pin!(child_result);
        let started = Instant::now();
        loop {
            let remaining = LAUNCHER_PROOF_DEADLINE.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                terminate_child(&mut child).await?;
                return send_failure(
                    OneShotFailureClass::Internal,
                    &correlation_id,
                    &mut client_writer,
                )
                .await;
            }

            tokio::select! {
                result = &mut child_result => {
                    let status = wait_for_child_exit(&mut child).await?;
                    return forward_child_result(
                        result,
                        status,
                        &correlation_id,
                        &mut client_writer,
                    ).await;
                }
                client_control = control.as_mut() => {
                    terminate_child(&mut child).await?;
                    return acknowledge_control(
                        client_control,
                        &correlation_id,
                        &mut client_writer,
                    ).await;
                }
                () = tokio::time::sleep(CHILD_POLL_INTERVAL.min(remaining)) => {}
            }
        }
    }

    fn spawn_child(&self) -> Result<Child, ProofSandboxLauncherError> {
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
            .map_err(|_| ProofSandboxLauncherError::ChildProcess)
    }
}

enum ChildAccept {
    Connected(UnixStream),
    Cancelled(ClientControl),
    Exited,
}

async fn accept_child<F>(
    listener: &UnixListener,
    child: &mut Child,
    mut control: Pin<&mut F>,
    timeout: Duration,
) -> Result<ChildAccept, ProofSandboxLauncherError>
where
    F: Future<Output = ClientControl>,
{
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|_| ProofSandboxLauncherError::ChildProcess)?
            .is_some()
        {
            return Ok(ChildAccept::Exited);
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            terminate_child(child).await?;
            return Ok(ChildAccept::Exited);
        }
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|_| ProofSandboxLauncherError::Transport)?;
                return Ok(ChildAccept::Connected(stream));
            }
            client_control = control.as_mut() => {
                return Ok(ChildAccept::Cancelled(client_control));
            }
            () = tokio::time::sleep(CHILD_POLL_INTERVAL.min(remaining)) => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClientControl {
    Cancel,
    Disconnect,
    Invalid,
}

async fn read_client_control(reader: &mut (impl AsyncRead + Unpin)) -> ClientControl {
    match reader.read_u8().await {
        Ok(CANCEL_CONTROL) => {
            let mut trailing = [0_u8; 1];
            match reader.read(&mut trailing).await {
                Ok(0) => ClientControl::Cancel,
                _ => ClientControl::Invalid,
            }
        }
        Ok(_) => ClientControl::Invalid,
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            ClientControl::Disconnect
        }
        Err(_) => ClientControl::Invalid,
    }
}

async fn acknowledge_control(
    control: ClientControl,
    correlation_id: &str,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), ProofSandboxLauncherError> {
    match control {
        ClientControl::Cancel => {
            send_failure(OneShotFailureClass::Cancelled, correlation_id, writer).await
        }
        ClientControl::Invalid => {
            send_failure(OneShotFailureClass::InvalidRequest, correlation_id, writer).await
        }
        ClientControl::Disconnect => Ok(()),
    }
}

async fn forward_child_result(
    result: Result<Vec<u8>, OneShotProtocolError>,
    status: ExitStatus,
    correlation_id: &str,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), ProofSandboxLauncherError> {
    let Ok(bytes) = result else {
        return send_failure(OneShotFailureClass::Internal, correlation_id, writer).await;
    };
    if !status.success() || decode_result(&bytes).is_err() {
        return send_failure(OneShotFailureClass::Internal, correlation_id, writer).await;
    }
    write_frame_async(writer, &bytes, MAX_ONE_SHOT_RESULT_BYTES)
        .await
        .map_err(|_| ProofSandboxLauncherError::Transport)?;
    writer
        .shutdown()
        .await
        .map_err(|_| ProofSandboxLauncherError::Transport)
}

async fn send_failure(
    class: OneShotFailureClass,
    correlation_id: &str,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), ProofSandboxLauncherError> {
    let failure = OneShotFailure::new(class, correlation_id.to_owned())
        .map_err(|_| ProofSandboxLauncherError::Protocol)?;
    let bytes = encode_result(&OneShotResult::Failure(failure))
        .map_err(|_| ProofSandboxLauncherError::Protocol)?;
    write_frame_async(writer, &bytes, MAX_ONE_SHOT_RESULT_BYTES)
        .await
        .map_err(|_| ProofSandboxLauncherError::Transport)?;
    writer
        .shutdown()
        .await
        .map_err(|_| ProofSandboxLauncherError::Transport)
}

async fn terminate_child(child: &mut Child) -> Result<ExitStatus, ProofSandboxLauncherError> {
    if let Some(status) = child
        .try_wait()
        .map_err(|_| ProofSandboxLauncherError::ChildProcess)?
    {
        return Ok(status);
    }
    child
        .start_kill()
        .map_err(|_| ProofSandboxLauncherError::ChildProcess)?;
    wait_for_child_exit(child).await
}

async fn wait_for_child_exit(child: &mut Child) -> Result<ExitStatus, ProofSandboxLauncherError> {
    tokio::time::timeout(CHILD_EXIT_TIMEOUT, child.wait())
        .await
        .map_err(|_| ProofSandboxLauncherError::TerminationUnconfirmed)?
        .map_err(|_| ProofSandboxLauncherError::ChildProcess)
}

fn bind_private_listener(
    path: &Path,
) -> Result<(UnixListener, SocketGuard), ProofSandboxLauncherError> {
    let owner_uid = validate_private_parent(path)?;
    remove_stale_socket(path, owner_uid)?;
    let listener = UnixListener::bind(path).map_err(|_| ProofSandboxLauncherError::Transport)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| ProofSandboxLauncherError::Transport)?;
    validate_socket(path, owner_uid)?;
    Ok((
        listener,
        SocketGuard {
            path: path.to_owned(),
            owner_uid,
        },
    ))
}

fn remove_stale_socket(path: &Path, owner_uid: u32) -> Result<(), ProofSandboxLauncherError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ProofSandboxLauncherError::Configuration),
    };
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != owner_uid
        || metadata.nlink() != 1
    {
        return Err(ProofSandboxLauncherError::Configuration);
    }
    std::fs::remove_file(path).map_err(|_| ProofSandboxLauncherError::Transport)
}

fn validate_socket(path: &Path, owner_uid: u32) -> Result<(), ProofSandboxLauncherError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| ProofSandboxLauncherError::Configuration)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != owner_uid
        || metadata.nlink() != 1
    {
        return Err(ProofSandboxLauncherError::Configuration);
    }
    Ok(())
}

fn validate_private_parent(path: &Path) -> Result<u32, ProofSandboxLauncherError> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or(ProofSandboxLauncherError::Configuration)?;
    let metadata =
        std::fs::symlink_metadata(parent).map_err(|_| ProofSandboxLauncherError::Configuration)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(ProofSandboxLauncherError::Configuration);
    }
    Ok(metadata.uid())
}

fn validate_child_executable(path: &Path) -> Result<(), ProofSandboxLauncherError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| ProofSandboxLauncherError::Configuration)?;
    let mode = metadata.permissions().mode();
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || mode & 0o022 != 0
        || mode & 0o111 == 0
        || metadata.nlink() != 1
    {
        return Err(ProofSandboxLauncherError::Configuration);
    }
    Ok(())
}

fn required_path(name: &str) -> Result<PathBuf, ProofSandboxLauncherError> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ProofSandboxLauncherError::Configuration)
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
pub enum ProofSandboxLauncherError {
    Configuration,
    Transport,
    Protocol,
    ChildProcess,
    TerminationUnconfirmed,
}

impl fmt::Display for ProofSandboxLauncherError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "proof sandbox launcher configuration is invalid",
            Self::Transport => "proof sandbox launcher transport failed",
            Self::Protocol => "proof sandbox launcher protocol failed",
            Self::ChildProcess => "proof sandbox child process failed",
            Self::TerminationUnconfirmed => "proof sandbox child termination is unconfirmed",
        })
    }
}

impl Error for ProofSandboxLauncherError {}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixStream as StdUnixStream;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use bullmq::worker::CancellationToken;

    use super::*;
    use crate::one_shot::UnixSocketExecutor;
    use crate::one_shot::tests::private_envelope;
    use crate::worker::{
        CapabilityExecutionRequest, CapabilityExecutor, ExecutionControl, ExecutorFailureClass,
    };

    const TRUSTED_NOW: i64 = 1_800_000_000;
    const CONTROLLED_CHILD_ENV: &str = "RAZIEL_CONTROLLED_CHILD_BEHAVIOR";
    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[tokio::test]
    async fn launcher_relays_closed_child_failure_classes() {
        for (behavior, expected) in [
            ("host-rejected", ExecutorFailureClass::Rejected),
            ("witness-rejected", ExecutorFailureClass::Rejected),
            ("proof-failed", ExecutorFailureClass::Rejected),
            ("journal-mismatch", ExecutorFailureClass::Rejected),
            ("resource-profile", ExecutorFailureClass::Unavailable),
        ] {
            let fixture = LauncherFixture::new(behavior);
            let execution = fixture.executor.execute(
                CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
                ExecutionControl::new(Duration::from_secs(5), CancellationToken::new()),
            );
            assert_eq!(execution.await, Err(expected));
            fixture.server.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn launcher_kills_and_waits_for_a_cancelled_child() {
        let fixture = LauncherFixture::new("hang");
        let cancellation = CancellationToken::new();
        let execution = tokio::spawn(fixture.executor.execute(
            CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
            ExecutionControl::new(Duration::from_secs(5), cancellation.clone()),
        ));
        tokio::time::sleep(Duration::from_millis(150)).await;
        cancellation.cancel();
        assert_eq!(
            execution.await.unwrap(),
            Err(ExecutorFailureClass::Cancelled)
        );
        fixture.server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn launcher_closes_crashed_and_malformed_children() {
        for behavior in ["crash", "malformed"] {
            let fixture = LauncherFixture::new(behavior);
            let result = fixture
                .executor
                .execute(
                    CapabilityExecutionRequest::new(private_envelope(), TRUSTED_NOW),
                    ExecutionControl::new(Duration::from_secs(5), CancellationToken::new()),
                )
                .await;
            assert_eq!(result, Err(ExecutorFailureClass::Unavailable));
            fixture.server.await.unwrap().unwrap();
        }
    }

    #[test]
    #[ignore = "spawned only by launcher process tests"]
    fn controlled_child_entry() {
        let behavior = std::env::var(CONTROLLED_CHILD_ENV).unwrap();
        let socket_path = std::env::var_os(CHILD_SOCKET_ENV).unwrap();
        let mut stream = StdUnixStream::connect(socket_path).unwrap();
        let request_bytes =
            super::super::read_frame_to_eof(&mut stream, super::super::MAX_ONE_SHOT_REQUEST_BYTES)
                .unwrap();
        let request = super::super::decode_request(&request_bytes).unwrap();

        match behavior.as_str() {
            "host-rejected" | "witness-rejected" | "proof-failed" | "journal-mismatch"
            | "resource-profile" => {
                let class = match behavior.as_str() {
                    "host-rejected" => super::super::OneShotFailureClass::HostRejected,
                    "witness-rejected" => super::super::OneShotFailureClass::WitnessRejected,
                    "proof-failed" => super::super::OneShotFailureClass::ProofFailed,
                    "journal-mismatch" => super::super::OneShotFailureClass::JournalMismatch,
                    "resource-profile" => super::super::OneShotFailureClass::ResourceProfile,
                    _ => unreachable!(),
                };
                let failure =
                    super::super::OneShotFailure::new(class, request.correlation_id().to_owned())
                        .unwrap();
                let result =
                    super::super::encode_result(&super::super::OneShotResult::Failure(failure))
                        .unwrap();
                super::super::write_frame(
                    &mut stream,
                    &result,
                    super::super::MAX_ONE_SHOT_RESULT_BYTES,
                )
                .unwrap();
            }
            "hang" => std::thread::sleep(Duration::from_secs(60)),
            "malformed" => {
                let bytes = [0xff];
                stream
                    .write_all(&(bytes.len() as u32).to_be_bytes())
                    .unwrap();
                stream.write_all(&bytes).unwrap();
            }
            "crash" => panic!("controlled child crash"),
            _ => panic!("unknown controlled child behavior"),
        }
    }

    struct LauncherFixture {
        _root: TestRoot,
        executor: UnixSocketExecutor,
        server: tokio::task::JoinHandle<Result<(), ProofSandboxLauncherError>>,
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
                "one_shot::launcher::tests::controlled_child_entry",
                "--ignored",
                "--nocapture",
            ]
            .into_iter()
            .map(OsString::from)
            .collect();
            let config = ProofSandboxLauncherConfig::for_test(
                launcher_socket.clone(),
                child_socket,
                executable,
                arguments,
                vec![(
                    OsString::from(CONTROLLED_CHILD_ENV),
                    OsString::from(behavior),
                )],
            )
            .unwrap();
            let launcher = ProofSandboxLauncher::bind(config).unwrap();
            let owner_uid = std::fs::symlink_metadata(&launcher_socket).unwrap().uid();
            let executor = UnixSocketExecutor::new(launcher_socket, owner_uid).unwrap();
            let server = tokio::spawn(async move {
                let (client, _) = launcher
                    .listener
                    .accept()
                    .await
                    .map_err(|_| ProofSandboxLauncherError::Transport)?;
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
                    "rz-launcher-{}-{timestamp}-{sequence}",
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
}
