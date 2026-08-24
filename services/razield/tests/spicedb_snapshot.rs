use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_policy_ir::{
    AllowedSubjectType, ObjectDefinition, PermissionDefinition, PermissionExpr, PolicyProgram,
    RelationDefinition, ValidatedPolicyProgram,
};
use razield::spicedb::SpiceDbSnapshotReader;
use spicedb_rs_proto::authzed::api::v1 as authzed;
use spicedb_rs_proto::authzed::api::v1::permissions_service_client::PermissionsServiceClient;
use spicedb_rs_proto::authzed::api::v1::schema_service_client::SchemaServiceClient;
use tonic::Request;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::Endpoint;

const SCHEMA: &str = r#"
definition dataset {
  relation viewer: user
  permission view = viewer
}

definition user {}
"#;

#[tokio::test]
#[ignore = "requires the pinned live SpiceDB service"]
async fn generated_v1_53_client_reads_exact_revision_from_v1_54_server() {
    let endpoint = std::env::var("RAZIEL_SPICEDB_TEST_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:50051".into());
    let token = std::env::var("RAZIEL_SPICEDB_TEST_TOKEN")
        .unwrap_or_else(|_| "raziel-local-development-only".into());
    let channel = Endpoint::from_shared(endpoint.clone())
        .unwrap()
        .connect()
        .await
        .unwrap();
    let authorization = authorization(&token);
    let mut schema = SchemaServiceClient::new(channel.clone());
    let mut permissions = PermissionsServiceClient::new(channel);

    schema
        .write_schema(authenticated(
            authzed::WriteSchemaRequest {
                schema: SCHEMA.into(),
            },
            &authorization,
        ))
        .await
        .unwrap();
    let written = permissions
        .write_relationships(authenticated(
            authzed::WriteRelationshipsRequest {
                updates: vec![authzed::RelationshipUpdate {
                    operation: authzed::relationship_update::Operation::Touch as i32,
                    relationship: Some(authzed::Relationship {
                        resource: Some(authzed::ObjectReference {
                            object_type: "dataset".into(),
                            object_id: "sg-payments".into(),
                        }),
                        relation: "viewer".into(),
                        subject: Some(authzed::SubjectReference {
                            object: Some(authzed::ObjectReference {
                                object_type: "user".into(),
                                object_id: "maya".into(),
                            }),
                            optional_relation: String::new(),
                        }),
                        optional_caveat: None,
                        optional_expires_at: None,
                    }),
                }],
                optional_preconditions: Vec::new(),
                optional_transaction_metadata: None,
            },
            &authorization,
        ))
        .await
        .unwrap()
        .into_inner();
    let revision = written.written_at.unwrap().token;
    if let Ok(path) = std::env::var("RAZIEL_ZED_TOKEN_OUTPUT_PATH") {
        write_revision(&path, &revision);
    }

    let mut reader = SpiceDbSnapshotReader::connect(&endpoint, &token)
        .await
        .unwrap();
    let relationships = reader.read_exact(&policy(), &revision).await.unwrap();
    assert_eq!(relationships.len(), 1);
    assert_eq!(relationships[0].resource_id, "sg-payments");
    assert_eq!(relationships[0].subject_id, "maya");
}

fn write_revision(path: &str, revision: &str) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(revision.as_bytes()).unwrap();
    file.sync_all().unwrap();
}

fn authorization(token: &str) -> MetadataValue<Ascii> {
    let mut value: MetadataValue<Ascii> = format!("Bearer {token}").try_into().unwrap();
    value.set_sensitive(true);
    value
}

fn authenticated<T>(value: T, authorization: &MetadataValue<Ascii>) -> Request<T> {
    let mut request = Request::new(value);
    request
        .metadata_mut()
        .insert("authorization", authorization.clone());
    request
}

fn policy() -> ValidatedPolicyProgram {
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "live-test".into(),
            compiler_version: "1".into(),
            source_schema_digest: [0x11; 32],
            definitions: vec![
                ObjectDefinition {
                    name: "dataset".into(),
                    relations: vec![RelationDefinition {
                        name: "viewer".into(),
                        allowed_subject_types: vec![AllowedSubjectType {
                            object_type: "user".into(),
                            subject_relation: None,
                            caveat_name: None,
                        }],
                    }],
                },
                ObjectDefinition {
                    name: "user".into(),
                    relations: Vec::new(),
                },
            ],
            permissions: vec![PermissionDefinition {
                object_type: "dataset".into(),
                name: "view".into(),
                expression: PermissionExpr::RelationRef {
                    name: "viewer".into(),
                },
            }],
            caveats: Vec::new(),
            limits: TRUSTED_POLICY_CEILINGS.policy_limits,
        },
        &TRUSTED_POLICY_CEILINGS,
    )
    .unwrap()
}
