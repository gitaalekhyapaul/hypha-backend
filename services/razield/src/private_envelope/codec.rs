use prost::Message;
use raziel_authorization_core::{
    CapabilityRequestBindingV1, capability_request_commitment, validate_capability_request_binding,
};
use raziel_policy_ir::{CaveatValue, ContextEntry};

use super::wire::{
    PrivateCapabilityEnvelopeWireV1, PrivateContextEntryWireV1, private_context_entry_wire_v1,
};
use super::{
    MAX_PRIVATE_ENVELOPE_BYTES, PRIVATE_ENVELOPE_FORMAT_VERSION, PrivateCapabilityEnvelope,
    PrivateEnvelopeError, PrivateEnvelopeRef,
};
use crate::admission::{AdmittedCapabilityRequest, MAX_PROTOBUF_TIMESTAMP_SECONDS};

pub(super) fn encode(
    admitted: &AdmittedCapabilityRequest,
) -> Result<(PrivateEnvelopeRef, Vec<u8>), PrivateEnvelopeError> {
    let bytes = encode_parts(admitted.request(), admitted.capability_request_commitment())?;
    Ok((
        PrivateEnvelopeRef::from_commitment(*admitted.capability_request_commitment()),
        bytes,
    ))
}

pub(super) fn encode_validated(
    envelope: &PrivateCapabilityEnvelope,
) -> Result<Vec<u8>, PrivateEnvelopeError> {
    encode_parts(envelope.request(), envelope.capability_request_commitment())
}

fn encode_parts(
    request: &CapabilityRequestBindingV1,
    commitment: &[u8; 32],
) -> Result<Vec<u8>, PrivateEnvelopeError> {
    let wire = PrivateCapabilityEnvelopeWireV1 {
        envelope_format_version: PRIVATE_ENVELOPE_FORMAT_VERSION,
        request_format_version: request.format_version,
        agent_key_thumbprint: request.agent_key_thumbprint.to_vec(),
        task_id: request.task_id.clone(),
        purpose: request.purpose.clone(),
        action: request.action.clone(),
        resource_type: request.resource_type.clone(),
        resource_scope: request.resource_scope.clone(),
        requested_expires_at_unix_seconds: request.requested_expires_at_unix_seconds,
        issuance_nonce: request.issuance_nonce.to_vec(),
        snapshot_id: request.snapshot_id.clone(),
        request_context: request
            .request_context
            .iter()
            .map(context_to_wire)
            .collect(),
        capability_request_commitment: commitment.to_vec(),
    };

    if wire.encoded_len() > MAX_PRIVATE_ENVELOPE_BYTES {
        return Err(PrivateEnvelopeError::EnvelopeTooLarge);
    }
    let bytes = wire.encode_to_vec();
    if bytes.len() > MAX_PRIVATE_ENVELOPE_BYTES {
        return Err(PrivateEnvelopeError::EnvelopeTooLarge);
    }

    Ok(bytes)
}

pub(super) fn decode(
    reference: &PrivateEnvelopeRef,
    bytes: &[u8],
) -> Result<PrivateCapabilityEnvelope, PrivateEnvelopeError> {
    let envelope = decode_unbound(bytes)?;
    if reference.commitment() != envelope.capability_request_commitment() {
        return Err(PrivateEnvelopeError::CommitmentMismatch);
    }
    Ok(envelope)
}

pub(super) fn decode_unbound(
    bytes: &[u8],
) -> Result<PrivateCapabilityEnvelope, PrivateEnvelopeError> {
    if bytes.is_empty() {
        return Err(PrivateEnvelopeError::MalformedEnvelope);
    }
    if bytes.len() > MAX_PRIVATE_ENVELOPE_BYTES {
        return Err(PrivateEnvelopeError::EnvelopeTooLarge);
    }

    let wire = PrivateCapabilityEnvelopeWireV1::decode(bytes)
        .map_err(|_| PrivateEnvelopeError::MalformedEnvelope)?;
    if wire.encode_to_vec() != bytes {
        return Err(PrivateEnvelopeError::MalformedEnvelope);
    }
    if wire.envelope_format_version != PRIVATE_ENVELOPE_FORMAT_VERSION {
        return Err(PrivateEnvelopeError::UnsupportedEnvelopeVersion);
    }

    let request = request_from_wire(&wire)?;
    validate_capability_request_binding(&request)
        .map_err(|_| PrivateEnvelopeError::InvalidEnvelope)?;
    if !(0..=MAX_PROTOBUF_TIMESTAMP_SECONDS).contains(&request.requested_expires_at_unix_seconds) {
        return Err(PrivateEnvelopeError::InvalidEnvelope);
    }

    let stored_commitment: [u8; 32] = wire
        .capability_request_commitment
        .as_slice()
        .try_into()
        .map_err(|_| PrivateEnvelopeError::InvalidEnvelope)?;
    let recalculated = capability_request_commitment(&request)
        .map_err(|_| PrivateEnvelopeError::InvalidEnvelope)?;
    if stored_commitment != recalculated {
        return Err(PrivateEnvelopeError::CommitmentMismatch);
    }

    Ok(PrivateCapabilityEnvelope::new(request, recalculated))
}

fn request_from_wire(
    wire: &PrivateCapabilityEnvelopeWireV1,
) -> Result<CapabilityRequestBindingV1, PrivateEnvelopeError> {
    let agent_key_thumbprint = wire
        .agent_key_thumbprint
        .as_slice()
        .try_into()
        .map_err(|_| PrivateEnvelopeError::InvalidEnvelope)?;
    let issuance_nonce = wire
        .issuance_nonce
        .as_slice()
        .try_into()
        .map_err(|_| PrivateEnvelopeError::InvalidEnvelope)?;
    let request_context = wire
        .request_context
        .iter()
        .map(context_from_wire)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CapabilityRequestBindingV1 {
        format_version: wire.request_format_version,
        agent_key_thumbprint,
        task_id: wire.task_id.clone(),
        purpose: wire.purpose.clone(),
        action: wire.action.clone(),
        resource_type: wire.resource_type.clone(),
        resource_scope: wire.resource_scope.clone(),
        requested_expires_at_unix_seconds: wire.requested_expires_at_unix_seconds,
        issuance_nonce,
        snapshot_id: wire.snapshot_id.clone(),
        request_context,
    })
}

fn context_to_wire(entry: &ContextEntry) -> PrivateContextEntryWireV1 {
    let value = match &entry.value {
        CaveatValue::Bool(value) => private_context_entry_wire_v1::Value::Bool(*value),
        CaveatValue::Int(value) => private_context_entry_wire_v1::Value::Int(*value),
        CaveatValue::String(value) => private_context_entry_wire_v1::Value::String(value.clone()),
    };
    PrivateContextEntryWireV1 {
        name: entry.name.clone(),
        value: Some(value),
    }
}

fn context_from_wire(
    entry: &PrivateContextEntryWireV1,
) -> Result<ContextEntry, PrivateEnvelopeError> {
    let value = match entry
        .value
        .as_ref()
        .ok_or(PrivateEnvelopeError::InvalidEnvelope)?
    {
        private_context_entry_wire_v1::Value::Bool(value) => CaveatValue::Bool(*value),
        private_context_entry_wire_v1::Value::Int(value) => CaveatValue::Int(*value),
        private_context_entry_wire_v1::Value::String(value) => CaveatValue::String(value.clone()),
    };
    Ok(ContextEntry {
        name: entry.name.clone(),
        value,
    })
}
