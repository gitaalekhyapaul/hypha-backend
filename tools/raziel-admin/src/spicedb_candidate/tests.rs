use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use prost::Message;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    AuthoritySigner, PolicyApprovalRole, TrustedAuthorityKey,
};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::{
    AllowedSubjectType, CaveatDefinition, CaveatExpr, CaveatInputSource, CaveatParameter,
    CaveatValueType, ObjectDefinition, PermissionDefinition, PermissionExpr, PolicyLimits,
    PolicyProgram, RelationDefinition, ValidatedPolicyProgram, ValidationCeilings,
    decode_policy_program_strict,
};
use raziel_snapshot::{RelationshipRecord, StoredCaveat, StoredContextEntry};
use raziel_verifier::{
    PolicyRecordStatus, PolicyRegistryRecordV1, authority_key_to_proto, policy_record_to_proto,
};
use spicedb_rs_proto::authzed::api::v1 as authzed;

use super::store::{load_existing_handoff, prepare_candidate, validate_relationship_batch};
use super::wire::{RelationshipBatchInput, SpiceDbRevisionHandoffWire};
use super::{
    CandidateConfig, CandidateRevisionHandoffReader, CandidateRevisionReaderError,
    CandidateWriteError, PreparedCandidate, SpiceDbWriteApi, parse_candidate_id,
    relationship_update, write_prepared_candidate,
};
use crate::args::Arguments;
use crate::config::validation_ceilings;
use crate::policy_authority::PolicyCandidateManifestWire;
use crate::registry::publish;

const SCHEMA_REVISION: &str = "schema-private-revision";
const FINAL_REVISION: &str = "relationship-private-revision";
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);
const TEST_NOW: i64 = 1_700_000_000;
const TEST_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];
const SUPPORTED_POLICY_BYTES: &[u8] =
    include_bytes!("../../../spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb");
const SUPPORTED_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spicedb-ir-compiler/internal/adapter/testdata/supported.zed");

