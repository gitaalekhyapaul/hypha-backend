use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::policy_activation::{PolicyActivationClient, PolicyActivationClientError};

#[test]
fn catalogue_operation_rejects_activation_fields_and_unknown_discriminators() {
    let base = PolicyActivationRequestWire {
        format_version: 1,
        candidate_id: String::new(),
        approved_by: String::new(),
        operation: PolicyOperationWire::ListCatalogue as i32,
    };
    for request in [
        PolicyActivationRequestWire {
            format_version: 2,
            ..base.clone()
        },
        PolicyActivationRequestWire {
            operation: 2,
            ..base.clone()
        },
        PolicyActivationRequestWire {
            candidate_id: "candidate".into(),
            ..base.clone()
        },
        PolicyActivationRequestWire {
            approved_by: "owner".into(),
            ..base.clone()
        },
    ] {
        let response = dispatch_request(
            request,
            |_| panic!("unexpected activation"),
            || panic!("unexpected catalogue read"),
        );
        assert_eq!(
            response.failure,
            PolicyActivationFailureWire::InvalidRequest as i32
        );
        assert!(response.projection.is_none() && response.catalogue.is_none());
    }
    let response = dispatch_request(
        base,
        |_| panic!("unexpected activation"),
        || {
            Ok(super::super::wire::PolicyCatalogueWire {
                format_version: 1,
                policies: vec![],
            })
        },
    );
    assert_eq!(response.failure, 0);
    assert!(response.projection.is_none());
    assert!(response.catalogue.unwrap().policies.is_empty());
}

#[test]
fn real_owner_socket_accepts_composed_catalogue_and_legacy_refuses_it() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "rz-cat-rpc-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    for name in ["publication", "packages", "versions"] {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(root.join(name))
            .unwrap();
    }
    let store = || {
        PolicyActivationStore::new(
            root.join("candidates"),
            root.join("activation"),
            root.join("registry"),
            root.join("authority.sock"),
            root.join("writer.sock"),
            "catalogue-rpc-test",
        )
        .unwrap()
    };
    let config = PublicationConfig {
        root: root.join("publication"),
        package_root: root.join("packages"),
        version_root: root.join("versions"),
        candidate_root: root.join("candidates"),
        registry_root: root.join("registry"),
        compiler: root.join("compiler-unused"),
        legacy_root: None,
        token_path: root.join("token-unused"),
        endpoint: "http://unused.invalid:50051".into(),
        trust_domain_id: "catalogue-rpc-test".into(),
    };
    let composed =
        ActivationRole::Composed(Box::new(PolicyPublisher::new(config, store()).unwrap()));
    let legacy = ActivationRole::Legacy(Box::new(store()));
    for (role, expected) in [
        (composed, None),
        (legacy, Some(PolicyActivationClientError::InvalidRequest)),
    ] {
        let path = root.join("owner.sock");
        let listener = bind_private_socket(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let response = handle_connection(&role, &mut stream);
            write_response(&mut stream, &response).unwrap();
        });
        let result = PolicyActivationClient::new(path).unwrap().list_catalogue();
        if let Some(error) = expected {
            assert_eq!(result, Err(error));
        } else {
            assert!(result.unwrap().policies.is_empty());
        }
        server.join().unwrap();
    }
    assert_eq!(
        std::fs::read_dir(root.join("publication")).unwrap().count(),
        0
    );
    assert!(!root.join("registry").exists());
    std::fs::remove_dir_all(root).unwrap();
}
