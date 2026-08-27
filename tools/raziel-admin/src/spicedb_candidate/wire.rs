use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RelationshipBatchInput {
    pub format_version: u32,
    pub candidate_id: String,
    pub relationships: Vec<RelationshipInput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RelationshipInput {
    pub resource_type: String,
    pub resource_id: String,
    pub relation: String,
    pub subject_type: String,
    pub subject_id: String,
    #[serde(default)]
    pub subject_relation: Option<String>,
    #[serde(default)]
    pub caveat: Option<StoredCaveatInput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredCaveatInput {
    pub name: String,
    pub context: Vec<StoredContextInput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredContextInput {
    pub name: String,
    pub value: StoredValueInput,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub(super) enum StoredValueInput {
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SpiceDbRevisionHandoffWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(string, tag = "3")]
    pub policy_record_id: String,
    #[prost(bytes = "vec", tag = "4")]
    pub policy_hash: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    pub source_schema_digest: Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub relationship_batch_digest: Vec<u8>,
    #[prost(uint32, tag = "7")]
    pub accepted_relationship_count: u32,
    #[prost(string, tag = "8")]
    pub zed_token: String,
}