#[tokio::test]
async fn writes_schema_then_relationships_and_reuses_private_handoff() {
    let fixture = Fixture::new();
    let prepared = fixture.prepared(vec![direct_relationship()]);
    let mut writer = FakeWriter::successful();

    let result = write_prepared_candidate(&mut writer, &prepared)
        .await
        .unwrap();

    assert_eq!(writer.calls, vec!["schema", "relationships"]);
    assert_eq!(writer.schema_requests[0].schema, "definition agent {}\n");
    assert_eq!(writer.relationship_requests[0].updates.len(), 1);
    assert_eq!(
        writer.relationship_requests[0].updates[0].operation,
        authzed::relationship_update::Operation::Touch as i32
    );
    assert_eq!(result.candidate_id, "candidate-1");
    assert_eq!(result.accepted_relationship_count, 1);

    let metadata = std::fs::metadata(&prepared.handoff_path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    let bytes = std::fs::read(&prepared.handoff_path).unwrap();
    let handoff = SpiceDbRevisionHandoffWire::decode(bytes.as_slice()).unwrap();
    assert_eq!(handoff.zed_token, FINAL_REVISION);
    assert!(!format!("{prepared:?}").contains(FINAL_REVISION));
    let reader = CandidateRevisionHandoffReader::new(&fixture.root).unwrap();
    let revision = reader
        .read_bound("candidate-1", "policy-1", &[0x11; 32])
        .unwrap();
    assert_eq!(revision.zed_token(), FINAL_REVISION);
    assert!(!format!("{reader:?}{revision:?}").contains(FINAL_REVISION));
    assert!(matches!(
        reader.read_bound("candidate-1", "policy-other", &[0x11; 32]),
        Err(CandidateRevisionReaderError::Rejected)
    ));

    let mut retry_writer = FakeWriter::successful();
    let retry = write_prepared_candidate(&mut retry_writer, &prepared)
        .await
        .unwrap();
    assert_eq!(retry, result);
    assert!(retry_writer.calls.is_empty());
    assert!(load_existing_handoff(&prepared).unwrap().is_some());
}

#[tokio::test]
async fn partial_failures_never_create_a_false_handoff() {
    let fixture = Fixture::new();
    let prepared = fixture.prepared(vec![direct_relationship()]);
    let mut schema_failure = FakeWriter::failing_schema();
    assert_eq!(
        write_prepared_candidate(&mut schema_failure, &prepared).await,
        Err(CandidateWriteError::SchemaWriteFailed)
    );
    assert_eq!(schema_failure.calls, vec!["schema"]);
    assert!(!prepared.handoff_path.exists());

    let mut relationship_failure = FakeWriter::failing_relationships();
    assert_eq!(
        write_prepared_candidate(&mut relationship_failure, &prepared).await,
        Err(CandidateWriteError::RelationshipWriteFailed)
    );
    assert_eq!(relationship_failure.calls, vec!["schema", "relationships"]);
    assert!(!prepared.handoff_path.exists());

    let mut missing_schema_revision = FakeWriter::successful();
    missing_schema_revision.schema_revision = None;
    assert_eq!(
        write_prepared_candidate(&mut missing_schema_revision, &prepared).await,
        Err(CandidateWriteError::MissingRevision)
    );
    assert_eq!(missing_schema_revision.calls, vec!["schema"]);
    assert!(!prepared.handoff_path.exists());
}

#[test]
fn fixed_candidate_files_must_match_the_registered_policy_source() {
    let fixture = PublishedCandidateFixture::new();
    let prepared = prepare_candidate(&fixture.config, "candidate-registered", TEST_NOW).unwrap();
    assert_eq!(prepared.policy_record_id, "policy-registered");
    assert_eq!(prepared.resource_type, "payment_batch");
    assert_eq!(prepared.accepted_relationship_count, 1);

    write_private(
        &fixture.candidate_dir.join("schema.zed"),
        b"definition user {}\n",
        false,
    );
    assert!(matches!(
        prepare_candidate(&fixture.config, "candidate-registered", TEST_NOW),
        Err(CandidateWriteError::CandidateRejected)
    ));
}

#[test]
fn relationship_file_is_strict_typed_bounded_and_policy_checked() {
    let policy = policy();
    let accepted: RelationshipBatchInput = serde_json::from_str(
        r#"{
          "format_version": 1,
          "candidate_id": "candidate-1",
          "relationships": [{
            "resource_type": "dataset",
            "resource_id": "payments",
            "relation": "viewer",
            "subject_type": "agent",
            "subject_id": "agent-1"
          }]
        }"#,
    )
    .unwrap();
    assert_eq!(
        validate_relationship_batch(accepted, "candidate-1", &policy)
            .unwrap()
            .len(),
        1
    );

    let unknown_field = r#"{
      "format_version": 1,
      "candidate_id": "candidate-1",
      "relationships": [],
      "policy_ir": "forbidden"
    }"#;
    assert!(serde_json::from_str::<RelationshipBatchInput>(unknown_field).is_err());

    let wrong_subject: RelationshipBatchInput = serde_json::from_str(
        r#"{
          "format_version": 1,
          "candidate_id": "candidate-1",
          "relationships": [{
            "resource_type": "dataset",
            "resource_id": "payments",
            "relation": "viewer",
            "subject_type": "team",
            "subject_id": "finance"
          }]
        }"#,
    )
    .unwrap();
    assert_eq!(
        validate_relationship_batch(wrong_subject, "candidate-1", &policy),
        Err(CandidateWriteError::CandidateRejected)
    );

    let duplicate = RelationshipBatchInput {
        format_version: 1,
        candidate_id: "candidate-1".into(),
        relationships: vec![relationship_input("agent-1"), relationship_input("agent-1")],
    };
    assert_eq!(
        validate_relationship_batch(duplicate, "candidate-1", &policy),
        Err(CandidateWriteError::CandidateRejected)
    );
}

#[test]
fn caveat_conversion_preserves_scalars_and_rejects_lossy_integers() {
    let relationship = RelationshipRecord {
        stored_caveat: Some(StoredCaveat {
            name: "bounded".into(),
            context: vec![
                StoredContextEntry {
                    name: "approved".into(),
                    value: raziel_policy_ir::CaveatValue::Bool(true),
                },
                StoredContextEntry {
                    name: "limit".into(),
                    value: raziel_policy_ir::CaveatValue::Int(10_000),
                },
                StoredContextEntry {
                    name: "purpose".into(),
                    value: raziel_policy_ir::CaveatValue::String("reconciliation".into()),
                },
            ],
        }),
        ..direct_relationship()
    };
    let converted = relationship_update(&relationship).unwrap();
    let context = converted
        .relationship
        .unwrap()
        .optional_caveat
        .unwrap()
        .context
        .unwrap();
    assert_eq!(context.fields.len(), 3);

    let lossy = RelationshipRecord {
        stored_caveat: Some(StoredCaveat {
            name: "bounded".into(),
            context: vec![StoredContextEntry {
                name: "limit".into(),
                value: raziel_policy_ir::CaveatValue::Int(i64::MIN),
            }],
        }),
        ..direct_relationship()
    };
    assert_eq!(
        relationship_update(&lossy),
        Err(CandidateWriteError::CandidateRejected)
    );
}

