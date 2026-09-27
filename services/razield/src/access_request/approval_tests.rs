//! Lifecycle checks use an explicit test Redis URL and unique disposable prefixes.
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use ed25519_dalek::SigningKey;
use prost::Message;
use raziel_proto::raziel::v1::{
    AccessDatasetRevision, AccessGrantInputs, AccessIntent, AccessPolicyBinding,
    AccessRequestOwner, AccessRequestState as State, ApproveAccessRequestRequest,
    CreateAccessRequestRequest,
};

use super::approval::{ApprovalPreparer, StoredApproval, normalize_command};
use super::model::{NormalizedIntent, Record};
use super::private::PrivateStore;
use super::{
    AccessCoordinator, AccessError, AccessServiceConfig, RETENTION_SECONDS, REVIEW_SECONDS,
};

const NOW: i64 = 1_800_000_000;
const AGENT: &str = "11111111111111111111111111111111";
const REQUEST: &str = "00000000-0000-4000-8000-000000000001";

fn input() -> CreateAccessRequestRequest {
    CreateAccessRequestRequest {
        owner: Some(AccessRequestOwner {
            agent_id: AGENT.into(),
            agent_public_key: SigningKey::from_bytes(&[7; 32])
                .verifying_key()
                .to_bytes()
                .to_vec(),
        }),
        intent: Some(AccessIntent {
            format_version: 1,
            request_id: REQUEST.into(),
            task_id: "reconcile".into(),
            purpose: "settlement review".into(),
            action: "query".into(),
            dataset_id: "synthetic-ledger".into(),
            dataset_revision: 1,
            request_context: vec![],
        }),
        dataset: Some(AccessDatasetRevision {
            dataset_id: "synthetic-ledger".into(),
            revision: 1,
            checksum: vec![5; 32],
        }),
    }
}

fn command() -> ApproveAccessRequestRequest {
    ApproveAccessRequestRequest {
        agent_id: AGENT.into(),
        request_id: REQUEST.into(),
        expected_generation: 1,
        policy_record_id: "policy-123".into(),
        policy_version_id: "a".repeat(32),
        approval_epoch: 1,
        dataset: input().dataset,
        grant: Some(AccessGrantInputs {
            format_version: 1,
            team_id: "private-approval-team".into(),
            organization_id: "private-approval-org".into(),
            approved_amount: 50_000,
            is_approved: true,
            is_suspended: false,
            is_override: false,
        }),
    }
}

fn binding() -> AccessPolicyBinding {
    AccessPolicyBinding {
        policy_record_id: command().policy_record_id,
        policy_hash: vec![8; 32],
        resource_type: format!("dataset_{}", "a".repeat(32)),
        resource_scope: "synthetic-ledger".into(),
    }
}

fn retained() -> StoredApproval {
    StoredApproval {
        command: command(),
        binding: binding(),
        approved_at: NOW + 1,
    }
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut random = [0u8; 12];
        getrandom::fill(&mut random).unwrap();
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("access-approval-{suffix}"));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self(root)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Prepared(Arc<AtomicUsize>);
impl ApprovalPreparer for Prepared {
    fn prepare(
        &self,
        _: &ApproveAccessRequestRequest,
        _: &CreateAccessRequestRequest,
        _: i64,
    ) -> Result<AccessPolicyBinding, AccessError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(binding())
    }
}

struct Unavailable;
impl ApprovalPreparer for Unavailable {
    fn prepare(
        &self,
        _: &ApproveAccessRequestRequest,
        _: &CreateAccessRequestRequest,
        _: i64,
    ) -> Result<AccessPolicyBinding, AccessError> {
        Err(AccessError::ServiceUnavailable)
    }
}

