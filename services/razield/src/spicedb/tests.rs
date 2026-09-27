use std::collections::{BTreeMap, VecDeque};

use prost_types::{Struct, Timestamp, Value, value::Kind};
use raziel_authorization_core::TRUSTED_POLICY_CEILINGS;
use raziel_policy_ir::{
    AllowedSubjectType, ObjectDefinition, PermissionDefinition, PermissionExpr, PolicyProgram,
    RelationDefinition, ValidatedPolicyProgram,
};
use spicedb_rs_proto::authzed::api::v1 as authzed;

use super::conversion::relationship_from_api;
use super::{SpiceDbReadApi, SpiceDbSnapshotError, read_exact};

struct FakeApi {
    reflected_token: String,
    streams: VecDeque<Vec<authzed::ReadRelationshipsResponse>>,
    reflected_requests: Vec<authzed::ReflectSchemaRequest>,
    read_requests: Vec<authzed::ReadRelationshipsRequest>,
}

#[tonic::async_trait]
impl SpiceDbReadApi for FakeApi {
    async fn diff_schema(
        &mut self,
        _: authzed::DiffSchemaRequest,
    ) -> Result<authzed::DiffSchemaResponse, SpiceDbSnapshotError> {
        panic!("legacy M5 reader must not require composition comparison")
    }
    async fn reflect_schema(
        &mut self,
        request: authzed::ReflectSchemaRequest,
    ) -> Result<authzed::ReflectSchemaResponse, SpiceDbSnapshotError> {
        self.reflected_requests.push(request);
        Ok(authzed::ReflectSchemaResponse {
            definitions: Vec::new(),
            caveats: Vec::new(),
            read_at: Some(token(&self.reflected_token)),
        })
    }

    async fn read_relationships(
        &mut self,
        request: authzed::ReadRelationshipsRequest,
    ) -> Result<Vec<authzed::ReadRelationshipsResponse>, SpiceDbSnapshotError> {
        self.read_requests.push(request);
        Ok(self.streams.pop_front().unwrap_or_default())
    }
}

#[tokio::test]
async fn exact_reader_reflects_then_reads_each_sorted_definition() {
    let mut api = fake_api("zed-1");
    api.streams.push_back(vec![response(
        "zed-1",
        relationship("dataset", "b", "viewer", "user", "maya"),
    )]);
    api.streams.push_back(Vec::new());

    let relationships = read_exact(&mut api, &policy(), "zed-1").await.unwrap();

    assert_eq!(relationships.len(), 1);
    assert_eq!(api.reflected_requests.len(), 1);
    assert_eq!(api.read_requests.len(), 2);
    assert_eq!(
        api.read_requests[0]
            .relationship_filter
            .as_ref()
            .unwrap()
            .resource_type,
        "dataset"
    );
    assert_eq!(
        api.read_requests[1]
            .relationship_filter
            .as_ref()
            .unwrap()
            .resource_type,
        "user"
    );
    assert_eq!(api.read_requests[0].optional_limit, 257);
    assert_eq!(api.read_requests[1].optional_limit, 256);
}

#[tokio::test]
async fn exact_reader_rejects_revision_drift_and_aggregate_record_257() {
    let mut drifted = fake_api("other");
    assert_eq!(
        read_exact(&mut drifted, &policy(), "zed-1").await,
        Err(SpiceDbSnapshotError::RevisionMismatch)
    );

    let mut oversized = fake_api("zed-1");
    oversized.streams.push_back(
        (0..257)
            .map(|index| {
                response(
                    "zed-1",
                    relationship("dataset", &format!("d{index:03}"), "viewer", "user", "maya"),
                )
            })
            .collect(),
    );
    assert_eq!(
        read_exact(&mut oversized, &policy(), "zed-1").await,
        Err(SpiceDbSnapshotError::RelationshipLimitExceeded)
    );
}

#[tokio::test]
async fn exact_reader_accepts_the_aggregate_record_ceiling() {
    let mut api = fake_api("zed-1");
    api.streams.push_back(
        (0..256)
            .map(|index| {
                response(
                    "zed-1",
                    relationship("dataset", &format!("d{index:03}"), "viewer", "user", "maya"),
                )
            })
            .collect(),
    );

    let relationships = read_exact(&mut api, &policy(), "zed-1").await.unwrap();

    assert_eq!(relationships.len(), 256);
    assert_eq!(api.read_requests[1].optional_limit, 1);
}

#[tokio::test]
async fn exact_reader_sorts_and_rejects_duplicate_identities() {
    let mut api = fake_api("zed-1");
    api.streams.push_back(vec![
        response(
            "zed-1",
            relationship("dataset", "z", "viewer", "user", "maya"),
        ),
        response(
            "zed-1",
            relationship("dataset", "a", "viewer", "user", "maya"),
        ),
    ]);
    let sorted = read_exact(&mut api, &policy(), "zed-1").await.unwrap();
    assert_eq!(sorted[0].resource_id, "a");
    assert_eq!(sorted[1].resource_id, "z");

    let duplicate = relationship("dataset", "same", "viewer", "user", "maya");
    let mut api = fake_api("zed-1");
    api.streams.push_back(vec![
        response("zed-1", duplicate.clone()),
        response("zed-1", duplicate),
    ]);
    assert_eq!(
        read_exact(&mut api, &policy(), "zed-1").await,
        Err(SpiceDbSnapshotError::DuplicateRelationship)
    );
}