#[test]
fn command_accepts_only_one_candidate_identifier() {
    assert_eq!(
        parse_candidate_id(
            ["binary", "--candidate-id", "candidate-1"]
                .into_iter()
                .map(str::to_owned)
        )
        .unwrap(),
        "candidate-1"
    );
    assert_eq!(
        parse_candidate_id(
            ["binary", "--candidate-id", "candidate-1", "--path"]
                .into_iter()
                .map(str::to_owned)
        ),
        Err(CandidateWriteError::Usage)
    );
}

struct FakeWriter {
    calls: Vec<&'static str>,
    schema_requests: Vec<authzed::WriteSchemaRequest>,
    relationship_requests: Vec<authzed::WriteRelationshipsRequest>,
    schema_revision: Option<&'static str>,
    relationship_revision: Option<&'static str>,
    schema_error: Option<CandidateWriteError>,
    relationship_error: Option<CandidateWriteError>,
}

impl FakeWriter {
    fn successful() -> Self {
        Self {
            calls: Vec::new(),
            schema_requests: Vec::new(),
            relationship_requests: Vec::new(),
            schema_revision: Some(SCHEMA_REVISION),
            relationship_revision: Some(FINAL_REVISION),
            schema_error: None,
            relationship_error: None,
        }
    }

    fn failing_schema() -> Self {
        Self {
            schema_error: Some(CandidateWriteError::SchemaWriteFailed),
            ..Self::successful()
        }
    }

    fn failing_relationships() -> Self {
        Self {
            relationship_error: Some(CandidateWriteError::RelationshipWriteFailed),
            ..Self::successful()
        }
    }
}

#[tonic::async_trait]
impl SpiceDbWriteApi for FakeWriter {
    async fn write_schema(
        &mut self,
        request: authzed::WriteSchemaRequest,
    ) -> Result<authzed::WriteSchemaResponse, CandidateWriteError> {
        self.calls.push("schema");
        self.schema_requests.push(request);
        if let Some(error) = self.schema_error {
            return Err(error);
        }
        Ok(authzed::WriteSchemaResponse {
            written_at: self.schema_revision.map(token),
        })
    }

    async fn write_relationships(
        &mut self,
        request: authzed::WriteRelationshipsRequest,
    ) -> Result<authzed::WriteRelationshipsResponse, CandidateWriteError> {
        self.calls.push("relationships");
        self.relationship_requests.push(request);
        if let Some(error) = self.relationship_error {
            return Err(error);
        }
        Ok(authzed::WriteRelationshipsResponse {
            written_at: self.relationship_revision.map(token),
        })
    }
}

fn token(value: &str) -> authzed::ZedToken {
    authzed::ZedToken {
        token: value.into(),
    }
}

struct Fixture {
    root: PathBuf,
    candidate_dir: PathBuf,
}

struct PublishedCandidateFixture {
    root: PathBuf,
    candidate_dir: PathBuf,
    config: CandidateConfig,
}

