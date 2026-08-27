use prost::Message;
use raziel_proto::raziel::v1 as proto;

#[derive(Clone, PartialEq, Message)]
pub struct PolicyCandidateManifestWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(string, tag = "3")]
    pub record_id: String,
    #[prost(string, tag = "4")]
    pub policy_name: String,
    #[prost(string, tag = "5")]
    pub policy_version: String,
    #[prost(string, tag = "6")]
    pub resource_type: String,
    #[prost(string, tag = "7")]
    pub permission: String,
    #[prost(string, tag = "8")]
    pub rule_summary: String,
    #[prost(bytes = "vec", tag = "9")]
    pub source_schema_digest: Vec<u8>,
    #[prost(bytes = "vec", tag = "10")]
    pub policy_hash: Vec<u8>,
    #[prost(uint64, tag = "11")]
    pub expected_approval_epoch: u64,
    #[prost(int64, tag = "12")]
    pub valid_from_unix_seconds: i64,
    #[prost(int64, tag = "13")]
    pub valid_until_unix_seconds: i64,
    #[prost(int64, tag = "14")]
    pub created_at_unix_seconds: i64,
    #[prost(int64, tag = "15")]
    pub expires_at_unix_seconds: i64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyAuthorityRequestWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(bytes = "vec", tag = "3")]
    pub expected_policy_hash: Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub expected_approval_epoch: u64,
    #[prost(int64, tag = "5")]
    pub valid_from_unix_seconds: i64,
    #[prost(int64, tag = "6")]
    pub valid_until_unix_seconds: i64,
    #[prost(string, tag = "7")]
    pub approved_by: String,
    #[prost(bytes = "vec", tag = "8")]
    pub approval_nonce: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct StoredPolicyApprovalWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub approval_nonce_digest: Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub staged_policy_record: Option<proto::PolicyRecord>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyAuthorityResponseWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(enumeration = "PolicyAuthorityFailureWire", tag = "2")]
    pub failure: i32,
    #[prost(message, optional, tag = "3")]
    pub staged_policy_record: Option<proto::PolicyRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(super) enum PolicyAuthorityFailureWire {
    Unspecified = 0,
    InvalidRequest = 1,
    StaleEpoch = 2,
    Conflict = 3,
    CandidateRejected = 4,
    Unavailable = 5,
}
