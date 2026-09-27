use super::*;

const SCHEMA: &str = "definition user {}\ndefinition dataset {\n relation viewer: user\n permission read = viewer\n}\ndefinition unrelated {}";

pub(super) fn source(token: &str) -> RequestSnapshotSource {
    RequestSnapshotSource {
        receipt_key: "agent:request".into(),
        proposal_generation: 1,
        policy_record_id: "policy-v1".into(),
        zed_token: token.into(),
        source_schema_digest: [0x11; 32],
        accepted_composition_schema: SCHEMA.into(),
    }
}

struct Compared {
    reflected: authzed::ReflectSchemaResponse,
    diff: authzed::DiffSchemaResponse,
    steps: Vec<&'static str>,
}
impl Compared {
    fn matching() -> Self {
        Self {
            reflected: authzed::ReflectSchemaResponse {
                definitions: ["dataset", "user", "unrelated"]
                    .into_iter()
                    .map(|name| authzed::ReflectionDefinition {
                        name: name.into(),
                        ..Default::default()
                    })
                    .collect(),
                caveats: vec![],
                read_at: Some(authzed::ZedToken {
                    token: "exact".into(),
                }),
            },
            diff: authzed::DiffSchemaResponse {
                diffs: vec![],
                read_at: Some(authzed::ZedToken {
                    token: "exact".into(),
                }),
            },
            steps: vec![],
        }
    }
}

#[tonic::async_trait]
impl SpiceDbReadApi for Compared {
    async fn reflect_schema(
        &mut self,
        request: authzed::ReflectSchemaRequest,
    ) -> Result<authzed::ReflectSchemaResponse, SpiceDbSnapshotError> {
        assert_eq!(request.consistency, Some(exact_consistency("exact")));
        assert!(request.optional_filters.is_empty());
        self.steps.push("reflect");
        Ok(self.reflected.clone())
    }
    async fn diff_schema(
        &mut self,
        request: authzed::DiffSchemaRequest,
    ) -> Result<authzed::DiffSchemaResponse, SpiceDbSnapshotError> {
        assert_eq!(request.consistency, Some(exact_consistency("exact")));
        assert_eq!(request.comparison_schema, SCHEMA);
        self.steps.push("compare");
        Ok(self.diff.clone())
    }
    async fn read_relationships(
        &mut self,
        request: authzed::ReadRelationshipsRequest,
    ) -> Result<Vec<authzed::ReadRelationshipsResponse>, SpiceDbSnapshotError> {
        assert_eq!(request.consistency, Some(exact_consistency("exact")));
        assert_ne!(
            request.relationship_filter.unwrap().resource_type,
            "unrelated"
        );
        self.steps.push("read");
        Ok(vec![])
    }
}

#[tokio::test]
async fn aggregate_is_compared_before_selected_package_relationship_reads() {
    let mut api = Compared::matching();
    read_exact_composition(&mut api, &tests::policy(), "exact", Some(SCHEMA))
        .await
        .unwrap();
    assert_eq!(api.steps, ["reflect", "compare", "read", "read"]);
}

#[tokio::test]
async fn missing_package_differences_or_mismatched_revision_fail_before_tuple_reads() {
    let mut missing = Compared::matching();
    missing.reflected.definitions.remove(0);
    let mut changed = Compared::matching();
    // An unknown difference is rejected too, not silently discarded.
    changed
        .diff
        .diffs
        .push(authzed::ReflectionSchemaDiff::default());
    let mut drifted = Compared::matching();
    drifted.diff.read_at = Some(authzed::ZedToken {
        token: "newer".into(),
    });
    for (mut api, expected) in [
        (missing, SpiceDbSnapshotError::SchemaMismatch),
        (changed, SpiceDbSnapshotError::SchemaMismatch),
        (drifted, SpiceDbSnapshotError::RevisionMismatch),
    ] {
        assert_eq!(
            read_exact_composition(&mut api, &tests::policy(), "exact", Some(SCHEMA)).await,
            Err(expected)
        );
        assert!(!api.steps.contains(&"read"));
    }
}

