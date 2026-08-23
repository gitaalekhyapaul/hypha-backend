use prost::Message;

use super::wire::{
    OneShotFailureClassWireV1, OneShotFailureWireV1, OneShotRequestWireV1, OneShotResultWireV1,
    OneShotSuccessWireV1, one_shot_result_wire_v1,
};
use super::{
    MAX_ONE_SHOT_REQUEST_BYTES, MAX_ONE_SHOT_RESULT_BYTES, ONE_SHOT_PROTOCOL_VERSION,
    OneShotFailure, OneShotFailureClass, OneShotMetrics, OneShotProtocolError, OneShotRequest,
    OneShotResult, OneShotSuccess, PROOF_DEADLINE_SECONDS,
};
use crate::private_envelope::PrivateCapabilityEnvelope;

pub fn encode_request(request: &OneShotRequest) -> Result<Vec<u8>, OneShotProtocolError> {
    let wire = OneShotRequestWireV1 {
        format_version: ONE_SHOT_PROTOCOL_VERSION,
        private_envelope: request
            .envelope
            .encode_transport()
            .map_err(|_| OneShotProtocolError::InvalidRequest)?,
        worker_start_unix_seconds: request.worker_start_unix_seconds,
        deadline_seconds: PROOF_DEADLINE_SECONDS,
        correlation_id: request.correlation_id.clone(),
    };
    encode_bounded(
        wire,
        MAX_ONE_SHOT_REQUEST_BYTES,
        OneShotProtocolError::RequestTooLarge,
    )
}

pub fn decode_request(bytes: &[u8]) -> Result<OneShotRequest, OneShotProtocolError> {
    let wire: OneShotRequestWireV1 = decode_bounded(
        bytes,
        MAX_ONE_SHOT_REQUEST_BYTES,
        OneShotProtocolError::RequestTooLarge,
        OneShotProtocolError::MalformedRequest,
    )?;
    if wire.format_version != ONE_SHOT_PROTOCOL_VERSION {
        return Err(OneShotProtocolError::UnsupportedVersion);
    }
    if wire.deadline_seconds != PROOF_DEADLINE_SECONDS {
        return Err(OneShotProtocolError::InvalidRequest);
    }
    let envelope = PrivateCapabilityEnvelope::decode_transport(&wire.private_envelope)
        .map_err(|_| OneShotProtocolError::InvalidRequest)?;
    OneShotRequest::new(
        envelope,
        wire.worker_start_unix_seconds,
        wire.correlation_id,
    )
}

pub fn encode_result(result: &OneShotResult) -> Result<Vec<u8>, OneShotProtocolError> {
    let outcome = match result {
        OneShotResult::Success(success) => {
            one_shot_result_wire_v1::Outcome::Success(OneShotSuccessWireV1 {
                proof_artifact: success.proof_artifact.clone(),
                public_values: success.public_values_bytes.clone(),
                verification_key_id: success.verification_key_id.clone(),
                execute_millis: success.metrics.execute_millis,
                prove_millis: success.metrics.prove_millis,
                artifact_bytes: success.metrics.artifact_bytes,
                peak_memory_bytes: success.metrics.peak_memory_bytes,
                peak_swap_bytes: success.metrics.peak_swap_bytes,
            })
        }
        OneShotResult::Failure(failure) => {
            one_shot_result_wire_v1::Outcome::Failure(OneShotFailureWireV1 {
                class: failure_class_to_wire(failure.class) as i32,
                correlation_id: failure.correlation_id.clone(),
            })
        }
    };
    encode_bounded(
        OneShotResultWireV1 {
            format_version: ONE_SHOT_PROTOCOL_VERSION,
            outcome: Some(outcome),
        },
        MAX_ONE_SHOT_RESULT_BYTES,
        OneShotProtocolError::ResultTooLarge,
    )
}

