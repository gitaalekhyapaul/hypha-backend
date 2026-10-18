use std::fs;
use std::marker::PhantomData;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use prost::Message;
use raziel_authority::{AuthoritySigner, PolicyApprovalRole};
use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_proto::raziel::v1 as proto;
use serde_json::json;
use sha2::{Digest, Sha256};
use spicedb_rs_proto::authzed::api::v1 as api;
use tonic::body::Body;
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::codegen::{BoxFuture, Service, http};
use tonic::server::{NamedService, UnaryService};
use tonic::{Request, Response, Status};

use crate::io::{atomic_write_new, lowercase_hex};
use crate::policy_activation::store::{PolicyActivationStore, trusted_now};
use crate::policy_activation::wire::PolicyActivationRequestWire;
use crate::policy_package::{PolicyPackageKey, PolicyPackageStore};
use crate::policy_version::{AllocatedPolicyVersion, PolicyVersionStore};

use super::super::compiler::{Compiler, Input};
use super::super::files::Artifacts;
use super::super::network::{SchemaApi, SchemaClient};
use super::super::{PolicyPublisher, PublicationConfig};

const TRUST_DOMAIN: &str = "publication-integration-test";
const TOKEN: &str = "publication-test-only";

pub(super) struct Fixture {
    // Field order stops children and the listener before deleting their private files.
    _children: Vec<ChildGuard>,
    schema: Option<SchemaHarness>,
    endpoint: String,
    compiler: PathBuf,
    bin_dir: PathBuf,
    root: PrivateRoot,
}

impl Fixture {
    pub fn new() -> Self {
        Self::with_endpoint(std::env::var("RAZIEL_POLICY_PUBLICATION_TEST_SPICEDB_ENDPOINT").ok())
    }

    pub fn new_controlled() -> Self {
        Self::with_endpoint(None)
    }

