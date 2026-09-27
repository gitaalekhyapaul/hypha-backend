use super::super::network::SchemaApi;
use super::*;
use network::Observed;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn relationship(
    resource_type: &str,
    id: &str,
    relation: &str,
    subject_type: &str,
    subject_id: &str,
) -> api::Relationship {
    api::Relationship {
        resource: Some(api::ObjectReference {
            object_type: resource_type.into(),
            object_id: id.into(),
        }),
        relation: relation.into(),
        subject: Some(api::SubjectReference {
            object: Some(api::ObjectReference {
                object_type: subject_type.into(),
                object_id: subject_id.into(),
            }),
            optional_relation: String::new(),
        }),
        optional_caveat: None,
        optional_expires_at: None,
    }
}
fn update(r: api::Relationship) -> api::RelationshipUpdate {
    api::RelationshipUpdate {
        operation: api::relationship_update::Operation::Touch as i32,
        relationship: Some(r),
    }
}
struct Fake {
    rows: Vec<api::Relationship>,
    reads: usize,
    writes: usize,
    lose_response: bool,
    read_fail: bool,
}
impl Fake {
    fn new() -> Self {
        Self {
            rows: vec![],
            reads: 0,
            writes: 0,
            lose_response: false,
            read_fail: false,
        }
    }
}
#[tonic::async_trait]
impl SchemaApi for Fake {
    async fn read(&mut self) -> Result<Vec<u8>, StoreActivationError> {
        Ok(vec![])
    }
    async fn write(&mut self, _: &[u8]) -> Result<(), StoreActivationError> {
        panic!("provisioning must not write schema")
    }
}
#[tonic::async_trait]
impl GrantApi for Fake {
    async fn snapshot(&mut self, _: &[String]) -> Result<Observed, ProvisioningError> {
        self.reads += 1;
        if self.read_fail {
            return Err(ProvisioningError::Unavailable);
        }
        Ok(Observed {
            token: format!("read-{}", self.reads),
            relationships: self.rows.clone(),
        })
    }
    async fn touch(
        &mut self,
        updates: &[api::RelationshipUpdate],
    ) -> Result<String, ProvisioningError> {
        assert!(self.reads > self.writes);
        self.writes += 1;
        for update in updates {
            let row = update.relationship.clone().unwrap();
            self.rows.retain(|old| !same_identity(old, &row));
            self.rows.push(row);
        }
        if self.lose_response {
            self.lose_response = false;
            Err(ProvisioningError::Unavailable)
        } else {
            Ok(format!("write-{}", self.writes))
        }
    }
}

#[test]
fn direct_and_team_relationship_batches_read_before_touch() {
    let direct = relationship("dataset_v1", "ledger", "viewer", "agent", "key");
    let mut team = relationship("dataset_v1", "ledger", "team_viewer", "team_v1", "finance");
    team.subject.as_mut().unwrap().optional_relation = "member".into();
    for batch in [
        vec![update(direct)],
        vec![
            update(team),
            update(relationship("team_v1", "finance", "member", "agent", "key")),
        ],
    ] {
        let mut fake = Fake::new();
        assert_eq!(
            runtime()
                .block_on(write_or_reconcile(
                    &mut fake,
                    &["dataset_v1".into(), "team_v1".into()],
                    &batch
                ))
                .unwrap(),
            "write-1"
        );
        assert_eq!(fake.writes, 1);
        assert_eq!(fake.rows.len(), batch.len());
        assert_eq!(
            runtime()
                .block_on(write_or_reconcile(
                    &mut fake,
                    &["dataset_v1".into()],
                    &batch
                ))
                .unwrap(),
            "read-2"
        );
        assert_eq!(fake.writes, 1);
    }
}

#[test]
fn applied_write_with_lost_response_is_reconciled_without_another_write() {
    let mut fake = Fake::new();
    fake.lose_response = true;
    let batch = vec![update(relationship(
        "dataset_v1",
        "ledger",
        "viewer",
        "agent",
        "key",
    ))];
    assert_eq!(
        runtime().block_on(write_or_reconcile(
            &mut fake,
            &["dataset_v1".into()],
            &batch
        )),
        Err(ProvisioningError::Unavailable)
    );
    assert_eq!(
        runtime()
            .block_on(write_or_reconcile(
                &mut fake,
                &["dataset_v1".into()],
                &batch
            ))
            .unwrap(),
        "read-2"
    );
    assert_eq!(fake.writes, 1);
}

