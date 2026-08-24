use std::time::{SystemTime, UNIX_EPOCH};

use prost_types::Timestamp;
use raziel_proto::raziel::v1::snapshot_service_client::SnapshotServiceClient;
use raziel_proto::raziel::v1::{
    BuildSnapshotRequest, GetSnapshotRequest, RevokeSnapshotRequest, SnapshotStatus,
};

#[tokio::test]
#[ignore = "requires the live B-0026 service profile and owner fixture"]
async fn build_get_and_revoke_snapshot_through_public_rpc() {
    let endpoint = std::env::var("RAZIEL_SNAPSHOT_TEST_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:50052".into());
    let policy_hash = decode_hash(&required("RAZIEL_SNAPSHOT_TEST_POLICY_HASH"));
    let authority_key_id = required("RAZIEL_SNAPSHOT_TEST_AUTHORITY_KEY_ID");
    let policy_record_id = required("RAZIEL_SNAPSHOT_TEST_POLICY_RECORD_ID");
    let expected_previous_epoch = std::env::var("RAZIEL_SNAPSHOT_TEST_PREVIOUS_EPOCH")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut client = SnapshotServiceClient::connect(endpoint).await.unwrap();

    let built = client
        .build_snapshot(BuildSnapshotRequest {
            spicedb_endpoint_ref: "owner-spicedb".into(),
            zed_token_secret_ref: "reconciliation-revision".into(),
            policy_record_id,
            policy_hash: policy_hash.to_vec(),
            expires_at: Some(Timestamp {
                seconds: now + 3_600,
                nanos: 0,
            }),
            expected_previous_epoch,
            authority_key_id,
        })
        .await
        .unwrap()
        .into_inner()
        .snapshot
        .unwrap();
    assert!(built.snapshot_id.starts_with("snapshot-v1:"));
    assert_eq!(built.relationship_snapshot_root.len(), 32);
    assert_eq!(built.policy_hash, policy_hash);
    assert!(!built.spicedb_revision.is_empty());

    let found = client
        .get_snapshot(GetSnapshotRequest {
            snapshot_id: built.snapshot_id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(found.status, SnapshotStatus::Accepted as i32);
    assert_eq!(found.snapshot.unwrap(), built);

    let revoked = client
        .revoke_snapshot(RevokeSnapshotRequest {
            snapshot_id: built.snapshot_id.clone(),
            reason: "B-0026 live lifecycle gate".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(revoked.snapshot_id, built.snapshot_id);
    assert_eq!(revoked.status, SnapshotStatus::Revoked as i32);
    assert!(revoked.revoked_at.is_some());

    let found = client
        .get_snapshot(GetSnapshotRequest {
            snapshot_id: built.snapshot_id,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(found.status, SnapshotStatus::Revoked as i32);
}

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing {name}"))
}

fn decode_hash(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    output
}
