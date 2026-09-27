//! Opt-in local owner integration. Reads one explicitly selected synthetic approval.
//! It writes a real grant and signed snapshot, but does not run or submit a proof.

use raziel_admin::request_provisioning::ProvisioningConfig;
use raziel_proto::raziel::v1::AccessRequestState;
use razield::access_request::{AccessCoordinator, AccessServiceConfig};
use razield::request_preparation::RequestPreparation;
use razield::service::{SnapshotServiceConfig, SystemClock, TrustedClock};

fn required(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("missing explicit integration setting: {key}"))
}

#[tokio::test]
#[ignore = "requires an explicitly selected synthetic approval and isolated M6 owner services"]
async fn approved_request_prepares_a_pinned_snapshot_without_a_proof() {
    assert_eq!(required("RAZIEL_M6_PREPARATION_TEST_APPROVED"), "1");
    let agent = required("RAZIEL_M6_TEST_AGENT_ID");
    let request = required("RAZIEL_M6_TEST_REQUEST_ID");
    let generation: u64 = required("RAZIEL_M6_TEST_REQUEST_GENERATION")
        .parse()
        .unwrap();
    let access = AccessServiceConfig::from_env().unwrap();
    let mut owner = AccessCoordinator::open(&access).unwrap();
    let before = owner
        .get(&agent, &request, SystemClock.now_unix_seconds().unwrap())
        .unwrap();
    assert_eq!(before.state, AccessRequestState::ApprovedWaiting as i32);
    assert_eq!(before.generation, generation);
    assert!(before.proposal.is_none());
    assert!(before.capability.is_empty());
    let preparation = RequestPreparation::open(
        ProvisioningConfig {
            publication_root: required("RAZIEL_POLICY_PUBLICATION_ROOT").into(),
            package_root: required("RAZIEL_POLICY_PACKAGE_ROOT").into(),
            registry_root: required("RAZIEL_REGISTRY_PATH").into(),
            binding_root: required("RAZIEL_DATASET_BINDING_ROOT").into(),
            compiler: required("RAZIEL_POLICY_COMPILER_EXECUTABLE").into(),
            token_path: required("RAZIEL_SPICEDB_TOKEN_PATH").into(),
            endpoint: required("RAZIEL_SPICEDB_ENDPOINT"),
            trust_domain_id: required("RAZIEL_TRUST_DOMAIN_ID"),
        },
        SnapshotServiceConfig::from_env().unwrap(),
    )
    .unwrap();
    let first = preparation
        .prepare(&mut owner, &agent, &request, generation, 1)
        .await
        .unwrap();
    let second = preparation
        .prepare(&mut owner, &agent, &request, generation, 1)
        .await
        .unwrap();
    assert_eq!(first.snapshot, second.snapshot);
    assert_eq!(Some(first.policy_binding.clone()), before.policy_binding);
    assert_eq!(first.task_id, before.task_id);
    assert_eq!(first.purpose, before.purpose);
    assert_eq!(first.action, before.action);
    assert_eq!(first.request_generation, generation);
    assert_eq!(first.snapshot.relationship_snapshot_root.len(), 32);
    assert_eq!(first.snapshot.authority_signature.len(), 64);
    assert_eq!(first.snapshot.policy_hash, first.policy_binding.policy_hash);
    assert_eq!(
        first.snapshot.policy_record_id,
        first.policy_binding.policy_record_id
    );
    let after = owner
        .get(&agent, &request, SystemClock.now_unix_seconds().unwrap())
        .unwrap();
    assert_eq!(after, before);
    println!(
        "b0065_preparation: snapshot=accepted, policy=pinned, retry=same_snapshot, public_state=approved_waiting, proof=not_started"
    );
}
