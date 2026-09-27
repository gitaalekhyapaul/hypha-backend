use std::fs::{self, File};
use std::os::unix::fs::DirBuilderExt;

use crate::io::atomic_write;
use crate::policy_publication::catalogue::{
    self,
    tests::{Fixture, NOW},
};

use super::*;

fn inputs() -> (
    proto::ApproveAccessRequestRequest,
    proto::CreateAccessRequestRequest,
) {
    let dataset = proto::AccessDatasetRevision {
        dataset_id: "processor-ledger".into(),
        revision: 2,
        checksum: vec![0x33; 32],
    };
    let command = proto::ApproveAccessRequestRequest {
        agent_id: "d".repeat(32),
        request_id: "11111111-1111-4111-8111-111111111111".into(),
        expected_generation: 1,
        policy_record_id: "policy-b-1".into(),
        policy_version_id: "b".repeat(32),
        approval_epoch: 1,
        dataset: Some(dataset.clone()),
        grant: Some(proto::AccessGrantInputs {
            format_version: 1,
            team_id: "finance-operations".into(),
            organization_id: "nus-fintech-lab".into(),
            approved_amount: 50_000,
            is_approved: true,
            is_suspended: false,
            is_override: false,
        }),
    };
    let original = proto::CreateAccessRequestRequest {
        owner: Some(proto::AccessRequestOwner {
            agent_id: command.agent_id.clone(),
            agent_public_key: vec![0x44; 32],
        }),
        intent: Some(proto::AccessIntent {
            format_version: 1,
            request_id: command.request_id.clone(),
            task_id: "reconcile".into(),
            purpose: "payment reconciliation".into(),
            action: "query".into(),
            dataset_id: dataset.dataset_id.clone(),
            dataset_revision: dataset.revision,
            request_context: vec![],
        }),
        dataset: Some(dataset),
    };
    (command, original)
}

fn config(f: &Fixture) -> ApprovalConfig {
    let binding_root = f.config.package_root.parent().unwrap().join("bindings");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&binding_root)
        .unwrap();
    ApprovalConfig {
        publication_root: f.config.publication_root.clone(),
        package_root: f.config.package_root.clone(),
        registry_root: f.config.registry_root.clone(),
        binding_root,
        trust_domain_id: f.config.trust_domain_id.clone(),
    }
}

fn installed() -> (Fixture, ApprovalConfig) {
    let mut f = Fixture::new();
    f.retain('b', 1, 1);
    f.install(&[('b', 1)]);
    let c = config(&f);
    (f, c)
}

#[test]
fn exact_approval_and_retry_retain_one_immutable_binding() {
    let (f, c) = installed();
    let (command, original) = inputs();
    let before = fs::read(c.publication_root.join("installed.json")).unwrap();
    let binding = c.prepare(&command, &original, NOW).unwrap();
    assert_eq!(binding, c.prepare(&command, &original, NOW + 1).unwrap());
    let catalogue = catalogue::list_approved(&f.config, NOW).unwrap();
    assert_eq!(binding.policy_record_id, catalogue.policies[0].record_id);
    assert_eq!(
        crate::io::lowercase_hex(&binding.policy_hash),
        catalogue.policies[0].policy_hash
    );
    let stored = DatasetBindingStore::open(c.binding_root.clone())
        .unwrap()
        .read(
            &crate::io::lowercase_hex(&binding.policy_hash),
            &binding.resource_type,
            &binding.resource_scope,
        )
        .unwrap();
    assert_eq!(stored.dataset_id, "processor-ledger");
    assert_eq!(stored.dataset_revision, "2");
    assert_eq!(stored.dataset_checksum, "33".repeat(32));
    assert_eq!(
        before,
        fs::read(c.publication_root.join("installed.json")).unwrap()
    );
}

#[test]
fn wrong_record_version_epoch_and_uninstalled_package_conflict() {
    let (mut f, c) = installed();
    f.retain('c', 2, 1);
    let (base, original) = inputs();
    let mut changed = base.clone();
    changed.policy_record_id = "policy-c-1".into();
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed.policy_version_id = "c".repeat(32);
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed = base.clone();
    changed.policy_version_id = "e".repeat(32);
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed = base;
    changed.approval_epoch = 2;
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    assert_eq!(fs::read_dir(c.binding_root).unwrap().count(), 0);
}

#[test]
fn publication_of_another_version_keeps_the_selected_approval_usable() {
    let (mut f, c) = installed();
    f.retain('c', 2, 1);
    f.install(&[('b', 1), ('c', 1)]);
    let (command, original) = inputs();
    c.prepare(&command, &original, NOW).unwrap();
    assert_eq!(
        catalogue::list_approved(&f.config, NOW)
            .unwrap()
            .policies
            .len(),
        2
    );
}

