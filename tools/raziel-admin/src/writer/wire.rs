use prost::{Message, Oneof};
use raziel_proto::raziel::v1 as proto;

#[derive(Clone, PartialEq, Message)]
pub(super) struct RegistryWriterRequestWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(uint64, tag = "2")]
    pub expected_registry_generation: u64,
    #[prost(oneof = "registry_writer_request_wire::Operation", tags = "3, 4, 5")]
    pub operation: Option<registry_writer_request_wire::Operation>,
}

pub(super) mod registry_writer_request_wire {
    use super::{Oneof, RevokeSnapshotWire, proto};

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Operation {
        #[prost(message, boxed, tag = "3")]
        AppendSnapshot(Box<proto::RelationshipSnapshotDescriptor>),
        #[prost(message, tag = "4")]
        RevokeSnapshot(RevokeSnapshotWire),
        #[prost(message, boxed, tag = "5")]
        AppendApprovedPolicy(Box<proto::PolicyRecord>),
    }
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RevokeSnapshotWire {
    #[prost(string, tag = "1")]
    pub snapshot_id: String,
    #[prost(string, tag = "2")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RegistryWriterResponseWire {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(enumeration = "RegistryWriterFailureWire", tag = "2")]
    pub failure: i32,
    #[prost(uint64, tag = "3")]
    pub registry_generation: u64,
    #[prost(message, optional, tag = "4")]
    pub root_record: Option<proto::AcceptedRootRecord>,
    #[prost(message, optional, tag = "5")]
    pub policy_record: Option<proto::PolicyRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(super) enum RegistryWriterFailureWire {
    Unspecified = 0,
    StaleGeneration = 1,
    Conflict = 2,
    InvalidRequest = 3,
    RegistryRejected = 4,
    PublicationFailed = 5,
}