pub fn decode_result(bytes: &[u8]) -> Result<OneShotResult, OneShotProtocolError> {
    let wire: OneShotResultWireV1 = decode_bounded(
        bytes,
        MAX_ONE_SHOT_RESULT_BYTES,
        OneShotProtocolError::ResultTooLarge,
        OneShotProtocolError::MalformedResult,
    )?;
    if wire.format_version != ONE_SHOT_PROTOCOL_VERSION {
        return Err(OneShotProtocolError::UnsupportedVersion);
    }
    match wire.outcome.ok_or(OneShotProtocolError::InvalidResult)? {
        one_shot_result_wire_v1::Outcome::Success(success) => {
            Ok(OneShotResult::Success(Box::new(OneShotSuccess::new(
                success.proof_artifact,
                success.public_values,
                success.verification_key_id,
                OneShotMetrics {
                    execute_millis: success.execute_millis,
                    prove_millis: success.prove_millis,
                    artifact_bytes: success.artifact_bytes,
                    peak_memory_bytes: success.peak_memory_bytes,
                    peak_swap_bytes: success.peak_swap_bytes,
                },
            )?)))
        }
        one_shot_result_wire_v1::Outcome::Failure(failure) => {
            let class = OneShotFailureClassWireV1::try_from(failure.class)
                .map_err(|_| OneShotProtocolError::InvalidResult)?;
            Ok(OneShotResult::Failure(OneShotFailure::new(
                failure_class_from_wire(class)?,
                failure.correlation_id,
            )?))
        }
    }
}

fn encode_bounded<M: Message>(
    message: M,
    maximum: usize,
    too_large: OneShotProtocolError,
) -> Result<Vec<u8>, OneShotProtocolError> {
    if message.encoded_len() > maximum {
        return Err(too_large);
    }
    let bytes = message.encode_to_vec();
    if bytes.len() > maximum {
        return Err(too_large);
    }
    Ok(bytes)
}

fn decode_bounded<M: Message + Default>(
    bytes: &[u8],
    maximum: usize,
    too_large: OneShotProtocolError,
    malformed: OneShotProtocolError,
) -> Result<M, OneShotProtocolError> {
    if bytes.is_empty() {
        return Err(malformed);
    }
    if bytes.len() > maximum {
        return Err(too_large);
    }
    let message = M::decode(bytes).map_err(|_| malformed)?;
    if message.encode_to_vec() != bytes {
        return Err(malformed);
    }
    Ok(message)
}

const fn failure_class_to_wire(value: OneShotFailureClass) -> OneShotFailureClassWireV1 {
    match value {
        OneShotFailureClass::InvalidRequest => OneShotFailureClassWireV1::InvalidRequest,
        OneShotFailureClass::TrustedState => OneShotFailureClassWireV1::TrustedState,
        OneShotFailureClass::WitnessRejected => OneShotFailureClassWireV1::WitnessRejected,
        OneShotFailureClass::HostRejected => OneShotFailureClassWireV1::HostRejected,
        OneShotFailureClass::ProofFailed => OneShotFailureClassWireV1::ProofFailed,
        OneShotFailureClass::JournalMismatch => OneShotFailureClassWireV1::JournalMismatch,
        OneShotFailureClass::ResourceProfile => OneShotFailureClassWireV1::ResourceProfile,
        OneShotFailureClass::Cancelled => OneShotFailureClassWireV1::Cancelled,
        OneShotFailureClass::Internal => OneShotFailureClassWireV1::Internal,
    }
}

fn failure_class_from_wire(
    value: OneShotFailureClassWireV1,
) -> Result<OneShotFailureClass, OneShotProtocolError> {
    match value {
        OneShotFailureClassWireV1::Unspecified => Err(OneShotProtocolError::InvalidResult),
        OneShotFailureClassWireV1::InvalidRequest => Ok(OneShotFailureClass::InvalidRequest),
        OneShotFailureClassWireV1::TrustedState => Ok(OneShotFailureClass::TrustedState),
        OneShotFailureClassWireV1::WitnessRejected => Ok(OneShotFailureClass::WitnessRejected),
        OneShotFailureClassWireV1::HostRejected => Ok(OneShotFailureClass::HostRejected),
        OneShotFailureClassWireV1::ProofFailed => Ok(OneShotFailureClass::ProofFailed),
        OneShotFailureClassWireV1::JournalMismatch => Ok(OneShotFailureClass::JournalMismatch),
        OneShotFailureClassWireV1::ResourceProfile => Ok(OneShotFailureClass::ResourceProfile),
        OneShotFailureClassWireV1::Cancelled => Ok(OneShotFailureClass::Cancelled),
        OneShotFailureClassWireV1::Internal => Ok(OneShotFailureClass::Internal),
    }
}
