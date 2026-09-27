use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use prost::Message;
use raziel_authority::PolicyApprovalRole;
use raziel_canonical::{
    PolicyApprovalClaimsV1, PolicyApprovalLimits, ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::decode_policy_program_strict;
use raziel_verifier::{PolicyRecordStatus, PolicyRegistryRecordV1, policy_record_to_proto};
use spicedb_rs_proto::authzed::api::v1 as authzed;

use super::*;
use request_receipt::RequestReceipt;

const DOMAIN: &str = "request-snapshot-test";
const POLICY_BYTES: &[u8] = include_bytes!(
    "../../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb"
);
const SCHEMA: &str =
    include_str!("../../../../tools/spicedb-ir-compiler/internal/adapter/testdata/supported.zed");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rz-snap-{}", lowercase_hex(&nonce)));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn source() -> RequestSnapshotSource {
    RequestSnapshotSource {
        receipt_key: "agent:request".into(),
        proposal_generation: 1,
        policy_record_id: "policy-1".into(),
        zed_token: "exact".into(),
        source_schema_digest: [1; 32],
        accepted_composition_schema: SCHEMA.into(),
    }
}
fn build() -> proto::BuildSnapshotRequest {
    proto::BuildSnapshotRequest {
        spicedb_endpoint_ref: "request-source".into(),
        zed_token_secret_ref: "agent:request:1".into(),
        policy_record_id: "policy-1".into(),
        policy_hash: vec![1; 32],
        expected_previous_epoch: 0,
        authority_key_id: "test-key".into(),
        expires_at: Some(timestamp(2_000_000_000)),
    }
}

