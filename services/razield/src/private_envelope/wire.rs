//! Rust-only Protobuf model for the private store.

use std::fmt::{self, Debug, Formatter};

use prost::{Message, Oneof};

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct PrivateCapabilityEnvelopeWireV1 {
    #[prost(uint32, tag = "1")]
    pub envelope_format_version: u32,
    #[prost(uint32, tag = "2")]
    pub request_format_version: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub agent_key_thumbprint: Vec<u8>,
    #[prost(string, tag = "4")]
    pub task_id: String,
    #[prost(string, tag = "5")]
    pub purpose: String,
    #[prost(string, tag = "6")]
    pub action: String,
    #[prost(string, tag = "7")]
    pub resource_type: String,
    #[prost(string, tag = "8")]
    pub resource_scope: String,
    #[prost(int64, tag = "9")]
    pub requested_expires_at_unix_seconds: i64,
    #[prost(bytes = "vec", tag = "10")]
    pub issuance_nonce: Vec<u8>,
    #[prost(string, tag = "11")]
    pub snapshot_id: String,
    #[prost(message, repeated, tag = "12")]
    pub request_context: Vec<PrivateContextEntryWireV1>,
    #[prost(bytes = "vec", tag = "13")]
    pub capability_request_commitment: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct PrivateContextEntryWireV1 {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(oneof = "private_context_entry_wire_v1::Value", tags = "2, 3, 4")]
    pub value: Option<private_context_entry_wire_v1::Value>,
}

pub(super) mod private_context_entry_wire_v1 {
    use super::Oneof;

    #[derive(Clone, PartialEq, Oneof)]
    #[prost(skip_debug)]
    pub enum Value {
        #[prost(bool, tag = "2")]
        Bool(bool),
        #[prost(sint64, tag = "3")]
        Int(i64),
        #[prost(string, tag = "4")]
        String(String),
    }
}

impl Debug for PrivateCapabilityEnvelopeWireV1 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateCapabilityEnvelopeWireV1")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

impl Debug for PrivateContextEntryWireV1 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateContextEntryWireV1")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

impl Debug for private_context_entry_wire_v1::Value {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}