#[test]
fn conversion_accepts_only_supported_exact_caveat_scalars() {
    let mut value = relationship("dataset", "sg", "viewer", "user", "maya");
    value.optional_caveat = Some(authzed::ContextualizedCaveat {
        caveat_name: "approved_context".into(),
        context: Some(Struct {
            fields: BTreeMap::from([
                ("approved".into(), scalar(Kind::BoolValue(true))),
                ("batch".into(), scalar(Kind::NumberValue(42.0))),
                ("region".into(), scalar(Kind::StringValue("sg".into()))),
            ]),
        }),
    });
    let converted = relationship_from_api(&value).unwrap();
    let context = &converted.stored_caveat.unwrap().context;
    assert_eq!(context[0].name, "approved");
    assert_eq!(context[1].name, "batch");
    assert_eq!(context[2].name, "region");

    for unsupported in [
        Kind::NumberValue(1.5),
        Kind::NumberValue(f64::NAN),
        Kind::NumberValue(9_007_199_254_740_992.0),
        Kind::NullValue(0),
        Kind::ListValue(prost_types::ListValue { values: Vec::new() }),
        Kind::StructValue(Struct::default()),
    ] {
        let mut invalid = value.clone();
        invalid.optional_caveat.as_mut().unwrap().context = Some(Struct {
            fields: BTreeMap::from([("bad".into(), scalar(unsupported))]),
        });
        assert_eq!(
            relationship_from_api(&invalid),
            Err(SpiceDbSnapshotError::UnsupportedCaveatValue)
        );
    }

    value.optional_caveat.as_mut().unwrap().context = Some(Struct {
        fields: BTreeMap::from([("bad".into(), Value { kind: None })]),
    });
    assert_eq!(
        relationship_from_api(&value),
        Err(SpiceDbSnapshotError::UnsupportedCaveatValue)
    );
}

#[test]
fn conversion_rejects_wildcards_and_expirations() {
    let wildcard = relationship("dataset", "sg", "viewer", "user", "*");
    assert_eq!(
        relationship_from_api(&wildcard),
        Err(SpiceDbSnapshotError::UnsupportedWildcard)
    );

    let mut expiring = relationship("dataset", "sg", "viewer", "user", "maya");
    expiring.optional_expires_at = Some(Timestamp {
        seconds: 1_700_000_000,
        nanos: 0,
    });
    assert_eq!(
        relationship_from_api(&expiring),
        Err(SpiceDbSnapshotError::UnsupportedExpiration)
    );

    let mut missing_resource = relationship("dataset", "sg", "viewer", "user", "maya");
    missing_resource.resource = None;
    assert_eq!(
        relationship_from_api(&missing_resource),
        Err(SpiceDbSnapshotError::MalformedRelationship)
    );

    let mut missing_subject = relationship("dataset", "sg", "viewer", "user", "maya");
    missing_subject.subject = None;
    assert_eq!(
        relationship_from_api(&missing_subject),
        Err(SpiceDbSnapshotError::MalformedRelationship)
    );
}

fn fake_api(reflected_token: &str) -> FakeApi {
    FakeApi {
        reflected_token: reflected_token.into(),
        streams: VecDeque::new(),
        reflected_requests: Vec::new(),
        read_requests: Vec::new(),
    }
}

fn response(
    revision: &str,
    relationship: authzed::Relationship,
) -> authzed::ReadRelationshipsResponse {
    authzed::ReadRelationshipsResponse {
        read_at: Some(token(revision)),
        relationship: Some(relationship),
        after_result_cursor: None,
    }
}

fn token(value: &str) -> authzed::ZedToken {
    authzed::ZedToken {
        token: value.into(),
    }
}

fn relationship(
    resource_type: &str,
    resource_id: &str,
    relation: &str,
    subject_type: &str,
    subject_id: &str,
) -> authzed::Relationship {
    authzed::Relationship {
        resource: Some(authzed::ObjectReference {
            object_type: resource_type.into(),
            object_id: resource_id.into(),
        }),
        relation: relation.into(),
        subject: Some(authzed::SubjectReference {
            object: Some(authzed::ObjectReference {
                object_type: subject_type.into(),
                object_id: subject_id.into(),
            }),
            optional_relation: String::new(),
        }),
        optional_caveat: None,
        optional_expires_at: None,
    }
}

fn scalar(kind: Kind) -> Value {
    Value { kind: Some(kind) }
}

pub(super) fn policy() -> ValidatedPolicyProgram {
    ValidatedPolicyProgram::new(
        PolicyProgram {
            ir_version: 1,
            compiler_name: "test".into(),
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
