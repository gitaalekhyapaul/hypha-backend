use std::fs;
use std::os::unix::fs::DirBuilderExt;

use super::super::catalogue::tests::{Fixture, NOW};
use super::super::compiler::{Composition, Input};
use super::super::files::Artifacts;
use super::super::network::{SchemaApi, SchemaClient};
use super::*;

fn install(fixture: &Fixture, compiler: &Compiler, versions: &[char]) -> Composition {
    let mut packages = PolicyPackageStore::new(
        fixture.config.package_root.clone(),
        fixture.config.registry_root.clone(),
        &fixture.config.trust_domain_id,
    )
    .unwrap();
    let inputs = versions
        .iter()
        .map(|id| {
            let version = id.to_string().repeat(32);
            let package = packages
                .read_approved(&PolicyPackageKey::new(version.clone(), 1).unwrap(), NOW)
                .unwrap();
            Input {
                version_id: Some(version),
                artifacts: Artifacts {
                    schema: package.source_schema().into(),
                    config: package.compiler_config().into(),
                    policy: package.policy_pb().into(),
                    manifest: package.candidate_manifest().into(),
                },
            }
        })
        .collect::<Vec<_>>();
    let composition = compiler.compose(&inputs).unwrap();
    let state = serde_json::json!({
        "format_version": 1, "generation": versions.len(), "legacy": false,
        "schema_sha256": composition.digest(),
        "packages": versions.iter().map(|id| serde_json::json!({
            "version_id": id.to_string().repeat(32), "approval_epoch": 1
        })).collect::<Vec<_>>()
    });
    crate::io::atomic_write(
        &fixture.config.publication_root.join("installed.json"),
        &serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
    composition
}

#[test]
#[ignore = "requires a dedicated empty pinned SpiceDB and the pinned compiler"]
fn live_provisioning_preserves_exact_revision_across_publication_and_retry() {
    assert_eq!(
        std::env::var("RAZIEL_PROVISIONING_LIVE_APPROVED").as_deref(),
        Ok("1")
    );
    let mut fixture = Fixture::new();
    fixture.retain('b', 1, 1);
    let root = fixture.config.publication_root.parent().unwrap();
    let binding_root = root.join("bindings");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&binding_root)
        .unwrap();
    let token_path = root.join("spicedb-token");
    crate::io::atomic_write(&token_path, b"raziel-provisioning-isolated-test").unwrap();
    let config = ProvisioningConfig {
        publication_root: fixture.config.publication_root.clone(),
        package_root: fixture.config.package_root.clone(),
        registry_root: fixture.config.registry_root.clone(),
        binding_root,
        compiler: std::env::var_os("RAZIEL_PROVISIONING_COMPILER")
            .unwrap()
            .into(),
        token_path,
        endpoint: std::env::var("RAZIEL_PROVISIONING_ENDPOINT").unwrap(),
        trust_domain_id: fixture.config.trust_domain_id.clone(),
    };
    let compiler = Compiler::new(&config.compiler, &config.publication_root).unwrap();
    let first = install(&fixture, &compiler, &['b']);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut client = SchemaClient::connect(&config.endpoint, &config.token_path)
            .await
            .unwrap();
        assert!(
            client.read().await.unwrap().is_empty(),
            "live gate requires an empty isolated schema"
        );
        client.write(&first.schema).await.unwrap();
    });
    let command = super::tests::command();
    let original = proto::CreateAccessRequestRequest {
        owner: Some(proto::AccessRequestOwner {
            agent_id: command.agent_id.clone(),
            agent_public_key: vec![
                0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
                0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
                0xf7, 0x07, 0x51, 0x1a,
            ],
        }),
        intent: Some(proto::AccessIntent {
            format_version: 1,
            request_id: command.request_id.clone(),
            task_id: "reconcile".into(),
            purpose: "reconcile payments".into(),
            action: "query".into(),
            dataset_id: "ledger".into(),
            dataset_revision: 1,
            request_context: vec![],
        }),
        dataset: command.dataset.clone(),
    };
    let first_revision = config.prepare(&command, &original, 2, NOW).unwrap();
    assert_eq!(first_revision.proposal_generation(), 2);
    assert_eq!(first_revision.accepted_relationship_count(), 1);
    assert_eq!(
        first_revision.accepted_composition_schema().as_bytes(),
        first.schema
    );
    let retry = config.prepare(&command, &original, 2, NOW).unwrap();
    assert_eq!(retry.zed_token(), first_revision.zed_token());
    assert!(retry.stored == first_revision.stored);
    let lock = File::open(&config.publication_root).unwrap();
    assert!(
        lock.try_lock().is_ok(),
        "preparation must release publication lock"
    );
    fixture.retain('c', 2, 1);
    let second = install(&fixture, &compiler, &['b', 'c']);
    runtime.block_on(async {
        SchemaClient::connect(&config.endpoint, &config.token_path)
            .await
            .unwrap()
            .write(&second.schema)
            .await
            .unwrap();
    });
    drop(lock);
    let retry = config.prepare(&command, &original, 2, NOW).unwrap();
    assert!(retry.stored == first_revision.stored);
    let mut altered = command.clone();
    altered.grant.as_mut().unwrap().approved_amount += 1;
    assert!(matches!(
        config.prepare(&altered, &original, 2, NOW),
        Err(ProvisioningError::Conflict)
    ));
    runtime.block_on(async {
        let mut client = GrantClient::connect(&config.endpoint, &config.token_path)
            .await
            .unwrap();
        let observed = client
            .snapshot(&[format!("dataset_{}", "b".repeat(32))])
            .await
            .unwrap();
        assert_eq!(observed.relationships.len(), 1);
        // A separate proposal owns its own revision, but does not duplicate a grant.
    });
    let later = config.prepare(&command, &original, 3, NOW).unwrap();
    assert_eq!(later.proposal_generation(), 3);
    assert_eq!(
        later.accepted_composition_schema().as_bytes(),
        second.schema
    );
    crate::io::atomic_write(&config.publication_root.join("pending.json"), b"pending").unwrap();
    assert!(matches!(
        config.prepare(&command, &original, 3, NOW),
        Err(ProvisioningError::Unavailable)
    ));
}
