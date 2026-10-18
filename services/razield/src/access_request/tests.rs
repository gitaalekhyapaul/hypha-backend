//! Live tests use RAZIEL_ACCESS_TEST_REDIS_URL and delete only their unique prefixes.

use std::fs;
use std::net::TcpListener;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use ed25519_dalek::SigningKey;
use prost::Message;
use raziel_proto::raziel::v1::{
    AccessDatasetRevision, AccessIntent, AccessRequestOwner, AccessRequestState as State,
    AuthorizationContextEntry, CreateAccessRequestRequest, GetAccessRequestRequest,
    ListAccessRequestsRequest, access_request_service_client::AccessRequestServiceClient,
    authorization_context_entry::Value,
};
use razield::access_request::{
    AccessCoordinator, AccessError, AccessRequestHandler, AccessServiceConfig, MAX_INTENT_BYTES,
    RETENTION_SECONDS, REVIEW_SECONDS, bounded_access_service,
};
use razield::service::{ClockError, TrustedClock};

const NOW: i64 = 1_800_000_000;
const A: &str = "11111111111111111111111111111111";
const B: &str = "22222222222222222222222222222222";

fn intent(agent: &str, index: u64) -> CreateAccessRequestRequest {
    CreateAccessRequestRequest {
        owner: Some(AccessRequestOwner {
            agent_id: agent.to_owned(),
            agent_public_key: SigningKey::from_bytes(&[7; 32])
                .verifying_key()
                .to_bytes()
                .to_vec(),
        }),
        intent: Some(AccessIntent {
            format_version: 1,
            request_id: format!("00000000-0000-4000-8000-{index:012x}"),
            task_id: "reconcile".into(),
            purpose: "settlement review".into(),
            action: "query".into(),
            dataset_id: "synthetic-ledger".into(),
            dataset_revision: 1,
            request_context: vec![
                AuthorizationContextEntry {
                    name: "secret".into(),
                    value: Some(Value::StringValue("private-test-marker-92".into())),
                },
                AuthorizationContextEntry {
                    name: "count".into(),
                    value: Some(Value::IntValue(5)),
                },
            ],
        }),
        dataset: Some(AccessDatasetRevision {
            dataset_id: "synthetic-ledger".into(),
            revision: 1,
            checksum: vec![5; 32],
        }),
    }
}

fn id(index: u64) -> String {
    intent(A, index).intent.unwrap().request_id
}

struct Live {
    root: PathBuf,
    config: AccessServiceConfig,
    url: String,
}