struct Live {
    _root: Root,
    config: AccessServiceConfig,
    calls: Arc<AtomicUsize>,
    url: String,
}
impl Live {
    fn new() -> Self {
        let root = Root::new();
        let url = std::env::var("RAZIEL_ACCESS_TEST_REDIS_URL")
            .expect("explicit test Redis URL required");
        let config = AccessServiceConfig::from_values(
            "127.0.0.1:50054",
            root.0.join("private").to_str().unwrap(),
            &url,
            root.0.file_name().unwrap().to_str().unwrap(),
        )
        .unwrap();
        Self {
            _root: root,
            config,
            calls: Arc::default(),
            url,
        }
    }
    fn owner(&self) -> AccessCoordinator {
        let mut owner = AccessCoordinator::open(&self.config).unwrap();
        owner.approval = Some(Box::new(Prepared(self.calls.clone())));
        owner
    }
    fn key(&self, suffix: &str) -> String {
        format!("{}:access:v1:{suffix}", self.config.prefix())
    }
    fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    fn raw(&self) -> String {
        redis::cmd("HGET")
            .arg(self.key("records"))
            .arg(format!("{AGENT}:{REQUEST}"))
            .query(&mut self.connection())
            .unwrap()
    }
    fn record(&self) -> Record {
        Record::decode(&format!("{AGENT}:{REQUEST}"), &self.raw()).unwrap()
    }
    fn path(&self, extension: &str) -> PathBuf {
        self.config
            .private_root()
            .join(format!("{}.{}", self.record().private_ref, extension))
    }
}
impl Drop for Live {
    fn drop(&mut self) {
        let _: redis::RedisResult<i64> = redis::cmd("DEL")
            .arg(&["records", "active", "index", "due"].map(|name| self.key(name)))
            .query(&mut self.connection());
    }
}

#[test]
fn private_approval_codec_and_exact_command_comparison() {
    let input = NormalizedIntent::parse(input()).unwrap();
    let approval = retained();
    let bytes = approval.encode().unwrap();
    assert!(StoredApproval::decode(&bytes, &input).unwrap() == approval);
    let mut lookup = command();
    lookup.dataset = None;
    assert!(approval.matches_retry(&lookup));
    lookup.expected_generation = 2;
    assert!(!approval.matches_retry(&lookup));
    let mut changed = command();
    changed.grant.as_mut().unwrap().is_override = true;
    assert!(!approval.matches_retry(&changed));
    let mut unknown = bytes.clone();
    unknown.extend_from_slice(&[0x78, 0x01]);
    assert!(StoredApproval::decode(&unknown, &input).is_err());
    let mut altered = retained();
    altered.binding.resource_type = "dataset".into();
    assert!(altered.validate(&input).is_err());
    altered = retained();
    altered.command.dataset.as_mut().unwrap().checksum[0] ^= 1;
    assert!(altered.validate(&input).is_err());
    let mut invalid = command();
    invalid.grant.as_mut().unwrap().team_id = "../elsewhere".into();
    assert!(normalize_command(invalid).is_err());
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn preparation_reads_only_the_current_approval_and_cancellation_fences_it() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(input(), NOW).unwrap();
    assert!(matches!(
        owner.approved_preparation(AGENT, REQUEST, 1, NOW),
        Err(AccessError::Conflict)
    ));
    owner.approve(command(), NOW + 1).unwrap();
    let prepared = owner
        .approved_preparation(AGENT, REQUEST, 2, NOW + 2)
        .unwrap();
    assert_eq!(prepared.original, input());
    assert_eq!(prepared.approval, command());
    assert_eq!(prepared.binding, binding());
    owner.confirm_preparation(&prepared, NOW + 3).unwrap();
    assert!(matches!(
        owner.approved_preparation(AGENT, REQUEST, 1, NOW + 3),
        Err(AccessError::Conflict)
    ));
    owner.cancel(AGENT, REQUEST, 2, NOW + 4).unwrap();
    assert_eq!(
        owner.confirm_preparation(&prepared, NOW + 5),
        Err(AccessError::Conflict)
    );
}