    fn with_endpoint(endpoint: Option<String>) -> Self {
        let compiler = PathBuf::from(
            std::env::var_os("RAZIEL_POLICY_PUBLICATION_TEST_COMPILER").expect(
                "set RAZIEL_POLICY_PUBLICATION_TEST_COMPILER for this ignored integration test",
            ),
        );
        let bin_dir = std::env::var_os("RAZIEL_POLICY_PUBLICATION_TEST_BIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/workspace/target/b0064-linux/debug".into());
        assert!(compiler.is_absolute() && compiler.is_file());
        for name in [
            "raziel-admin",
            "raziel-policy-authority",
            "raziel-registry-writer",
        ] {
            assert!(
                bin_dir.join(name).is_file(),
                "build the requested admin binaries first"
            );
        }
        let root = PrivateRoot::new();
        for name in [
            "registry",
            "candidates",
            "activation",
            "versions",
            "packages",
            "publication",
            "ipc",
            "schema-checks",
        ] {
            private_dir(&root.0.join(name));
        }
        let key_path = root.0.join("policy.pk8");
        seed_registry(&root.0.join("registry"), &key_path);
        atomic_write_new(&root.0.join("spicedb.token"), TOKEN.as_bytes()).unwrap();
        fs::set_permissions(
            root.0.join("spicedb.token"),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        let mut children = Vec::new();
        let authority_socket = root.0.join("ipc/authority.sock");
        let writer_socket = root.0.join("ipc/writer.sock");
        children.push(ChildGuard::spawn(
            &bin_dir.join("raziel-policy-authority"),
            &[
                ("RAZIEL_POLICY_CANDIDATE_ROOT", root.0.join("candidates")),
                ("RAZIEL_REGISTRY_PATH", root.0.join("registry")),
                (
                    "RAZIEL_POLICY_AUTHORITY_SOCKET_PATH",
                    authority_socket.clone(),
                ),
                ("RAZIEL_POLICY_KEY_PATH", key_path),
            ],
            &authority_socket,
        ));
        children.push(ChildGuard::spawn(
            &bin_dir.join("raziel-registry-writer"),
            &[
                ("RAZIEL_REGISTRY_PATH", root.0.join("registry")),
                ("RAZIEL_REGISTRY_WRITER_SOCKET_PATH", writer_socket.clone()),
            ],
            &writer_socket,
        ));
        let schema = endpoint
            .is_none()
            .then(|| SchemaHarness::new(compiler.clone(), root.0.join("schema-checks")));
        let endpoint = endpoint.unwrap_or_else(|| schema.as_ref().unwrap().endpoint.clone());
        let fixture = Self {
            _children: children,
            schema,
            endpoint,
            compiler,
            bin_dir,
            root,
        };
        assert!(
            fixture.schema_bytes().is_empty(),
            "publication test requires an isolated empty SpiceDB datastore"
        );
        fixture
    }

    pub fn publisher(&self) -> PolicyPublisher {
        let root = &self.root.0;
        let activation = PolicyActivationStore::new(
            root.join("candidates"),
            root.join("activation"),
            root.join("registry"),
            root.join("ipc/authority.sock"),
            root.join("ipc/writer.sock"),
            TRUST_DOMAIN,
        )
        .unwrap();
        PolicyPublisher::new(
            PublicationConfig {
                root: root.join("publication"),
                package_root: root.join("packages"),
                version_root: root.join("versions"),
                candidate_root: root.join("candidates"),
                registry_root: root.join("registry"),
                compiler: self.compiler.clone(),
                legacy_root: None,
                token_path: root.join("spicedb.token"),
                endpoint: self.endpoint.clone(),
                trust_domain_id: TRUST_DOMAIN.into(),
            },
            activation,
        )
        .unwrap()
    }

    pub fn prepare(
        &self,
        candidate_id: &str,
        family: Option<&str>,
        alternative: bool,
    ) -> AllocatedPolicyVersion {
        let allocation = PolicyVersionStore::new(self.root.0.join("versions"))
            .unwrap()
            .allocate(
                candidate_id,
                &lowercase_hex(&Sha256::digest(candidate_id.as_bytes())),
                family,
            )
            .unwrap();
        let candidate = self.root.0.join("candidates").join(candidate_id);
        private_dir(&candidate);
        let identity = allocation.identity();
        let (extra, permission) = if alternative {
            ("  relation auditor: agent\n", "viewer + auditor")
        } else {
            ("", "viewer")
        };
        let source = format!(
            "definition agent {{}}\n\ndefinition {} {{\n  relation viewer: agent\n{extra}  permission query = {permission}\n}}\n",
            identity.resource_type(),
        );
        atomic_write_new(&candidate.join("schema.zed"), source.as_bytes()).unwrap();
        let config =
            json!({"config_version":1,"limits":TRUSTED_POLICY_CEILINGS.policy_limits,"caveats":{}});
        atomic_write_new(
            &candidate.join("compiler.json"),
            &serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        run(Command::new(&self.compiler)
            .args([
                "compile",
                "--schema",
                "schema.zed",
                "--config",
                "compiler.json",
                "--out",
                "policy.pb",
                "--diagnostic-out",
                "policy.json",
                "--compiler-version",
                "1.54.0",
            ])
            .current_dir(&candidate));
        let now = trusted_now().unwrap();
        run(Command::new(self.bin_dir.join("raziel-admin"))
            .arg("inspect-policy")
            .arg("--policy")
            .arg(candidate.join("policy.pb"))
            .args([
                "--candidate-id",
                candidate_id,
                "--record-id",
                &format!("record-{candidate_id}"),
                "--policy-name",
                &identity.signed_policy_name(),
                "--policy-version",
                &identity.version().to_string(),
                "--resource-type",
                &identity.resource_type(),
                "--permission",
                "query",
                "--rule-summary",
                permission,
                "--expected-approval-epoch",
                "1",
                "--valid-from",
                &(now - 30).to_string(),
                "--valid-until",
                &(now + 7200).to_string(),
                "--created-at",
                &now.to_string(),
                "--expires-at",
                &(now + 600).to_string(),
            ])
            .arg("--output")
            .arg(candidate.join("review.json"))
            .arg("--state-output")
            .arg(candidate.join("candidate.pb")));
        allocation
    }

    pub fn packages(&self) -> PolicyPackageStore {
        PolicyPackageStore::new(
            self.root.0.join("packages"),
            self.root.0.join("registry"),
            TRUST_DOMAIN,
        )
        .unwrap()
    }

    pub fn schema(&self) -> &SchemaHarness {
        self.schema.as_ref().expect("controlled schema test")
    }
    pub fn controlled_write_count(&self) -> Option<usize> {
        self.schema.as_ref().map(SchemaHarness::writes)
    }
    pub fn schema_bytes(&self) -> Vec<u8> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut client =
                SchemaClient::connect(&self.endpoint, &self.root.0.join("spicedb.token"))
                    .await
                    .unwrap();
            client.read().await.unwrap()
        })
    }

    pub fn package_bytes(&self, key: &PolicyPackageKey) -> Vec<u8> {
        fs::read(self.root.0.join("packages").join(format!(
            "{}.{}.pb",
            key.version_id(),
            key.approval_epoch()
        )))
        .unwrap()
    }

    pub fn registry_bytes(&self) -> Vec<u8> {
        fs::read(self.root.0.join("registry/current.pb")).unwrap()
    }
    pub fn registry(&self) -> proto::RegistryGenerationManifest {
        proto::RegistryGenerationManifest::decode(self.registry_bytes().as_slice()).unwrap()
    }
    pub fn installed_path(&self) -> PathBuf {
        self.root.0.join("publication/installed.json")
    }
    pub fn installed_bytes(&self) -> Vec<u8> {
        fs::read(self.installed_path()).unwrap()
    }
    pub fn pending_path(&self) -> PathBuf {
        self.root.0.join("publication/pending.json")
    }

    pub fn assert_matches_candidates(&self, candidates: &[&AllocatedPolicyVersion]) {
        let compiler = Compiler::new(&self.compiler, &self.root.0.join("publication")).unwrap();
        let inputs = candidates
            .iter()
            .map(|allocation| Input {
                version_id: Some(allocation.identity().version_id().into()),
                artifacts: Artifacts::load(
                    &self
                        .root
                        .0
                        .join("candidates")
                        .join(allocation.candidate_id()),
                )
                .unwrap(),
            })
            .collect::<Vec<_>>();
        let expected = compiler.compose(&inputs).unwrap();
        assert!(
            compiler
                .equivalent(&expected.schema, &self.schema_bytes())
                .unwrap()
        );
    }
}