#[test]
fn receipt_is_immutable_private_and_recovers_only_the_original_epoch() {
    let root = Directory::new();
    let path = root.0.join("receipts");
    let source = source();
    let request = build();
    let descriptor = proto::RelationshipSnapshotDescriptor {
        snapshot_id: "snapshot-v1:test".into(),
        epoch: 1,
        ..Default::default()
    };
    let receipt = RequestReceipt::open(&path, &request, &source).unwrap();
    assert!(receipt.read().unwrap().is_none());
    receipt.publish(&descriptor).unwrap();
    assert_eq!(receipt.read().unwrap().unwrap().descriptor, descriptor);
    assert_eq!(
        fs::metadata(path.join("agent:request-1.receipt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(RequestReceipt::open(&path, &request, &source).is_err());
    drop(receipt);
    let mut observed = request.clone();
    observed.expected_previous_epoch = 1;
    let retry = RequestReceipt::open(&path, &observed, &source).unwrap();
    let retained = retry.read().unwrap().unwrap();
    assert_eq!(retained.request, request);
    assert_eq!(retained.descriptor, descriptor);
    drop(retry);
    let mut changed = request.clone();
    changed.expires_at.as_mut().unwrap().seconds += 1;
    let changed = RequestReceipt::open(&path, &changed, &source).unwrap();
    assert!(matches!(
        changed.read(),
        Err(SnapshotOperationError::WriterConflict)
    ));
    drop(changed);
    for field in 0..3 {
        let mut changed = source.clone();
        match field {
            0 => changed.zed_token.push('x'),
            1 => changed.source_schema_digest[0] ^= 1,
            _ => changed.accepted_composition_schema.push(' '),
        }
        assert!(matches!(
            RequestReceipt::open(&path, &request, &changed)
                .unwrap()
                .read(),
            Err(SnapshotOperationError::WriterConflict)
        ));
    }
    fs::set_permissions(
        path.join("agent:request-1.receipt"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(matches!(
        RequestReceipt::open(&path, &request, &source)
            .unwrap()
            .read(),
        Err(SnapshotOperationError::PrivateStore)
    ));
}

fn config_values(root: &Path, endpoint: &str) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("RAZIEL_SPICEDB_ENDPOINT_REF", "request-source".into()),
        ("RAZIEL_SPICEDB_ENDPOINT", endpoint.into()),
        (
            "RAZIEL_SPICEDB_AUTH_TOKEN_PATH",
            root.join("token").display().to_string(),
        ),
        (
            "RAZIEL_ZED_TOKEN_REQUEST_ROOT",
            root.join("receipts").display().to_string(),
        ),
        (
            "RAZIEL_PRIVATE_SNAPSHOT_ROOT",
            root.join("private").display().to_string(),
        ),
        (
            "RAZIEL_REGISTRY_WRITER_SOCKET_PATH",
            root.join("writer.sock").display().to_string(),
        ),
        (
            "RAZIEL_REGISTRY_PATH",
            root.join("registry").display().to_string(),
        ),
        (
            "RAZIEL_SNAPSHOT_KEY_PATH",
            root.join("snapshot.pk8").display().to_string(),
        ),
        ("RAZIEL_TRUST_DOMAIN_ID", DOMAIN.into()),
    ])
}

#[test]
fn request_config_is_explicit_exclusive_and_rejects_the_legacy_rpc() {
    let root = Directory::new();
    let mut values = config_values(&root.0, "http://127.0.0.1:50051");
    let config = SnapshotServiceConfig::from_lookup(|key| values.get(key).cloned()).unwrap();
    assert_eq!(
        config.request_snapshot_root(),
        Some(root.0.join("receipts").as_path())
    );
    assert!(matches!(
        ValidatedBuildRequest::new(&config, build(), 1_700_000_000),
        Err(SnapshotOperationError::ConfigurationReferenceMismatch)
    ));
    values.insert(
        "RAZIEL_ZED_TOKEN_CANDIDATE_ROOT",
        root.0.join("candidate").display().to_string(),
    );
    assert!(SnapshotServiceConfig::from_lookup(|key| values.get(key).cloned()).is_err());
    values.remove("RAZIEL_ZED_TOKEN_REQUEST_ROOT");
    let legacy = SnapshotServiceConfig::from_lookup(|key| values.get(key).cloned()).unwrap();
    assert!(legacy.request_snapshot_root().is_none());
    assert!(legacy.accepts_zed_token_reference("candidate-1"));
}

struct Writer(Child);
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Owns its registry and private files. Use a disposable, empty SpiceDB instance
/// and a non-root UID so the authority key is not effectively writable.
#[tokio::test]
#[ignore = "requires non-root UID, empty dedicated SpiceDB and RAZIEL_SNAPSHOT_TEST_BIN_DIR"]
async fn live_request_snapshot_recovers_after_private_receipt_before_registry_append() {
    let endpoint = std::env::var("RAZIEL_SNAPSHOT_TEST_SPICEDB_ENDPOINT").unwrap();
    let token = std::env::var("RAZIEL_SNAPSHOT_TEST_SPICEDB_TOKEN").unwrap();
    let bins = PathBuf::from(std::env::var("RAZIEL_SNAPSHOT_TEST_BIN_DIR").unwrap());
    let root = Directory::new();
    let now = SystemClock.now_unix_seconds().unwrap();
    let policy = decode_policy_program_strict(
        POLICY_BYTES,
        POLICY_BYTES.len() as u64,
        &TRUSTED_POLICY_CEILINGS,
    )
    .unwrap();
    let policy_signer =
        AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&key_bytes(31)).unwrap();
    let signer = AuthoritySigner::<SnapshotRole>::from_pkcs8_der(&key_bytes(32)).unwrap();
    let hash = policy_hash(&policy).unwrap();
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: 1,
            trust_domain_id: DOMAIN.into(),
            record_id: "policy-1".into(),
            policy_name: "request-policy".into(),
            policy_version: "1".into(),
            approval_epoch: 1,
            policy_hash: hash,
            valid_from_unix_seconds: now - 60,
            valid_until_unix_seconds: now + 7200,
            approved_by: "operator".into(),
            signing_key_id: policy_signer.key_id().into(),
        },
        PolicyApprovalLimits {
            max_string_bytes: 512,
            max_lifetime_seconds: 86_400,
        },
    )
    .unwrap();
    let record = PolicyRegistryRecordV1 {
        authority_signature: policy_signer.sign_policy_approval(&claims).unwrap(),
        claims,
        policy: policy.clone(),
        canonical_policy_ir_bytes: POLICY_BYTES.to_vec(),
        status: PolicyRecordStatus::Approved,
        registered_at_unix_seconds: now,
        revoked_at_unix_seconds: None,
        revocation_reason: None,
        registry_generation: 1,
    };
    let registry_root = root.0.join("registry");
    DirBuilder::new()
        .mode(0o700)
        .create(&registry_root)
        .unwrap();
    write_private(
        &registry_root.join("policy.pb"),
        &policy_record_to_proto(&record).unwrap().encode_to_vec(),
    );
    write_private(
        &registry_root.join("policy-key.pb"),
        &authority_key(
            policy_signer.key_id(),
            policy_signer.public_key_bytes(),
            proto::AuthorityRole::PolicyApproval,
            now,
        )
        .encode_to_vec(),
    );
    write_private(
        &registry_root.join("snapshot-key.pb"),
        &authority_key(
            signer.key_id(),
            signer.public_key_bytes(),
            proto::AuthorityRole::Snapshot,
            now,
        )
        .encode_to_vec(),
    );
    write_private(&root.0.join("snapshot.pk8"), &key_bytes(32));
    fs::set_permissions(
        root.0.join("snapshot.pk8"),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    write_private(&root.0.join("token"), token.as_bytes());
    let status = Command::new(bins.join("raziel-admin"))
        .arg("publish-registry")
        .arg("--root")
        .arg(&registry_root)
        .args([
            "--trust-domain",
            DOMAIN,
            "--generation",
            "1",
            "--published-at",
            &now.to_string(),
            "--observed-now",
            &now.to_string(),
            "--output-directory",
            "generation-1",
        ])
        .arg("--output-manifest")
        .arg(registry_root.join("current.pb"))
        .arg("--output-json")
        .arg(root.0.join("registry.json"))
        .arg("--authority-key")
        .arg("policy-key.pb")
        .arg("--authority-key")
        .arg("snapshot-key.pb")
        .arg("--policy")
        .arg("policy.pb")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "initial registry publication failed");
    let mut schema = authzed::schema_service_client::SchemaServiceClient::connect(endpoint.clone())
        .await
        .unwrap();
    let composition = format!("{SCHEMA}\ndefinition other_package {{}}\n");
    let mut write = tonic::Request::new(authzed::WriteSchemaRequest {
        schema: composition.clone(),
    });
    write
        .metadata_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let revision = schema
        .write_schema(write)
        .await
        .unwrap()
        .into_inner()
        .written_at
        .unwrap()
        .token;
    let source = RequestSnapshotSource {
        zed_token: revision,
        source_schema_digest: policy.program().source_schema_digest,
        accepted_composition_schema: composition,
        ..source()
    };
    let request = proto::BuildSnapshotRequest {
        policy_hash: hash.to_vec(),
        authority_key_id: signer.key_id().into(),
        expires_at: Some(timestamp(now + 6000)),
        ..build()
    };
    let values = config_values(&root.0, &endpoint);
    let config = SnapshotServiceConfig::from_lookup(|key| values.get(key).cloned()).unwrap();
    let coordinator = SnapshotCoordinator::open(config).unwrap();
    // Simulate a writer outage after the signed descriptor and private snapshot
    // have been synced. The retry must not rebuild or need live SpiceDB.
    assert_eq!(
        coordinator
            .build_from_request(request.clone(), source.clone())
            .await,
        Err(SnapshotOperationError::WriterUnavailable)
    );
    let before = RequestReceipt::open(&root.0.join("receipts"), &request, &source)
        .unwrap()
        .read()
        .unwrap()
        .unwrap()
        .descriptor;
    assert_eq!(fs::read_dir(root.0.join("private")).unwrap().count(), 1);
    let mut writer = Writer(
        Command::new(bins.join("raziel-registry-writer"))
            .env("RAZIEL_REGISTRY_PATH", &registry_root)
            .env(
                "RAZIEL_REGISTRY_WRITER_SOCKET_PATH",
                root.0.join("writer.sock"),
            )
            .env("RAZIEL_TRUST_DOMAIN_ID", DOMAIN)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            writer.0.try_wait().unwrap().is_none(),
            "registry writer exited"
        );
        if fs::symlink_metadata(root.0.join("writer.sock")).is_ok_and(|m| m.file_type().is_socket())
        {
            break;
        }
        assert!(Instant::now() < deadline, "writer socket unavailable");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let offline = config_values(&root.0, "http://127.0.0.1:1");
    let config = SnapshotServiceConfig::from_lookup(|key| offline.get(key).cloned()).unwrap();
    let restarted = SnapshotCoordinator::open(config).unwrap();
    let recovered = restarted
        .build_from_request(request.clone(), source.clone())
        .await
        .unwrap()
        .snapshot
        .unwrap();
    assert_eq!(recovered, before);
    let mut new_observation = request.clone();
    new_observation.expected_previous_epoch = recovered.epoch;
    assert_eq!(
        restarted
            .build_from_request(new_observation, source.clone())
            .await
            .unwrap()
            .snapshot
            .unwrap(),
        before
    );
    assert_eq!(fs::read_dir(root.0.join("private")).unwrap().count(), 1);
    let current = load_registry(restarted.config(), SystemClock.now_unix_seconds().unwrap())
        .await
        .unwrap();
    assert_eq!(current.accepted_roots.len(), 1);
    assert_eq!(
        current.accepted_roots[0]
            .claims
            .claims()
            .relationship_snapshot_root
            .as_slice(),
        recovered.relationship_snapshot_root
    );
    let mut changed = source.clone();
    changed.source_schema_digest[0] ^= 1;
    assert_eq!(
        restarted.build_from_request(request.clone(), changed).await,
        Err(SnapshotOperationError::SpiceDb)
    );
    let mut changed = request.clone();
    changed.expires_at.as_mut().unwrap().seconds -= 1;
    assert_eq!(
        restarted.build_from_request(changed, source.clone()).await,
        Err(SnapshotOperationError::WriterConflict)
    );
    // Missing witness storage must fail closed, even for an accepted descriptor.
    for entry in fs::read_dir(root.0.join("private")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    assert_eq!(
        restarted.build_from_request(request, source).await,
        Err(SnapshotOperationError::PrivateStore)
    );
}

fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn key_bytes(seed: u8) -> Vec<u8> {
    [
        vec![
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20,
        ],
        vec![seed; 32],
    ]
    .concat()
}
fn authority_key(
    id: &str,
    public: [u8; 32],
    role: proto::AuthorityRole,
    now: i64,
) -> proto::AuthorityKeyRecord {
    proto::AuthorityKeyRecord {
        format_version: 1,
        trust_domain_id: DOMAIN.into(),
        key_id: id.into(),
        role: role as i32,
        public_key: public.to_vec(),
        status: proto::AuthorityKeyStatus::Active as i32,
        valid_from: Some(timestamp(now - 60)),
        valid_until: Some(timestamp(now + 86400)),
        revoked_at: None,
        revocation_reason: String::new(),
        registry_generation: 1,
    }
}
