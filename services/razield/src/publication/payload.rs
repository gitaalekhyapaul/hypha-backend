use std::fmt::{self, Debug, Formatter};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::private_envelope::{PrivateEnvelopeError, PrivateEnvelopeRef};

/// Version of the public BullMQ handoff payload.
pub const CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION: u32 = 1;

/// Strict queue payload that points the proof worker at owner-only storage.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityJobPayload {
    private_envelope_ref: PrivateEnvelopeRef,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilityJobPayloadWire {
    format_version: u32,
    private_envelope_ref: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PayloadError {
    Malformed,
    UnsupportedVersion,
    InvalidReference,
}

impl CapabilityJobPayload {
    pub(super) const fn new(private_envelope_ref: PrivateEnvelopeRef) -> Self {
        Self {
            private_envelope_ref,
        }
    }

    /// Returns the queue payload version checked by the proof worker.
    pub const fn format_version(&self) -> u32 {
        CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION
    }

    /// Returns the opaque storage reference. It is never a filesystem path.
    pub const fn private_envelope_ref(&self) -> PrivateEnvelopeRef {
        self.private_envelope_ref
    }

    pub(super) fn to_json_value(&self) -> Result<Value, PayloadError> {
        serde_json::to_value(CapabilityJobPayloadWire {
            format_version: CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION,
            private_envelope_ref: self.private_envelope_ref.queue_token(),
        })
        .map_err(|_| PayloadError::Malformed)
    }

    pub(super) fn from_json_value(value: &Value) -> Result<Self, PayloadError> {
        let wire: CapabilityJobPayloadWire =
            serde_json::from_value(value.clone()).map_err(|_| PayloadError::Malformed)?;
        if wire.format_version != CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION {
            return Err(PayloadError::UnsupportedVersion);
        }
        let private_envelope_ref = PrivateEnvelopeRef::from_queue_token(&wire.private_envelope_ref)
            .map_err(map_reference_error)?;
        Ok(Self {
            private_envelope_ref,
        })
    }
}

impl Debug for CapabilityJobPayload {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityJobPayload")
            .field("format_version", &CAPABILITY_JOB_PAYLOAD_FORMAT_VERSION)
            .field("private_envelope_ref", &"<redacted>")
            .finish()
    }
}

const fn map_reference_error(_error: PrivateEnvelopeError) -> PayloadError {
    PayloadError::InvalidReference
}