impl PublishedCandidateFixture {
    fn new() -> Self {
        let root = private_temp_root("published");
        let registry_root = root.join("registry");
        std::fs::create_dir(&registry_root).unwrap();
        std::fs::create_dir(registry_root.join("drafts")).unwrap();

        let signer =
            AuthoritySigner::<PolicyApprovalRole>::from_pkcs8_der(&test_pkcs8(0x41)).unwrap();
        let trusted_key = TrustedAuthorityKey::from_record(
            AuthorityKeyRecordV1 {
                format_version: AUTHORITY_KEY_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                key_id: signer.key_id().into(),
                role: AuthorityRole::PolicyApproval,
                public_key: signer.public_key_bytes(),
                status: AuthorityKeyStatus::Active,
                valid_from_unix_seconds: TEST_NOW - 1_000,
                valid_until_unix_seconds: TEST_NOW + 10_000,
                revoked_at_unix_seconds: None,
                revocation_reason: None,
                registry_generation: 1,
            },
            "raziel-test",
        )
        .unwrap();
        let policy = decode_policy_program_strict(
            SUPPORTED_POLICY_BYTES,
            SUPPORTED_POLICY_BYTES.len() as u64,
            &validation_ceilings(),
        )
        .unwrap();
        let policy_hash = policy_hash(&policy).unwrap();
        let claims = ValidatedPolicyApprovalClaimsV1::new(
            PolicyApprovalClaimsV1 {
                format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
                trust_domain_id: "raziel-test".into(),
                record_id: "policy-registered".into(),
                policy_name: "reconciliation".into(),
                policy_version: "1".into(),
                approval_epoch: 1,
                policy_hash,
                valid_from_unix_seconds: TEST_NOW - 100,
                valid_until_unix_seconds: TEST_NOW + 10_000,
                approved_by: "operator".into(),
                signing_key_id: signer.key_id().into(),
            },
            PolicyApprovalLimits {
                max_string_bytes: 512,
                max_lifetime_seconds: 31 * 24 * 60 * 60,
            },
        )
        .unwrap();
        let policy_record = PolicyRegistryRecordV1 {
            authority_signature: signer.sign_policy_approval(&claims).unwrap(),
            claims,
            policy: policy.clone(),
            canonical_policy_ir_bytes: SUPPORTED_POLICY_BYTES.to_vec(),
            status: PolicyRecordStatus::Approved,
            registered_at_unix_seconds: TEST_NOW,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation: 1,
        };
        write_private(
            &registry_root.join("drafts/policy-key.pb"),
            &authority_key_to_proto(&trusted_key).encode_to_vec(),
            true,
        );
        write_private(
            &registry_root.join("drafts/policy.pb"),
            &policy_record_to_proto(&policy_record)
                .unwrap()
                .encode_to_vec(),
            true,
        );
        publish(&arguments([
            "publish-registry",
            "--root",
            registry_root.to_str().unwrap(),
            "--trust-domain",
            "raziel-test",
            "--generation",
            "1",
            "--published-at",
            "1700000000",
            "--observed-now",
            "1700000000",
            "--output-directory",
            "generations/000001",
            "--output-manifest",
            registry_root.join("current.pb").to_str().unwrap(),
            "--output-json",
            registry_root.join("current.json").to_str().unwrap(),
            "--authority-key",
            "drafts/policy-key.pb",
            "--policy",
            "drafts/policy.pb",
        ]))
        .unwrap();

        let candidate_root = root.join("candidates");
        create_private_dir(&candidate_root);
        let candidate_dir = candidate_root.join("candidate-registered");
        create_private_dir(&candidate_dir);
        let manifest = PolicyCandidateManifestWire {
            format_version: 1,
            candidate_id: "candidate-registered".into(),
            record_id: "policy-registered".into(),
            policy_name: "reconciliation".into(),
            policy_version: "1".into(),
            resource_type: "payment_batch".into(),
            permission: "view".into(),
            rule_summary: "direct viewer".into(),
            source_schema_digest: policy.program().source_schema_digest.to_vec(),
            policy_hash: policy_hash.to_vec(),
            expected_approval_epoch: 1,
            valid_from_unix_seconds: TEST_NOW - 100,
            valid_until_unix_seconds: TEST_NOW + 10_000,
            created_at_unix_seconds: TEST_NOW - 10,
            expires_at_unix_seconds: TEST_NOW + 1_000,
        };
        write_private(
            &candidate_dir.join("candidate.pb"),
            &manifest.encode_to_vec(),
            true,
        );
        write_private(
            &candidate_dir.join("schema.zed"),
            SUPPORTED_SCHEMA_BYTES,
            true,
        );
        write_private(
            &candidate_dir.join("relationships.json"),
            br#"{
              "format_version": 1,
              "candidate_id": "candidate-registered",
              "relationships": [{
                "resource_type": "payment_batch",
                "resource_id": "payments",
                "relation": "viewer",
                "subject_type": "user",
                "subject_id": "maya"
              }]
            }"#,
            true,
        );
        let token_path = root.join("spicedb-token");
        write_private(&token_path, b"private-token", true);
        Self {
            candidate_dir,
            config: CandidateConfig {
                candidate_root,
                registry_root,
                token_path,
                trust_domain_id: "raziel-test".into(),
                endpoint: "http://spicedb:50051".into(),
            },
            root,
        }
    }
}

