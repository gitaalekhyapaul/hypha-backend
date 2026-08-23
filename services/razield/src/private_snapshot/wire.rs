//! Rust-only Protobuf model for private relationship snapshots.

use std::fmt::{self, Debug, Formatter};

use prost::{Message, Oneof};

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct PrivateRelationshipSnapshotWireV1 {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(string, tag = "2")]
    pub snapshot_id: String,
    #[prost(bytes = "vec", tag = "3")]
    pub relationship_root: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub snapshot_secret_salt: Vec<u8>,
    #[prost(message, repeated, tag = "5")]
    pub relationships: Vec<PrivateRelationshipWireV1>,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct PrivateRelationshipWireV1 {
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
    pub stored_caveat: Option<PrivateStoredCaveatWireV1>,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct PrivateStoredCaveatWireV1 {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(message, repeated, tag = "2")]
    pub context: Vec<PrivateStoredContextEntryWireV1>,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct PrivateStoredContextEntryWireV1 {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(
        oneof = "private_stored_context_entry_wire_v1::Value",
        tags = "2, 3, 4"
    )]
    pub value: Option<private_stored_context_entry_wire_v1::Value>,
}

pub(super) mod private_stored_context_entry_wire_v1 {
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

macro_rules! redacted_debug {
    ($type_name:ty, $name:literal) => {
        impl Debug for $type_name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct($name)
                    .field("private_fields", &"<redacted>")
                    .finish()
            }
        }
    };
}

redacted_debug!(
    PrivateRelationshipSnapshotWireV1,
    "PrivateRelationshipSnapshotWireV1"
);
redacted_debug!(PrivateRelationshipWireV1, "PrivateRelationshipWireV1");
redacted_debug!(PrivateStoredCaveatWireV1, "PrivateStoredCaveatWireV1");
redacted_debug!(
    PrivateStoredContextEntryWireV1,
    "PrivateStoredContextEntryWireV1"
);

impl Debug for private_stored_context_entry_wire_v1::Value {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}