#[test]
fn failed_reads_or_conflicting_values_never_trigger_a_touch() {
    let desired = relationship("dataset_v1", "ledger", "viewer", "agent", "key");
    let batch = vec![update(desired.clone())];
    let mut fake = Fake::new();
    fake.read_fail = true;
    assert_eq!(
        runtime().block_on(write_or_reconcile(
            &mut fake,
            &["dataset_v1".into()],
            &batch
        )),
        Err(ProvisioningError::Unavailable)
    );
    fake.read_fail = false;
    let mut changed = desired;
    changed.optional_expires_at = Some(prost_types::Timestamp {
        seconds: 1,
        nanos: 0,
    });
    fake.rows.push(changed);
    assert_eq!(
        runtime().block_on(write_or_reconcile(
            &mut fake,
            &["dataset_v1".into()],
            &batch
        )),
        Err(ProvisioningError::Conflict)
    );
    assert_eq!(fake.writes, 0);
}

#[test]
fn relationship_limit_counts_existing_rows_and_missing_grants() {
    let mut fake = Fake::new();
    fake.rows = (0..256)
        .map(|i| relationship("dataset_v1", &i.to_string(), "viewer", "agent", "key"))
        .collect();
    let batch = vec![update(relationship(
        "dataset_v1",
        "new",
        "viewer",
        "agent",
        "key",
    ))];
    assert_eq!(
        runtime().block_on(write_or_reconcile(
            &mut fake,
            &["dataset_v1".into()],
            &batch
        )),
        Err(ProvisioningError::Invalid)
    );
    assert_eq!(fake.writes, 0);
}

pub(super) fn command() -> proto::ApproveAccessRequestRequest {
    proto::ApproveAccessRequestRequest {
        agent_id: "d".repeat(32),
        request_id: "11111111-1111-4111-8111-111111111111".into(),
        expected_generation: 1,
        policy_record_id: "policy-b-1".into(),
        policy_version_id: "b".repeat(32),
        approval_epoch: 1,
        dataset: Some(proto::AccessDatasetRevision {
            dataset_id: "ledger".into(),
            revision: 1,
            checksum: vec![0x22; 32],
        }),
        grant: Some(proto::AccessGrantInputs {
            format_version: 1,
            team_id: "finance".into(),
            organization_id: "owner".into(),
            approved_amount: 100,
            is_approved: true,
            is_suspended: false,
            is_override: false,
        }),
    }
}
fn stored(command: &proto::ApproveAccessRequestRequest) -> StoredRequest {
    StoredRequest {
        format_version: 1,
        agent_id: command.agent_id.clone(),
        request_id: command.request_id.clone(),
        proposal_generation: 1,
        command: command.encode_to_vec(),
        original: vec![1],
        policy_record_id: command.policy_record_id.clone(),
        policy_hash: vec![1; 32],
        source_schema_digest: vec![2; 32],
        relationships: vec![vec![1]],
        accepted_schema: "definition agent {}".into(),
        accepted_reflection: b"{}".to_vec(),
        zed_token: None,
    }
}

#[test]
fn private_write_state_survives_reload_and_completed_handoff_is_immutable() {
    let fixture = super::super::catalogue::tests::Fixture::new();
    let command = command();
    let store = RequestStore::open(&fixture.config.publication_root, &command, 1).unwrap();
    let mut record = stored(&command);
    store.persist(&record).unwrap();
    let loaded = store.read().unwrap().unwrap();
    assert!(loaded.same_input(&record));
    let root = fixture
        .config
        .publication_root
        .join("requests")
        .join(&command.agent_id)
        .join(&command.request_id);
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(root.join("1.pb"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    record.zed_token = Some("opaque-exact-revision".into());
    assert_eq!(
        format!("{record:?}"),
        "StoredRequest { private_fields: \"<redacted>\" }"
    );
    store.persist(&record).unwrap();
    store.persist(&record).unwrap();
    let mut changed = record.clone();
    changed.zed_token = Some("other-revision".into());
    assert_eq!(store.persist(&changed), Err(ProvisioningError::Conflict));
    changed = record.clone();
    changed.original.push(2);
    assert_eq!(store.persist(&changed), Err(ProvisioningError::Conflict));
    let revision = RequestRevision { stored: record };
    assert!(!format!("{revision:?}").contains("opaque-exact"));
    fs::copy(root.join("1.pb"), root.join("2.pb")).unwrap();
    assert!(
        RequestStore::open(&fixture.config.publication_root, &command, 2)
            .unwrap()
            .read()
            .is_err()
    );
}

#[test]
fn revision_bounds_fail_without_private_error_details() {
    for token in [
        "".into(),
        "x".repeat(4097),
        "token\n".into(),
        "\u{00e9}".into(),
    ] {
        assert_eq!(
            network::revision(Some(api::ZedToken { token })),
            Err(ProvisioningError::Unavailable)
        );
    }
}
