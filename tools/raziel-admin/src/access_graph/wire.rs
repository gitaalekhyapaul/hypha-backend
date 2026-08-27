use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct AccessGraphRequestWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(string, tag = "3")]
    pub operator_id: String,
    #[prost(uint32, tag = "4")]
    pub valid_for_seconds: u32,
    #[prost(message, repeated, tag = "5")]
    pub relationships: Vec<RelationshipWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RelationshipWire {
    #[prost(string, tag = "1")]
    pub resource_type: String,
    #[prost(string, tag = "2")]
    pub resource_id: String,
    #[prost(string, tag = "3")]
    pub relation: String,
    #[prost(string, tag = "4")]
    pub subject_type: String,
    #[prost(string, tag = "5")]
    pub subject_id: String,
    #[prost(string, optional, tag = "6")]
    pub subject_relation: Option<String>,
    #[prost(message, optional, tag = "7")]
    pub caveat: Option<StoredCaveatWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct StoredCaveatWire {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(message, repeated, tag = "2")]
    pub context: Vec<StoredContextWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct StoredContextWire {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(oneof = "stored_context_wire::Value", tags = "2, 3, 4")]
    pub value: Option<stored_context_wire::Value>,
}

pub(super) mod stored_context_wire {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Value {
        #[prost(bool, tag = "2")]
        Bool(bool),
        #[prost(int64, tag = "3")]
        Int(i64),
        #[prost(string, tag = "4")]
        String(String),
    }
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct AccessGraphProjectionWire {
    #[prost(string, tag = "1")]
    pub policy_record_id: String,
    #[prost(string, tag = "2")]
    pub policy_hash: String,
    #[prost(string, tag = "3")]
    pub snapshot_id: String,
    #[prost(string, tag = "4")]
    pub relationship_root: String,
    #[prost(uint64, tag = "5")]
    pub epoch: u64,
    #[prost(int64, tag = "6")]
    pub issued_at_unix_seconds: i64,
    #[prost(int64, tag = "7")]
    pub expires_at_unix_seconds: i64,
    #[prost(uint32, tag = "8")]
    pub accepted_relationship_count: u32,
    #[prost(uint64, tag = "9")]
    pub registry_generation: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct AccessGraphResponseWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(enumeration = "AccessGraphFailureWire", tag = "2")]
    pub failure: i32,
    #[prost(message, optional, tag = "3")]
    pub projection: Option<AccessGraphProjectionWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct AccessGraphStateWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(bytes = "vec", tag = "3")]
    pub request_digest: Vec<u8>,
    #[prost(message, optional, tag = "4")]
    pub projection: Option<AccessGraphProjectionWire>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(super) enum AccessGraphFailureWire {
    Unspecified = 0,
    InvalidRequest = 1,
    Conflict = 2,
    CandidateRejected = 3,
    Unavailable = 4,
}