#[test]
fn private_approval_is_immutable_owner_only_and_deleted_with_intent() {
    let root = Root::new();
    let store = PrivateStore::open(&root.0).unwrap();
    let _lock = store.lock().unwrap();
    let input = NormalizedIntent::parse(input()).unwrap();
    let record = Record::reserved(&input, NOW).unwrap();
    store.persist(&record.private_ref, &input).unwrap();
    store
        .persist_approval(&record.private_ref, &input, &retained())
        .unwrap();
    store
        .persist_approval(&record.private_ref, &input, &retained())
        .unwrap();
    let path = root.0.join(format!("{}.approval", record.private_ref));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let mut changed = retained();
    changed.command.grant.as_mut().unwrap().is_override = true;
    assert_eq!(
        store
            .persist_approval(&record.private_ref, &input, &changed)
            .unwrap_err(),
        AccessError::Conflict
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.read_approval(&record.private_ref, &input).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let backup = root.0.join("backup");
    fs::rename(&path, &backup).unwrap();
    symlink(&backup, &path).unwrap();
    assert!(store.read_approval(&record.private_ref, &input).is_err());
    fs::remove_file(&path).unwrap();
    fs::hard_link(&backup, &path).unwrap();
    assert!(store.read_approval(&record.private_ref, &input).is_err());
    fs::remove_file(&backup).unwrap();
    store.delete(&record.private_ref).unwrap();
    store.delete(&record.private_ref).unwrap();
    assert!(!path.exists());
    assert!(
        !root
            .0
            .join(format!("{}.intent", record.private_ref))
            .exists()
    );
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_approval_exact_retry_is_durable_and_private() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(input(), NOW).unwrap();
    let before = live.raw();
    let mut lookup = command();
    lookup.dataset = None;
    assert_eq!(
        owner.approve(lookup.clone(), NOW + 1).unwrap_err(),
        AccessError::NotFound
    );
    assert_eq!(live.raw(), before);
    assert!(!live.path("approval").exists());
    assert_eq!(live.calls.load(Ordering::SeqCst), 0);
    let approved = owner.approve(command(), NOW + 1).unwrap();
    assert_eq!(approved.state, State::ApprovedWaiting as i32);
    assert_eq!(approved.generation, 2);
    assert_eq!(approved.policy_binding, Some(binding()));
    assert!(
        approved.proposal.is_none()
            && approved.capability.is_empty()
            && approved.proof_job_id.is_none()
    );
    assert!(!live.raw().contains("private-approval"));
    for field in ["grant", "checksum", "approved_amount", "approval_epoch"] {
        assert!(!live.raw().contains(field));
    }
    assert!(
        !approved
            .encode_to_vec()
            .windows(16)
            .any(|bytes| bytes == b"private-approval")
    );
    drop(owner);
    let mut offline = AccessCoordinator::open(&live.config).unwrap();
    assert_eq!(offline.approve(lookup, NOW + 2).unwrap(), approved);
    assert_eq!(offline.approve(command(), NOW + 3).unwrap(), approved);
    assert_eq!(offline.get(AGENT, REQUEST, NOW + 3).unwrap(), approved);
    assert_eq!(
        offline.create(input(), NOW + 3).unwrap().request.unwrap(),
        approved
    );
    assert_eq!(live.calls.load(Ordering::SeqCst), 1);
    for mutation in 0..5 {
        let mut changed = command();
        match mutation {
            0 => changed.expected_generation = 2,
            1 => changed.policy_record_id = "policy-other".into(),
            2 => changed.grant.as_mut().unwrap().is_suspended = true,
            3 => changed.dataset.as_mut().unwrap().checksum[0] ^= 1,
            _ => changed.approval_epoch = 2,
        }
        assert_eq!(
            offline.approve(changed, NOW + 4).unwrap_err(),
            AccessError::Conflict
        );
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_approval_recovers_private_write_before_redis_without_revalidation() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(input(), NOW).unwrap();
    {
        let _lock = owner.private.lock().unwrap();
        owner
            .private
            .persist_approval(
                &live.record().private_ref,
                &NormalizedIntent::parse(input()).unwrap(),
                &retained(),
            )
            .unwrap();
    }
    drop(owner);
    let mut owner = AccessCoordinator::open(&live.config).unwrap();
    let mut different = command();
    different.grant.as_mut().unwrap().is_approved = false;
    assert_eq!(
        owner.approve(different, NOW + 2).unwrap_err(),
        AccessError::Conflict
    );
    let mut lookup = command();
    lookup.dataset = None;
    let recovered = owner.approve(lookup, NOW + 2).unwrap();
    assert_eq!(recovered.state, State::ApprovedWaiting as i32);
    assert_eq!(recovered.updated_at_unix_seconds, NOW + 1);
    assert_eq!(live.calls.load(Ordering::SeqCst), 0);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_approval_cancellation_and_expiry_never_revive_and_cleanup_private_state() {
    for state in [State::Canceled, State::Rejected, State::Expired] {
        let live = Live::new();
        let mut owner = live.owner();
        owner.create(input(), NOW).unwrap();
        let approval_path = live.path("approval");
        let intent_path = live.path("intent");
        {
            let _lock = owner.private.lock().unwrap();
            owner
                .private
                .persist_approval(
                    &live.record().private_ref,
                    &NormalizedIntent::parse(input()).unwrap(),
                    &retained(),
                )
                .unwrap();
        }
        let terminal_time = if state == State::Expired {
            NOW + REVIEW_SECONDS
        } else {
            NOW + 2
        };
        let terminal = match state {
            State::Canceled => owner.cancel(AGENT, REQUEST, 1, terminal_time).unwrap(),
            State::Rejected => owner.reject(AGENT, REQUEST, 1, terminal_time).unwrap(),
            _ => owner.get(AGENT, REQUEST, terminal_time).unwrap(),
        };
        assert_eq!(terminal.state, state as i32);
        let expected = if state == State::Expired {
            AccessError::Expired
        } else {
            AccessError::Conflict
        };
        assert_eq!(
            owner.approve(command(), terminal_time + 1).unwrap_err(),
            expected
        );
        owner
            .reconcile(terminal_time + RETENTION_SECONDS - 1)
            .unwrap();
        assert!(approval_path.exists() && intent_path.exists());
        owner.reconcile(terminal_time + RETENTION_SECONDS).unwrap();
        assert!(!approval_path.exists() && !intent_path.exists());
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_published_approval_missing_private_record_fails_closed() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(input(), NOW).unwrap();
    owner.approve(command(), NOW + 1).unwrap();
    fs::remove_file(live.path("approval")).unwrap();
    assert_eq!(
        owner.approve(command(), NOW + 2).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert_eq!(
        owner.get(AGENT, REQUEST, NOW + 2).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert_eq!(
        owner.create(input(), NOW + 2).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert_eq!(
        owner.cancel(AGENT, REQUEST, 2, NOW + 2).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert!(!live.path("approval").exists());
    assert_eq!(live.calls.load(Ordering::SeqCst), 1);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_published_approval_cancels_and_expires_without_losing_comparator() {
    for expire in [false, true] {
        let live = Live::new();
        let mut owner = live.owner();
        owner.create(input(), NOW).unwrap();
        owner.approve(command(), NOW + 1).unwrap();
        let now = if expire {
            NOW + REVIEW_SECONDS
        } else {
            NOW + 2
        };
        let terminal = if expire {
            owner.get(AGENT, REQUEST, now).unwrap()
        } else {
            owner.cancel(AGENT, REQUEST, 2, now).unwrap()
        };
        assert_eq!(
            terminal.state,
            if expire {
                State::Expired
            } else {
                State::Canceled
            } as i32
        );
        assert_eq!(terminal.generation, 3);
        assert_eq!(terminal.policy_binding, Some(binding()));
        assert_eq!(
            owner.approve(command(), now).unwrap_err(),
            if expire {
                AccessError::Expired
            } else {
                AccessError::Conflict
            }
        );
        assert!(live.path("intent").exists() && live.path("approval").exists());
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_new_approval_checks_generation_and_dataset_before_validator() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(input(), NOW).unwrap();
    let before = live.raw();
    let mut changed = command();
    changed.expected_generation = 2;
    assert_eq!(
        owner.approve(changed, NOW + 1).unwrap_err(),
        AccessError::Conflict
    );
    changed = command();
    changed.dataset.as_mut().unwrap().checksum[0] ^= 1;
    assert_eq!(
        owner.approve(changed, NOW + 1).unwrap_err(),
        AccessError::Conflict
    );
    assert_eq!(live.raw(), before);
    assert_eq!(live.calls.load(Ordering::SeqCst), 0);
    owner.approval = None;
    assert_eq!(
        owner.approve(command(), NOW + 1).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert!(!live.path("approval").exists());
    owner.approval = Some(Box::new(Unavailable));
    assert_eq!(
        owner.approve(command(), NOW + 1).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert_eq!(live.raw(), before);
    assert!(!live.path("approval").exists());
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_public_binding_cannot_replace_private_approval() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(input(), NOW).unwrap();
    owner.approve(command(), NOW + 1).unwrap();
    let mut record = live.record();
    record.policy_binding.as_mut().unwrap().policy_hash[0] ^= 1;
    let _: i64 = redis::cmd("HSET")
        .arg(live.key("records"))
        .arg(format!("{AGENT}:{REQUEST}"))
        .arg(record.encode().unwrap())
        .query(&mut live.connection())
        .unwrap();
    assert_eq!(
        owner.get(AGENT, REQUEST, NOW + 2).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert_eq!(
        owner.approve(command(), NOW + 2).unwrap_err(),
        AccessError::ServiceUnavailable
    );
}
