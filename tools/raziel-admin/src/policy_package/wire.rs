use prost::Message;

// Private storage only. These messages do not change a wire or proof contract.
#[derive(Clone, PartialEq, Message)]
pub(super) struct StoredPackage {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub family_id: String,
    #[prost(string, tag = "3")]
    pub version_id: String,
    #[prost(uint32, tag = "4")]
    pub version: u32,
    #[prost(bytes = "vec", tag = "5")]
    pub candidate_manifest: Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub source_schema: Vec<u8>,
    #[prost(bytes = "vec", tag = "7")]
    pub compiler_config: Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub policy_pb: Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub approval_record: Vec<u8>,
    #[prost(string, repeated, tag = "10")]
    pub definitions: Vec<String>,
    #[prost(string, repeated, tag = "11")]
    pub caveats: Vec<String>,
}
