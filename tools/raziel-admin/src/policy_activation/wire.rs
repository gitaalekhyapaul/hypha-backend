use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(crate) struct PolicyActivationRequestWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(string, tag = "3")]
    pub approved_by: String,
    #[prost(enumeration = "PolicyOperationWire", tag = "4")]
    pub operation: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(crate) enum PolicyOperationWire {
    Activate = 0,
    ListCatalogue = 1,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyActivationProjectionWire {
    #[prost(string, tag = "1")]
    pub record_id: String,
    #[prost(string, tag = "2")]
    pub policy_name: String,
    #[prost(string, tag = "3")]
    pub policy_version: String,
    #[prost(string, tag = "4")]
    pub policy_hash: String,
    #[prost(uint64, tag = "5")]
    pub approval_epoch: u64,
    #[prost(int64, tag = "6")]
    pub registered_at_unix_seconds: i64,
    #[prost(int64, tag = "7")]
    pub valid_from_unix_seconds: i64,
    #[prost(int64, tag = "8")]
    pub valid_until_unix_seconds: i64,
    #[prost(uint64, tag = "9")]
    pub registry_generation: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyActivationResponseWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(enumeration = "PolicyActivationFailureWire", tag = "2")]
    pub failure: i32,
    #[prost(message, optional, tag = "3")]
    pub projection: Option<PolicyActivationProjectionWire>,
    #[prost(message, optional, tag = "4")]
    pub catalogue: Option<PolicyCatalogueWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyCatalogueWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(message, repeated, tag = "2")]
    pub policies: Vec<PolicyCatalogueEntryWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyCatalogueEntryWire {
    #[prost(string, tag = "1")]
    pub family_id: String,
    #[prost(string, tag = "2")]
    pub version_id: String,
    #[prost(uint32, tag = "3")]
    pub version: u32,
    #[prost(string, tag = "4")]
    pub record_id: String,
    #[prost(string, tag = "5")]
    pub policy_hash: String,
    #[prost(string, tag = "6")]
    pub policy_name: String,
    #[prost(string, tag = "7")]
    pub policy_version: String,
    #[prost(string, tag = "8")]
    pub resource_type: String,
    #[prost(string, tag = "9")]
    pub permission: String,
    #[prost(uint64, tag = "10")]
    pub approval_epoch: u64,
    #[prost(int64, tag = "11")]
    pub valid_from_unix_seconds: i64,
    #[prost(int64, tag = "12")]
    pub valid_until_unix_seconds: i64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PolicyActivationEnvelopeWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub candidate_id: String,
    #[prost(bytes = "vec", tag = "3")]
    pub approval_nonce: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(super) enum PolicyActivationFailureWire {
    Unspecified = 0,
    InvalidRequest = 1,
    Conflict = 2,
    CandidateRejected = 3,
    Unavailable = 4,
}
