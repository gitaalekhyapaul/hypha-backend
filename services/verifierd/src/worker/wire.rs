use std::fmt::{self, Debug, Formatter};

use prost::{Enumeration, Message, Oneof};

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct VerificationWorkerRequestWireV1 {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub proof_artifact: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub expected_public_values: Vec<u8>,
    #[prost(string, tag = "4")]
    pub verification_key_id: String,
    #[prost(string, tag = "5")]
    pub sp1_vkey_hash: String,
    #[prost(uint32, tag = "6")]
    pub deadline_seconds: u32,
    #[prost(string, tag = "7")]
    pub correlation_id: String,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct VerificationWorkerResultWireV1 {
    #[prost(uint32, tag = "1")]
    pub format_version: u32,
    #[prost(oneof = "verification_worker_result_wire_v1::Outcome", tags = "2, 3")]
    pub outcome: Option<verification_worker_result_wire_v1::Outcome>,
}

pub(super) mod verification_worker_result_wire_v1 {
    use super::{Oneof, VerificationWorkerFailureWireV1, VerificationWorkerSuccessWireV1};

    #[derive(Clone, PartialEq, Oneof)]
    #[prost(skip_debug)]
    pub enum Outcome {
        #[prost(message, tag = "2")]
        Success(VerificationWorkerSuccessWireV1),
        #[prost(message, tag = "3")]
        Failure(VerificationWorkerFailureWireV1),
    }
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct VerificationWorkerSuccessWireV1 {
    #[prost(string, tag = "1")]
    pub correlation_id: String,
    #[prost(string, tag = "2")]
    pub verification_key_id: String,
}

#[derive(Clone, PartialEq, Message)]
#[prost(skip_debug)]
pub(super) struct VerificationWorkerFailureWireV1 {
    #[prost(enumeration = "VerificationWorkerFailureClassWireV1", tag = "1")]
    pub class: i32,
    #[prost(string, tag = "2")]
    pub correlation_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
#[repr(i32)]
pub(super) enum VerificationWorkerFailureClassWireV1 {
    Unspecified = 0,
    InvalidRequest = 1,
    ProofRejected = 2,
    ResourceProfile = 3,
    Cancelled = 4,
    Internal = 5,
}

macro_rules! redacted_debug {
    ($type_name:ty, $name:literal) => {
        impl Debug for $type_name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct($name)
                    .field("public_fields", &"<redacted>")
                    .finish()
            }
        }
    };
}

redacted_debug!(
    VerificationWorkerRequestWireV1,
    "VerificationWorkerRequestWireV1"
);
redacted_debug!(
    VerificationWorkerResultWireV1,
    "VerificationWorkerResultWireV1"
);
redacted_debug!(
    VerificationWorkerSuccessWireV1,
    "VerificationWorkerSuccessWireV1"
);
redacted_debug!(
    VerificationWorkerFailureWireV1,
    "VerificationWorkerFailureWireV1"
);

impl Debug for verification_worker_result_wire_v1::Outcome {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}
