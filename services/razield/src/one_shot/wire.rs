//! Rust-only Protobuf model for the local one-shot process channel.

use std::fmt::{self, Debug, Formatter};

use prost::{Enumeration, Message, Oneof};

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct OneShotRequestWireV1 {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub private_envelope: Vec<u8>,
    #[prost(int64, tag = "3")]
    pub worker_start_unix_seconds: i64,
    #[prost(uint32, tag = "4")]
    pub deadline_seconds: u32,
    #[prost(string, tag = "5")]
    pub correlation_id: String,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct OneShotResultWireV1 {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(oneof = "one_shot_result_wire_v1::Outcome", tags = "2, 3")]
    pub outcome: Option<one_shot_result_wire_v1::Outcome>,
}

pub(super) mod one_shot_result_wire_v1 {
    use super::{OneShotFailureWireV1, OneShotSuccessWireV1, Oneof};

    #[derive(Clone, PartialEq, Oneof)]
    #[prost(skip_debug)]
    pub enum Outcome {
        #[prost(message, tag = "2")]
        Success(OneShotSuccessWireV1),
        #[prost(message, tag = "3")]
        Failure(OneShotFailureWireV1),
    }
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct OneShotSuccessWireV1 {
    #[prost(bytes = "vec", tag = "1")]
    pub proof_artifact: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub public_values: Vec<u8>,
    #[prost(string, tag = "3")]
    pub verification_key_id: String,
    #[prost(uint64, tag = "4")]
    pub execute_millis: u64,
    #[prost(uint64, tag = "5")]
    pub prove_millis: u64,
    #[prost(uint64, tag = "6")]
    pub artifact_bytes: u64,
    #[prost(uint64, tag = "7")]
    pub peak_memory_bytes: u64,
    #[prost(uint64, tag = "8")]
    pub peak_swap_bytes: u64,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct OneShotFailureWireV1 {
    #[prost(enumeration = "OneShotFailureClassWireV1", tag = "1")]
    pub class: i32,
    #[prost(string, tag = "2")]
    pub correlation_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
#[repr(i32)]
pub(super) enum OneShotFailureClassWireV1 {
    Unspecified = 0,
    InvalidRequest = 1,
    TrustedState = 2,
    WitnessRejected = 3,
    HostRejected = 4,
    ProofFailed = 5,
    JournalMismatch = 6,
    ResourceProfile = 7,
    Cancelled = 8,
    Internal = 9,
}

macro_rules! redacted_debug {
    ($type_name:ty, $name:literal) => {
        impl Debug for $type_name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct($name)
                    .field("fields", &"<redacted>")
                    .finish()
            }
        }
    };
}

redacted_debug!(OneShotRequestWireV1, "OneShotRequestWireV1");
redacted_debug!(OneShotResultWireV1, "OneShotResultWireV1");
redacted_debug!(OneShotSuccessWireV1, "OneShotSuccessWireV1");
redacted_debug!(OneShotFailureWireV1, "OneShotFailureWireV1");

impl Debug for one_shot_result_wire_v1::Outcome {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}