impl Drop for PublishedCandidateFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

impl Fixture {
    fn new() -> Self {
        let unique = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "raziel-spicedb-candidate-{}-{unique}",
            std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700).create(&root).unwrap();
        let candidate_dir = root.join("candidate-1");
        create_private_dir(&candidate_dir);
        Self {
            root,
            candidate_dir,
        }
    }

    fn prepared(&self, relationships: Vec<RelationshipRecord>) -> PreparedCandidate {
        PreparedCandidate {
            candidate_id: "candidate-1".into(),
            policy_record_id: "policy-1".into(),
            policy_hash: [0x11; 32],
            resource_type: "dataset".into(),
            schema: "definition agent {}\n".into(),
            source_schema_digest: [0x22; 32],
            relationship_batch_digest: [0x33; 32],
            accepted_relationship_count: relationships.len() as u32,
            relationships,
            handoff_path: self.candidate_dir.join("spicedb-revision.pb"),
            candidate_owner: std::fs::metadata(&self.candidate_dir).unwrap().uid(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

fn direct_relationship() -> RelationshipRecord {
    RelationshipRecord {
        resource_type: "dataset".into(),
        resource_id: "payments".into(),
        relation: "viewer".into(),
        subject_type: "agent".into(),
        subject_id: "agent-1".into(),
        subject_relation: None,
        stored_caveat: None,
    }
}

fn relationship_input(subject_id: &str) -> super::wire::RelationshipInput {
    super::wire::RelationshipInput {
        resource_type: "dataset".into(),
        resource_id: "payments".into(),
        relation: "viewer".into(),
        subject_type: "agent".into(),
        subject_id: subject_id.into(),
        subject_relation: None,
        caveat: None,
    }
}

fn arguments<const N: usize>(values: [&str; N]) -> Arguments {
    Arguments::parse(values.into_iter().map(str::to_owned)).unwrap()
}

fn test_pkcs8(seed_byte: u8) -> Vec<u8> {
    [TEST_PKCS8_PREFIX.as_slice(), &[seed_byte; 32]].concat()
}

fn private_temp_root(label: &str) -> PathBuf {
    let unique = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "raziel-spicedb-{label}-{}-{unique}",
        std::process::id()
    ));
    create_private_dir(&root);
    root
}

fn create_private_dir(path: &std::path::Path) {
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700).create(path).unwrap();
}

fn write_private(path: &std::path::Path, bytes: &[u8], create_new: bool) {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).mode(0o600);
    if create_new {
        options.create_new(true);
    } else {
        options.truncate(true);
    }
    let mut file = options.open(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn policy() -> ValidatedPolicyProgram {
    let limits = PolicyLimits {
        max_expression_nodes: 16,
        max_caveat_nodes: 16,
        max_path_depth: 4,
        max_relationship_witnesses: 8,
        max_merkle_depth: 16,
        max_string_bytes: 256,
        max_context_entries: 8,
        max_total_private_input_bytes: 4096,
    };
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "test".into(),
            compiler_version: "1".into(),
            source_schema_digest: [0x22; 32],
            definitions: vec![
                ObjectDefinition {
                    name: "agent".into(),
                    relations: Vec::new(),
                },
                ObjectDefinition {
                    name: "dataset".into(),
                    relations: vec![RelationDefinition {
                        name: "viewer".into(),
                        allowed_subject_types: vec![AllowedSubjectType {
                            object_type: "agent".into(),
                            subject_relation: None,
                            caveat_name: None,
                        }],
                    }],
                },
            ],
            permissions: vec![PermissionDefinition {
                object_type: "dataset".into(),
                name: "query".into(),
                expression: PermissionExpr::RelationRef {
                    name: "viewer".into(),
                },
            }],
            caveats: vec![CaveatDefinition {
                name: "bounded".into(),
                parameters: vec![CaveatParameter {
                    source: CaveatInputSource::RelationshipContext,
                    name: "limit".into(),
                    value_type: CaveatValueType::Int,
                }],
                expression: CaveatExpr::ConstBool(true),
            }],
            limits,
        },
        &ValidationCeilings {
            policy_limits: limits,
            max_definitions: 8,
            max_relations_per_definition: 8,
            max_allowed_subject_types_per_relation: 8,
            max_permissions: 8,
            max_caveats: 8,
            max_parameters_per_caveat: 8,
            max_syntax_depth: 8,
        },
    )
    .unwrap()
}