impl Live {
    fn new() -> Self {
        let mut random = [0u8; 12];
        getrandom::fill(&mut random).unwrap();
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("raziel-access-test-{suffix}"));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let url = std::env::var("RAZIEL_ACCESS_TEST_REDIS_URL")
            .expect("explicit test Redis URL required");
        let config = AccessServiceConfig::from_values(
            "127.0.0.1:50054",
            root.join("private").to_str().unwrap(),
            &url,
            &format!("access-test-{suffix}"),
        )
        .unwrap();
        Self { root, config, url }
    }

    fn owner(&self) -> AccessCoordinator {
        AccessCoordinator::open(&self.config).unwrap()
    }
    fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    fn key(&self, suffix: &str) -> String {
        format!("{}:access:v1:{suffix}", self.config.prefix())
    }
    fn raw(&self, agent: &str, request: &str) -> serde_json::Value {
        let raw: String = redis::cmd("HGET")
            .arg(self.key("records"))
            .arg(format!("{agent}:{request}"))
            .query(&mut self.connection())
            .unwrap();
        serde_json::from_str(&raw).unwrap()
    }
    fn put_raw(&self, agent: &str, request: &str, raw: &serde_json::Value) {
        let _: i64 = redis::cmd("HSET")
            .arg(self.key("records"))
            .arg(format!("{agent}:{request}"))
            .arg(serde_json::to_string(raw).unwrap())
            .query(&mut self.connection())
            .unwrap();
    }
    fn intent_path(&self, agent: &str, index: u64) -> PathBuf {
        let raw = self.raw(agent, &id(index));
        self.config
            .private_root()
            .join(format!("{}.intent", raw["private_ref"].as_str().unwrap()))
    }
    fn files(&self) -> Vec<PathBuf> {
        fs::read_dir(self.config.private_root())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Ok(mut connection) = redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
        {
            let keys = ["records", "active", "index", "due"].map(|suffix| self.key(suffix));
            let _: redis::RedisResult<i64> = redis::cmd("DEL").arg(&keys).query(&mut connection);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn explicit_configuration_and_error_contract() {
    let config = AccessServiceConfig::from_values(
        "0.0.0.0:50054",
        "/private/tmp/access-owner",
        "redis://:secret@127.0.0.1:6379/0",
        "installation-1",
    )
    .unwrap();
    assert_eq!(config.bind_address().port(), 50054);
    assert!(!format!("{config:?}").contains("secret"));
    for (bind, root, url, prefix) in [
        ("127.0.0.1:0", "/private/tmp/a", "redis://127.0.0.1", "a"),
        ("127.0.0.1:50054", "relative", "redis://127.0.0.1", "a"),
        ("127.0.0.1:50054", "/", "redis://127.0.0.1", "a"),
        ("127.0.0.1:50054", "/private/tmp/a", "http://127.0.0.1", "a"),
        (
            "127.0.0.1:50054",
            "/private/tmp/a",
            "redis://127.0.0.1",
            "a:b",
        ),
    ] {
        assert!(AccessServiceConfig::from_values(bind, root, url, prefix).is_err());
    }
    for (error, code) in [
        (AccessError::InvalidRequest, tonic::Code::InvalidArgument),
        (AccessError::NotFound, tonic::Code::NotFound),
        (AccessError::Conflict, tonic::Code::Aborted),
        (AccessError::Expired, tonic::Code::OutOfRange),
        (
            AccessError::CapacityExceeded,
            tonic::Code::ResourceExhausted,
        ),
        (AccessError::ServiceUnavailable, tonic::Code::Unavailable),
    ] {
        assert_eq!(tonic::Status::from(error).code(), code);
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_owner_isolation_normalized_private_retry_and_projection() {
    let live = Live::new();
    let mut owner = live.owner();
    let result = owner.create(intent(A, 1), NOW).unwrap();
    assert!(result.created);
    let public = result.request.unwrap();
    assert_eq!(public.state, State::PendingReview as i32);
    assert_eq!(public.generation, 1);
    assert_eq!(public.deadline_unix_seconds, NOW + REVIEW_SECONDS);
    assert!(
        public.capability.is_empty()
            && public.proposal.is_none()
            && public.policy_binding.is_none()
    );
    assert!(
        !public
            .encode_to_vec()
            .windows(22)
            .any(|w| w == b"private-test-marker-92")
    );
    let mut retry = intent(A, 1);
    retry.intent.as_mut().unwrap().request_context.reverse();
    retry.intent.as_mut().unwrap().request_id = id(1).to_uppercase();
    assert!(!owner.create(retry.clone(), NOW + 1).unwrap().created);
    retry.dataset = None;
    assert!(!owner.create(retry.clone(), NOW + 2).unwrap().created);
    retry.intent.as_mut().unwrap().request_context[0].value = Some(Value::BoolValue(true));
    assert_eq!(
        owner.create(retry, NOW + 3).unwrap_err(),
        AccessError::Conflict
    );
    let mut changed_key = intent(A, 1);
    changed_key.owner.as_mut().unwrap().agent_public_key = SigningKey::from_bytes(&[8; 32])
        .verifying_key()
        .to_bytes()
        .to_vec();
    assert_eq!(
        owner.create(changed_key, NOW).unwrap_err(),
        AccessError::Conflict
    );
    assert_eq!(
        owner.get(B, &id(1), NOW).unwrap_err(),
        AccessError::NotFound
    );
    assert!(owner.create(intent(B, 1), NOW).unwrap().created);
    assert_ne!(live.intent_path(A, 1), live.intent_path(B, 1));
    assert_eq!(
        fs::metadata(live.config.private_root())
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o700
    );
    assert_eq!(
        fs::metadata(live.intent_path(A, 1))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    let values: Vec<String> = redis::cmd("HVALS")
        .arg(live.key("records"))
        .query(&mut live.connection())
        .unwrap();
    assert!(values.iter().all(|v| !v.contains("private-test-marker")
        && !v.contains("request_context")
        && !v.contains("checksum")));
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_lookup_only_and_invalid_inputs_have_no_side_effects() {
    let live = Live::new();
    let mut owner = live.owner();
    let before = live.files();
    let mut absent = intent(A, 1);
    absent.dataset = None;
    assert_eq!(
        owner.create(absent, NOW).unwrap_err(),
        AccessError::NotFound
    );
    let mut cases = Vec::new();
    let mut invalid = intent(A, 1);
    invalid.owner.as_mut().unwrap().agent_public_key = vec![0; 32];
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.owner.as_mut().unwrap().agent_public_key = vec![1; 31];
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.owner.as_mut().unwrap().agent_id = "../owner".into();
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().request_id = "not-a-uuid".into();
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().format_version = 2;
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.dataset.as_mut().unwrap().revision = 2;
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().action = "delete".into();
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().request_context[0].value = None;
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().request_context[0].name = "count".into();
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().request_context =
        vec![AuthorizationContextEntry::default(); 17];
    cases.push(invalid);
    let mut invalid = intent(A, 1);
    invalid.intent.as_mut().unwrap().purpose = "x".repeat(MAX_INTENT_BYTES);
    cases.push(invalid);
    for invalid in cases {
        assert_eq!(
            owner.create(invalid, NOW).unwrap_err(),
            AccessError::InvalidRequest
        );
    }
    assert_eq!(live.files(), before);
    let keys = ["records", "active", "index", "due"].map(|suffix| live.key(suffix));
    let count: usize = redis::cmd("EXISTS")
        .arg(&keys)
        .query(&mut live.connection())
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_generation_terminal_retention_and_deadline() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(intent(A, 1), NOW).unwrap();
    assert_eq!(
        owner.reject(A, &id(1), 2, NOW + 1).unwrap_err(),
        AccessError::Conflict
    );
    let rejected = owner.reject(A, &id(1), 1, NOW + 1).unwrap();
    assert_eq!(rejected.state, State::Rejected as i32);
    assert_eq!(rejected.generation, 2);
    assert_eq!(owner.reject(A, &id(1), 1, NOW + 2).unwrap(), rejected);
    assert_eq!(
        owner.cancel(A, &id(1), 2, NOW + 2).unwrap_err(),
        AccessError::Conflict
    );
    let path = live.intent_path(A, 1);
    owner.reconcile(NOW + RETENTION_SECONDS).unwrap();
    assert!(path.exists());
    assert_eq!(
        owner
            .create(intent(A, 1), NOW + RETENTION_SECONDS)
            .unwrap()
            .request
            .unwrap(),
        rejected
    );
    assert_eq!(
        owner
            .get(A, &id(1), NOW + RETENTION_SECONDS + 1)
            .unwrap_err(),
        AccessError::Expired
    );
    owner.reconcile(NOW + RETENTION_SECONDS + 1).unwrap();
    assert!(!path.exists());
    assert_eq!(
        owner
            .get(A, &id(1), NOW + RETENTION_SECONDS + 1)
            .unwrap_err(),
        AccessError::NotFound
    );
    owner.create(intent(A, 2), NOW).unwrap();
    assert_eq!(
        owner
            .get(A, &id(2), NOW + REVIEW_SECONDS - 1)
            .unwrap()
            .state,
        State::PendingReview as i32
    );
    let expired = owner.get(A, &id(2), NOW + REVIEW_SECONDS).unwrap();
    assert_eq!(expired.state, State::Expired as i32);
    assert_eq!(expired.generation, 2);
    assert_eq!(
        owner
            .cancel(A, &id(2), 1, NOW + REVIEW_SECONDS)
            .unwrap_err(),
        AccessError::Expired
    );
    assert_eq!(
        owner
            .create(intent(A, 2), NOW + REVIEW_SECONDS)
            .unwrap()
            .request
            .unwrap(),
        expired
    );
    owner.create(intent(A, 3), NOW).unwrap();
    let canceled = owner.cancel(A, &id(3), 1, NOW + 1).unwrap();
    assert_eq!(owner.cancel(A, &id(3), 1, NOW + 2).unwrap(), canceled);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_capacity_is_atomic_and_never_writes_rejected_private_input() {
    let live = Live::new();
    let mut owner = live.owner();
    for index in 0..16 {
        owner.create(intent(A, index), NOW).unwrap();
    }
    let before = live.files().len();
    assert_eq!(
        owner.create(intent(A, 17), NOW).unwrap_err(),
        AccessError::CapacityExceeded
    );
    assert_eq!(live.files().len(), before);
    assert!(!owner.create(intent(A, 0), NOW).unwrap().created);
    for agent in 2..=7 {
        for index in 0..14 {
            owner
                .create(intent(&format!("{agent:032x}"), index), NOW)
                .unwrap();
        }
    }
    let count: usize = redis::cmd("ZCARD")
        .arg(live.key("active"))
        .query(&mut live.connection())
        .unwrap();
    assert_eq!(count, 100);
    let before = live.files().len();
    assert_eq!(
        owner.create(intent(B, 99), NOW).unwrap_err(),
        AccessError::CapacityExceeded
    );
    assert_eq!(live.files().len(), before);
    owner.cancel(A, &id(0), 1, NOW + 1).unwrap();
    assert!(owner.create(intent(B, 99), NOW + 2).unwrap().created);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_lists_are_bounded_and_cursors_are_owner_scoped() {
    let live = Live::new();
    let mut owner = live.owner();
    for n in 0..3 {
        owner.create(intent(A, n), NOW + n as i64).unwrap();
        owner.create(intent(B, n), NOW + n as i64).unwrap();
    }
    let first = owner
        .list(
            ListAccessRequestsRequest {
                agent_id: Some(A.into()),
                limit: 2,
                cursor: None,
            },
            NOW + 3,
        )
        .unwrap();
    assert_eq!(first.requests.len(), 2);
    assert_eq!(
        first
            .requests
            .iter()
            .map(|r| r.request_id.clone())
            .collect::<Vec<_>>(),
        vec![id(2), id(1)]
    );
    assert!(
        first
            .requests
            .iter()
            .all(|r| r.agent_id == A && r.capability.is_empty())
    );
    let cursor = first.next_cursor.unwrap();
    let next = owner
        .list(
            ListAccessRequestsRequest {
                agent_id: Some(A.into()),
                limit: 2,
                cursor: Some(cursor.clone()),
            },
            NOW + 3,
        )
        .unwrap();
    assert_eq!(next.requests.len(), 1);
    assert_eq!(next.requests[0].request_id, id(0));
    assert!(next.next_cursor.is_none());
    assert_eq!(
        owner
            .list(
                ListAccessRequestsRequest {
                    agent_id: Some(B.into()),
                    limit: 2,
                    cursor: Some(cursor.clone())
                },
                NOW + 3
            )
            .unwrap_err(),
        AccessError::InvalidRequest
    );
    assert_eq!(
        owner
            .list(
                ListAccessRequestsRequest {
                    agent_id: None,
                    limit: 2,
                    cursor: Some(cursor)
                },
                NOW + 3
            )
            .unwrap_err(),
        AccessError::InvalidRequest
    );
    assert_eq!(
        owner
            .list(
                ListAccessRequestsRequest {
                    agent_id: None,
                    limit: 51,
                    cursor: None
                },
                NOW + 3
            )
            .unwrap_err(),
        AccessError::InvalidRequest
    );
    assert_eq!(
        owner
            .list(ListAccessRequestsRequest::default(), NOW + 3)
            .unwrap()
            .requests
            .len(),
        6
    );
    // Updating the oldest request promotes it across pages, not just within a page.
    owner.reject(A, &id(0), 1, NOW + 4).unwrap();
    let recent = owner
        .list(
            ListAccessRequestsRequest {
                limit: 2,
                ..Default::default()
            },
            NOW + 4,
        )
        .unwrap();
    assert_eq!(recent.requests[0].agent_id, A);
    assert_eq!(recent.requests[0].request_id, id(0));
    assert_eq!(recent.requests[1].agent_id, B);
    assert_eq!(recent.requests[1].request_id, id(2));
    let following = owner
        .list(
            ListAccessRequestsRequest {
                limit: 2,
                cursor: recent.next_cursor,
                ..Default::default()
            },
            NOW + 4,
        )
        .unwrap();
    assert_eq!(following.requests[0].request_id, id(2));
    assert_eq!(following.requests[0].agent_id, A);
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_later_states_fail_closed_without_private_cleanup() {
    let live = Live::new();
    let mut owner = live.owner();
    for (index, state) in [
        State::Preparing,
        State::AwaitingSignature,
        State::Submitting,
        State::Queued,
        State::Executing,
        State::Proving,
        State::Cancelling,
    ]
    .iter()
    .enumerate()
    {
        let index = index as u64;
        owner.create(intent(A, index), NOW).unwrap();
        let mut raw = live.raw(A, &id(index));
        raw["state"] = (*state as i32).into();
        live.put_raw(A, &id(index), &raw);
        assert_eq!(
            owner.cancel(A, &id(index), 1, NOW + 1).unwrap_err(),
            AccessError::ServiceUnavailable
        );
        assert!(live.intent_path(A, index).exists());
        assert_eq!(live.raw(A, &id(index))["state"], *state as i32);
    }
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_private_modes_symlinks_hardlinks_and_missing_intent_fail_closed() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(intent(A, 1), NOW).unwrap();
    let path = live.intent_path(A, 1);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        owner.get(A, &id(1), NOW).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let renamed = live.root.join("retained-private");
    fs::rename(&path, &renamed).unwrap();
    symlink(&renamed, &path).unwrap();
    assert_eq!(
        owner.get(A, &id(1), NOW).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    fs::remove_file(&path).unwrap();
    fs::hard_link(&renamed, &path).unwrap();
    assert_eq!(
        owner.get(A, &id(1), NOW).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    fs::remove_file(&path).unwrap();
    assert_eq!(
        owner.create(intent(A, 1), NOW).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert!(!path.exists());
    let link_root = live.root.join("linked-root");
    symlink(live.config.private_root(), &link_root).unwrap();
    let config = AccessServiceConfig::from_values(
        "127.0.0.1:50054",
        link_root.to_str().unwrap(),
        &live.url,
        "other",
    )
    .unwrap();
    assert!(matches!(
        AccessCoordinator::open(&config),
        Err(AccessError::ServiceUnavailable)
    ));
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_reopen_and_bounded_orphan_cleanup_preserve_published_intents() {
    let live = Live::new();
    let mut owner = live.owner();
    owner.create(intent(A, 1), NOW).unwrap();
    owner.create(intent(A, 2), NOW).unwrap();
    let kept = live.intent_path(A, 1);
    let orphan = live.intent_path(A, 2);
    let mut raw = live.raw(A, &id(2));
    raw["phase"] = "reserved".into();
    raw["due"] = (NOW + 600).into();
    live.put_raw(A, &id(2), &raw);
    let _: i64 = redis::cmd("ZADD")
        .arg(live.key("due"))
        .arg(NOW + 600)
        .arg(format!("{A}:{}", id(2)))
        .query(&mut live.connection())
        .unwrap();
    drop(owner);
    let mut owner = live.owner();
    assert!(!owner.create(intent(A, 1), NOW + 1).unwrap().created);
    owner.reconcile(NOW + 599).unwrap();
    assert!(orphan.exists());
    owner.reconcile(NOW + 600).unwrap();
    assert!(!orphan.exists());
    assert!(kept.exists());
    assert_eq!(
        owner.get(A, &id(2), NOW + 600).unwrap_err(),
        AccessError::NotFound
    );
    assert_eq!(
        owner.get(A, &id(1), NOW + 600).unwrap().state,
        State::PendingReview as i32
    );
    let mut retained = live.raw(A, &id(1));
    retained["phase"] = "reserved".into();
    live.put_raw(A, &id(1), &retained);
    let mut lookup = intent(A, 1);
    lookup.dataset = None;
    let recovered = owner.create(lookup, NOW + 601).unwrap();
    assert!(!recovered.created);
    assert_eq!(live.raw(A, &id(1))["phase"], "published");
    assert!(kept.exists());
    owner.create(intent(A, 3), NOW + 601).unwrap();
    let missing = live.intent_path(A, 3);
    let mut reserved = live.raw(A, &id(3));
    reserved["phase"] = "reserved".into();
    live.put_raw(A, &id(3), &reserved);
    fs::remove_file(&missing).unwrap();
    assert_eq!(
        owner.create(intent(A, 3), NOW + 602).unwrap_err(),
        AccessError::ServiceUnavailable
    );
    assert!(!missing.exists());
}

#[test]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
fn live_independent_coordinators_retain_one_durable_identity() {
    let live = Live::new();
    let mut first = live.owner();
    let mut second = live.owner();
    let thread = thread::spawn(move || {
        for _ in 0..100 {
            match first.create(intent(A, 1), NOW) {
                Err(AccessError::ServiceUnavailable) => thread::sleep(Duration::from_millis(5)),
                result => return result.unwrap().created,
            }
        }
        panic!("first coordinator did not complete")
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let second_created = loop {
        assert!(Instant::now() < deadline, "coordinator retry deadline");
        match second.create(intent(A, 1), NOW) {
            Err(AccessError::ServiceUnavailable) => thread::sleep(Duration::from_millis(5)),
            result => break result.unwrap().created,
        }
    };
    assert_ne!(thread.join().unwrap(), second_created);
    let count: usize = redis::cmd("HLEN")
        .arg(live.key("records"))
        .query(&mut live.connection())
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(live.files().len(), 2);
}

#[derive(Clone, Copy)]
struct FixedClock;
impl TrustedClock for FixedClock {
    fn now_unix_seconds(&self) -> Result<i64, ClockError> {
        Ok(NOW)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires RAZIEL_ACCESS_TEST_REDIS_URL"]
async fn live_generated_tonic_service_and_message_ceiling() {
    let live = Live::new();
    let handler = AccessRequestHandler::with_clock(live.owner(), FixedClock);
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(bounded_access_service(handler))
            .serve_with_shutdown(([127, 0, 0, 1], port).into(), async {
                let _ = stopped.await;
            }),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut client = loop {
        assert!(Instant::now() < deadline, "Tonic startup deadline");
        match AccessRequestServiceClient::connect(format!("http://127.0.0.1:{port}")).await {
            Ok(client) => break client,
            Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    };
    let created = client
        .create_access_request(intent(A, 1))
        .await
        .unwrap()
        .into_inner();
    assert!(created.created);
    let not_owned = client
        .get_access_request(GetAccessRequestRequest {
            agent_id: B.into(),
            request_id: id(1),
        })
        .await
        .unwrap_err();
    assert_eq!(not_owned.code(), tonic::Code::NotFound);
    let mut oversized = intent(A, 2);
    oversized.intent.as_mut().unwrap().purpose = "x".repeat(MAX_INTENT_BYTES * 2);
    let rejected = client.create_access_request(oversized).await.unwrap_err();
    assert!(matches!(
        rejected.code(),
        tonic::Code::OutOfRange | tonic::Code::ResourceExhausted
    ));
    assert_eq!(live.files().len(), 2);
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}