pub(super) fn request(candidate_id: &str) -> PolicyActivationRequestWire {
    PolicyActivationRequestWire {
        format_version: 1,
        candidate_id: candidate_id.into(),
        approved_by: "test-operator".into(),
        operation: 0,
        role_request: Vec::new(),
    }
}

fn seed_registry(registry: &Path, key_path: &Path) {
    let now = trusted_now().unwrap();
    let der = [
        vec![
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20,
        ],
        vec![0x61; 32],
    ]
    .concat();
    let signer = AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&der).unwrap();
    atomic_write_new(key_path, &der).unwrap();
    fs::set_permissions(key_path, fs::Permissions::from_mode(0o400)).unwrap();
    let key = proto::AuthorityKeyRecord {
        format_version: 1,
        trust_domain_id: TRUST_DOMAIN.into(),
        key_id: signer.key_id().into(),
        role: proto::AuthorityRole::PolicyApproval as i32,
        public_key: signer.public_key_bytes().to_vec(),
        status: proto::AuthorityKeyStatus::Active as i32,
        valid_from: Some(timestamp(now - 3600)),
        valid_until: Some(timestamp(now + 24 * 3600)),
        revoked_at: None,
        revocation_reason: String::new(),
        registry_generation: 1,
    };
    let key_bytes = key.encode_to_vec();
    atomic_write_new(&registry.join("key.pb"), &key_bytes).unwrap();
    let manifest = proto::RegistryGenerationManifest {
        format_version: 1,
        trust_domain_id: TRUST_DOMAIN.into(),
        generation: 1,
        published_at: Some(timestamp(now)),
        authority_keys: vec![proto::RegistryFile {
            relative_path: "key.pb".into(),
            sha256: Sha256::digest(&key_bytes).to_vec(),
        }],
        policies: vec![],
        accepted_roots: vec![],
    };
    atomic_write_new(&registry.join("current.pb"), &manifest.encode_to_vec()).unwrap();
}