#[test]
fn package_digest_schema_size_and_private_reference_are_bounded() {
    let mut value = source("exact");
    value.validate(&tests::policy()).unwrap();
    value.source_schema_digest[0] ^= 1;
    assert_eq!(
        value.validate(&tests::policy()),
        Err(SpiceDbSnapshotError::SchemaMismatch)
    );
    value = source("exact");
    value.accepted_composition_schema = "x".repeat(1024 * 1024 + 1);
    assert_eq!(
        value.validate(&tests::policy()),
        Err(SpiceDbSnapshotError::CompositionLimitExceeded)
    );
    value = source("exact");
    value.receipt_key = "../request".into();
    assert_eq!(
        value.validate(&tests::policy()),
        Err(SpiceDbSnapshotError::InvalidConfiguration)
    );
    assert!(!format!("{value:?}").contains("definition"));
}

/// Run only against a disposable pinned SpiceDB instance. This overwrites schema.
#[tokio::test]
#[ignore = "requires dedicated RAZIEL_SNAPSHOT_TEST_SPICEDB_ENDPOINT and RAZIEL_SNAPSHOT_TEST_SPICEDB_TOKEN"]
async fn live_complete_schema_comparison_uses_the_historical_revision() {
    use authzed::permissions_service_client::PermissionsServiceClient;
    use authzed::schema_service_client::SchemaServiceClient;
    let endpoint = std::env::var("RAZIEL_SNAPSHOT_TEST_SPICEDB_ENDPOINT").unwrap();
    let token = std::env::var("RAZIEL_SNAPSHOT_TEST_SPICEDB_TOKEN").unwrap();
    let authorize = |value| {
        let mut request = tonic::Request::new(value);
        request
            .metadata_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        request
    };
    let mut schema = SchemaServiceClient::connect(endpoint.clone())
        .await
        .unwrap();
    let revision = schema
        .write_schema(authorize(authzed::WriteSchemaRequest {
            schema: SCHEMA.into(),
        }))
        .await
        .unwrap()
        .into_inner()
        .written_at
        .unwrap()
        .token;
    // Advance the head without altering any reflected permission names. A partial
    // reflection or head-only schema read cannot establish this historical match.
    let changed = SCHEMA.replace(
        "permission read = viewer",
        "permission read = viewer + viewer",
    );
    let newer = schema
        .write_schema(authorize(authzed::WriteSchemaRequest {
            schema: changed.clone(),
        }))
        .await
        .unwrap()
        .into_inner()
        .written_at
        .unwrap()
        .token;
    let mut reader = SpiceDbSnapshotReader::connect(&endpoint, &token)
        .await
        .unwrap();
    reader
        .read_request_source(&tests::policy(), &source(&revision))
        .await
        .unwrap();
    assert_eq!(
        reader
            .read_request_source(&tests::policy(), &source(&newer))
            .await,
        Err(SpiceDbSnapshotError::SchemaMismatch)
    );
    let mut wrong = source(&revision);
    wrong.accepted_composition_schema = SCHEMA.replace(
        "definition unrelated {}",
        "definition unrelated {\n relation reader: user\n}",
    );
    assert_eq!(
        reader.read_request_source(&tests::policy(), &wrong).await,
        Err(SpiceDbSnapshotError::SchemaMismatch)
    );
    // The exact token is not limited to schema-write revisions.
    let mut permissions = PermissionsServiceClient::connect(endpoint.clone())
        .await
        .unwrap();
    let mut write = tonic::Request::new(authzed::WriteRelationshipsRequest {
        updates: vec![authzed::RelationshipUpdate {
            operation: authzed::relationship_update::Operation::Touch as i32,
            relationship: Some(authzed::Relationship {
                resource: Some(authzed::ObjectReference {
                    object_type: "dataset".into(),
                    object_id: "one".into(),
                }),
                relation: "viewer".into(),
                subject: Some(authzed::SubjectReference {
                    object: Some(authzed::ObjectReference {
                        object_type: "user".into(),
                        object_id: "agent".into(),
                    }),
                    optional_relation: String::new(),
                }),
                ..Default::default()
            }),
        }],
        ..Default::default()
    });
    write
        .metadata_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let tuple_revision = permissions
        .write_relationships(write)
        .await
        .unwrap()
        .into_inner()
        .written_at
        .unwrap()
        .token;
    let mut accepted = source(&tuple_revision);
    accepted.accepted_composition_schema = changed;
    assert_eq!(
        reader
            .read_request_source(&tests::policy(), &accepted)
            .await
            .unwrap()
            .len(),
        1
    );
}