#[test]
fn revoked_expired_and_superseded_approvals_cannot_be_prepared() {
    let (mut f, c) = installed();
    let (command, original) = inputs();
    assert_eq!(
        c.prepare(&command, &original, NOW + 5000),
        Err(ApprovalError::Conflict)
    );
    f.records[0].status = proto::PolicyStatus::Revoked as i32;
    f.records[0].revoked_at = Some(prost_types::Timestamp {
        seconds: NOW,
        nanos: 0,
    });
    f.publish();
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    assert_eq!(fs::read_dir(&c.binding_root).unwrap().count(), 0);
    let (mut f, c) = installed();
    f.retain('b', 1, 2);
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    f.install(&[('b', 2)]);
    let mut command = command;
    command.approval_epoch = 2;
    command.policy_record_id = "policy-b-2".into();
    c.prepare(&command, &original, NOW).unwrap();
}

#[test]
fn corrupt_registry_and_pending_or_locked_publication_fail_unavailable() {
    let (mut f, c) = installed();
    let (command, original) = inputs();
    let writer = File::open(&c.publication_root).unwrap();
    writer.try_lock().unwrap();
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Unavailable)
    );
    drop(writer);
    atomic_write(&c.publication_root.join("pending.json"), b"unfinished").unwrap();
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Unavailable)
    );
    fs::remove_file(c.publication_root.join("pending.json")).unwrap();
    f.records[0].authority_signature[0] ^= 1;
    f.publish();
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Unavailable)
    );
    assert_eq!(fs::read_dir(c.binding_root).unwrap().count(), 0);
}

#[test]
fn installed_resolver_holds_publication_lock_until_guard_drops() {
    let (f, _) = installed();
    let guard = catalogue::resolve_installed(
        &f.config,
        &PolicyPackageKey::new("b".repeat(32), 1).unwrap(),
        "policy-b-1",
        NOW,
    )
    .unwrap_or_else(|_| panic!("installed approval"));
    let writer = File::open(&f.config.publication_root).unwrap();
    assert!(writer.try_lock().is_err());
    drop(guard);
    assert!(writer.try_lock().is_ok());
}

#[test]
fn different_agent_request_or_finalized_metadata_cannot_replace_original() {
    let (_f, c) = installed();
    let (base, original) = inputs();
    let mut changed = base.clone();
    changed.agent_id = "e".repeat(32);
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed = base.clone();
    changed.request_id = "22222222-2222-4222-8222-222222222222".into();
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed = base.clone();
    changed.dataset.as_mut().unwrap().dataset_id = "bank-ledger".into();
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed = base.clone();
    changed.dataset.as_mut().unwrap().revision += 1;
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    changed = base;
    changed.dataset.as_mut().unwrap().checksum[0] ^= 1;
    assert_eq!(
        c.prepare(&changed, &original, NOW),
        Err(ApprovalError::Conflict)
    );
    assert_eq!(fs::read_dir(c.binding_root).unwrap().count(), 0);
}

#[test]
fn binding_collision_cannot_redirect_finalized_dataset() {
    let (_f, c) = installed();
    let (mut command, mut original) = inputs();
    c.prepare(&command, &original, NOW).unwrap();
    original.dataset.as_mut().unwrap().checksum[0] ^= 1;
    command.dataset = original.dataset.clone();
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Conflict)
    );
}

#[test]
fn basic_bounds_allow_lookup_without_metadata_but_prepare_requires_it() {
    let (_f, c) = installed();
    let (mut command, original) = inputs();
    command.dataset = None;
    validate_command(&command).unwrap();
    assert_eq!(
        c.prepare(&command, &original, NOW),
        Err(ApprovalError::Invalid)
    );
    let base = inputs().0;
    let mutations: &[fn(&mut proto::ApproveAccessRequestRequest)] = &[
        |c| c.agent_id = "A".repeat(32),
        |c| c.request_id = "not-a-request".into(),
        |c| c.expected_generation = 0,
        |c| c.policy_record_id = "../record".into(),
        |c| c.policy_version_id = "b".repeat(31),
        |c| c.approval_epoch = 0,
        |c| c.dataset.as_mut().unwrap().revision = 0,
        |c| c.dataset.as_mut().unwrap().revision = 9_007_199_254_740_992,
        |c| {
            c.dataset
                .as_mut()
                .unwrap()
                .checksum
                .pop()
                .map(|_| ())
                .unwrap()
        },
        |c| c.grant = None,
        |c| c.grant.as_mut().unwrap().format_version = 2,
        |c| c.grant.as_mut().unwrap().team_id = "a".repeat(129),
        |c| c.grant.as_mut().unwrap().organization_id = "..".into(),
        |c| c.grant.as_mut().unwrap().approved_amount = 0,
        |c| c.grant.as_mut().unwrap().approved_amount = 9_007_199_254_740_992,
    ];
    for mutation in mutations {
        let mut invalid = base.clone();
        mutation(&mut invalid);
        assert_eq!(validate_command(&invalid), Err(ApprovalError::Invalid));
    }
}