fn timestamp(seconds: i64) -> prost_types::Timestamp {
    prost_types::Timestamp { seconds, nanos: 0 }
}
fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}

struct PrivateRoot(PathBuf);
impl PrivateRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rz-pub-{}-{unique}", std::process::id()));
        private_dir(&path);
        Self(path)
    }
}
impl Drop for PrivateRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);
impl ChildGuard {
    fn spawn(binary: &Path, variables: &[(&str, PathBuf)], socket: &Path) -> Self {
        let mut command = Command::new(binary);
        command
            .env_clear()
            .env("RAZIEL_TRUST_DOMAIN_ID", TRUST_DOMAIN);
        for (name, value) in variables {
            command.env(name, value);
        }
        let mut guard = Self(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                guard.0.try_wait().unwrap().is_none(),
                "owner subprocess exited before socket readiness"
            );
            if fs::symlink_metadata(socket).is_ok_and(|m| {
                m.file_type().is_socket() && m.permissions().mode() & 0o7777 == 0o600
            }) {
                return guard;
            }
            assert!(
                Instant::now() < deadline,
                "owner subprocess socket did not become ready"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run(command: &mut Command) {
    let mut child = ChildGuard(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "pinned candidate preparation command failed"
            );
            return;
        }
        assert!(
            Instant::now() < deadline,
            "pinned candidate preparation command timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) struct SchemaHarness {
    task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    _runtime: tokio::runtime::Runtime,
    state: Arc<SchemaState>,
    endpoint: String,
}
impl SchemaHarness {
    fn new(compiler: PathBuf, root: PathBuf) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let state = Arc::new(SchemaState {
            schema: Mutex::new(Vec::new()),
            writes: AtomicUsize::new(0),
            lose_response: AtomicBool::new(false),
            unexpected: AtomicUsize::new(0),
            compiler,
            root,
        });
        let (endpoint, task) = {
            let _entered = runtime.enter();
            let incoming =
                tonic::transport::server::TcpIncoming::bind("127.0.0.1:0".parse().unwrap())
                    .unwrap();
            let endpoint = format!("http://{}", incoming.local_addr().unwrap());
            let service = SchemaService(state.clone());
            let task = runtime.spawn(
                tonic::transport::Server::builder()
                    .add_service(service)
                    .serve_with_incoming(incoming),
            );
            (endpoint, task)
        };
        Self {
            task,
            _runtime: runtime,
            state,
            endpoint,
        }
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.state.schema.lock().unwrap().clone()
    }
    pub fn replace(&self, schema: Vec<u8>) {
        *self.state.schema.lock().unwrap() = schema;
    }
    pub fn writes(&self) -> usize {
        self.state.writes.load(Ordering::SeqCst)
    }
    pub fn unexpected_calls(&self) -> usize {
        self.state.unexpected.load(Ordering::SeqCst)
    }
    pub fn lose_next_write_response(&self) {
        self.state.lose_response.store(true, Ordering::SeqCst);
    }
}
impl Drop for SchemaHarness {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct SchemaState {
    schema: Mutex<Vec<u8>>,
    writes: AtomicUsize,
    lose_response: AtomicBool,
    unexpected: AtomicUsize,
    compiler: PathBuf,
    root: PathBuf,
}

// spicedb-rs-proto publishes clients only. This test endpoint implements exactly
// the two schema RPCs and exposes no relationship or permission service.
#[derive(Clone)]
struct SchemaService(Arc<SchemaState>);
impl NamedService for SchemaService {
    const NAME: &'static str = "authzed.api.v1.SchemaService";
}
impl Service<http::Request<Body>> for SchemaService {
    type Response = http::Response<Body>;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let state = self.0.clone();
        match request.uri().path() {
            "/authzed.api.v1.SchemaService/ReadSchema" => Box::pin(async move {
                let mut grpc = tonic::server::Grpc::new(MessageCodec::<
                    api::ReadSchemaResponse,
                    api::ReadSchemaRequest,
                >::default());
                Ok(grpc.unary(ReadSchema(state), request).await)
            }),
            "/authzed.api.v1.SchemaService/WriteSchema" => Box::pin(async move {
                let mut grpc = tonic::server::Grpc::new(MessageCodec::<
                    api::WriteSchemaResponse,
                    api::WriteSchemaRequest,
                >::default());
                Ok(grpc.unary(WriteSchema(state), request).await)
            }),
            _ => {
                state.unexpected.fetch_add(1, Ordering::SeqCst);
                Box::pin(async {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(Body::empty())
                        .unwrap())
                })
            }
        }
    }
}

struct ReadSchema(Arc<SchemaState>);
impl UnaryService<api::ReadSchemaRequest> for ReadSchema {
    type Response = api::ReadSchemaResponse;
    type Future = BoxFuture<Response<Self::Response>, Status>;
    fn call(&mut self, request: Request<api::ReadSchemaRequest>) -> Self::Future {
        let state = self.0.clone();
        Box::pin(async move {
            authenticate(&request)?;
            let schema = state.schema.lock().unwrap().clone();
            if schema.is_empty() {
                return Err(Status::not_found("no schema"));
            }
            Ok(Response::new(api::ReadSchemaResponse {
                schema_text: String::from_utf8(schema).unwrap(),
                read_at: Some(api::ZedToken {
                    token: "test-read-revision".into(),
                }),
            }))
        })
    }
}
struct WriteSchema(Arc<SchemaState>);
impl UnaryService<api::WriteSchemaRequest> for WriteSchema {
    type Response = api::WriteSchemaResponse;
    type Future = BoxFuture<Response<Self::Response>, Status>;
    fn call(&mut self, request: Request<api::WriteSchemaRequest>) -> Self::Future {
        let state = self.0.clone();
        Box::pin(async move {
            authenticate(&request)?;
            let schema = request.into_inner().schema.into_bytes();
            let compiler = Compiler::new(&state.compiler, &state.root)
                .map_err(|_| Status::internal("compiler unavailable"))?;
            if !compiler
                .equivalent(&schema, &schema)
                .map_err(|_| Status::invalid_argument("schema rejected"))?
            {
                return Err(Status::invalid_argument("schema rejected"));
            }
            *state.schema.lock().unwrap() = schema;
            state.writes.fetch_add(1, Ordering::SeqCst);
            if state.lose_response.swap(false, Ordering::SeqCst) {
                return Err(Status::unavailable("injected loss after applied write"));
            }
            Ok(Response::new(api::WriteSchemaResponse {
                written_at: Some(api::ZedToken {
                    token: "test-written-revision".into(),
                }),
            }))
        })
    }
}
fn authenticate<T>(request: &Request<T>) -> Result<(), Status> {
    if request
        .metadata()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        != Some("Bearer publication-test-only")
    {
        return Err(Status::unauthenticated("test authentication required"));
    }
    Ok(())
}

#[derive(Default)]
struct MessageCodec<E, D>(PhantomData<(E, D)>);
impl<E: Message + Default + Send + 'static, D: Message + Default + Send + 'static> Codec
    for MessageCodec<E, D>
{
    type Encode = E;
    type Decode = D;
    type Encoder = MessageEncoder<E>;
    type Decoder = MessageDecoder<D>;
    fn encoder(&mut self) -> Self::Encoder {
        MessageEncoder(PhantomData)
    }
    fn decoder(&mut self) -> Self::Decoder {
        MessageDecoder(PhantomData)
    }
}
struct MessageEncoder<T>(PhantomData<T>);
impl<T: Message> Encoder for MessageEncoder<T> {
    type Item = T;
    type Error = Status;
    fn encode(&mut self, item: T, output: &mut EncodeBuf<'_>) -> Result<(), Status> {
        item.encode(output)
            .map_err(|_| Status::internal("test response encoding failed"))
    }
}
struct MessageDecoder<T>(PhantomData<T>);
impl<T: Message + Default> Decoder for MessageDecoder<T> {
    type Item = T;
    type Error = Status;
    fn decode(&mut self, input: &mut DecodeBuf<'_>) -> Result<Option<T>, Status> {
        T::decode(input)
            .map(Some)
            .map_err(|_| Status::invalid_argument("test request decoding failed"))
    }
}
