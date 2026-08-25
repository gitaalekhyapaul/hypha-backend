use prost::Message;
use raziel_prover::TrustedProgramKey;

use super::wire::{
    VerificationWorkerFailureClassWireV1, VerificationWorkerFailureWireV1,
    VerificationWorkerRequestWireV1, VerificationWorkerResultWireV1,
    VerificationWorkerSuccessWireV1, verification_worker_result_wire_v1,
};
use super::{
    IsolatedVerificationRequest, MAX_VERIFICATION_WORKER_REQUEST_BYTES,
    MAX_VERIFICATION_WORKER_RESULT_BYTES, VERIFICATION_DEADLINE_SECONDS,
    VERIFICATION_WORKER_PROTOCOL_VERSION, VerificationWorkerFailure,
    VerificationWorkerFailureClass, VerificationWorkerProtocolError, VerificationWorkerResult,
    VerificationWorkerSuccess,
};

pub fn encode_request(
    request: &IsolatedVerificationRequest,
) -> Result<Vec<u8>, VerificationWorkerProtocolError> {
    encode_bounded(
        VerificationWorkerRequestWireV1 {
            format_version: VERIFICATION_WORKER_PROTOCOL_VERSION,
            proof_artifact: request.proof_artifact.clone(),
            expected_public_values: request.expected_public_values.clone(),
            verification_key_id: request.trusted_program_key.verification_key_id().to_owned(),
            sp1_vkey_hash: request.trusted_program_key.sp1_vkey_hash().to_owned(),
            deadline_seconds: VERIFICATION_DEADLINE_SECONDS,
            correlation_id: request.correlation_id.clone(),
        },
        MAX_VERIFICATION_WORKER_REQUEST_BYTES,
        VerificationWorkerProtocolError::RequestTooLarge,
    )
}

pub fn decode_request(
    bytes: &[u8],
) -> Result<IsolatedVerificationRequest, VerificationWorkerProtocolError> {
    let wire: VerificationWorkerRequestWireV1 = decode_bounded(
        bytes,
        MAX_VERIFICATION_WORKER_REQUEST_BYTES,
        VerificationWorkerProtocolError::RequestTooLarge,
        VerificationWorkerProtocolError::MalformedRequest,
    )?;
    if wire.format_version != VERIFICATION_WORKER_PROTOCOL_VERSION {
        return Err(VerificationWorkerProtocolError::UnsupportedVersion);
    }
    if wire.deadline_seconds != VERIFICATION_DEADLINE_SECONDS {
        return Err(VerificationWorkerProtocolError::InvalidRequest);
    }
    let trusted_program_key =
        TrustedProgramKey::from_registry_record(&wire.verification_key_id, &wire.sp1_vkey_hash)
            .map_err(|_| VerificationWorkerProtocolError::InvalidRequest)?;
    IsolatedVerificationRequest::new(
        wire.proof_artifact,
        wire.expected_public_values,
        trusted_program_key,
        wire.correlation_id,
    )
}

pub fn encode_result(
    result: &VerificationWorkerResult,
) -> Result<Vec<u8>, VerificationWorkerProtocolError> {
    let outcome = match result {
        VerificationWorkerResult::Success(success) => {
            verification_worker_result_wire_v1::Outcome::Success(VerificationWorkerSuccessWireV1 {
                correlation_id: success.correlation_id.clone(),
                verification_key_id: success.verification_key_id.clone(),
            })
        }
        VerificationWorkerResult::Failure(failure) => {
            verification_worker_result_wire_v1::Outcome::Failure(VerificationWorkerFailureWireV1 {
                class: failure_class_to_wire(failure.class) as i32,
                correlation_id: failure.correlation_id.clone(),
            })
        }
    };
    encode_bounded(
        VerificationWorkerResultWireV1 {
            format_version: VERIFICATION_WORKER_PROTOCOL_VERSION,
            outcome: Some(outcome),
        },
        MAX_VERIFICATION_WORKER_RESULT_BYTES,
        VerificationWorkerProtocolError::ResultTooLarge,
    )
}

pub fn decode_result(
    bytes: &[u8],
) -> Result<VerificationWorkerResult, VerificationWorkerProtocolError> {
    let wire: VerificationWorkerResultWireV1 = decode_bounded(
        bytes,
        MAX_VERIFICATION_WORKER_RESULT_BYTES,
        VerificationWorkerProtocolError::ResultTooLarge,
        VerificationWorkerProtocolError::MalformedResult,
    )?;
    if wire.format_version != VERIFICATION_WORKER_PROTOCOL_VERSION {
        return Err(VerificationWorkerProtocolError::UnsupportedVersion);
    }
    match wire
        .outcome
        .ok_or(VerificationWorkerProtocolError::InvalidResult)?
    {
        verification_worker_result_wire_v1::Outcome::Success(success) => Ok(
            VerificationWorkerResult::Success(VerificationWorkerSuccess::new(
                success.correlation_id,
                success.verification_key_id,
            )?),
        ),
        verification_worker_result_wire_v1::Outcome::Failure(failure) => {
            let class = VerificationWorkerFailureClassWireV1::try_from(failure.class)
                .map_err(|_| VerificationWorkerProtocolError::InvalidResult)?;
            Ok(VerificationWorkerResult::Failure(
                VerificationWorkerFailure::new(
                    failure_class_from_wire(class)?,
                    failure.correlation_id,
                )?,
            ))
        }
    }
}

fn encode_bounded<M: Message>(
    message: M,
    maximum: usize,
    too_large: VerificationWorkerProtocolError,
) -> Result<Vec<u8>, VerificationWorkerProtocolError> {
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
    too_large: VerificationWorkerProtocolError,
    malformed: VerificationWorkerProtocolError,
) -> Result<M, VerificationWorkerProtocolError> {
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

const fn failure_class_to_wire(
    value: VerificationWorkerFailureClass,
) -> VerificationWorkerFailureClassWireV1 {
    match value {
        VerificationWorkerFailureClass::InvalidRequest => {
            VerificationWorkerFailureClassWireV1::InvalidRequest
        }
        VerificationWorkerFailureClass::ProofRejected => {
            VerificationWorkerFailureClassWireV1::ProofRejected
        }
        VerificationWorkerFailureClass::ResourceProfile => {
            VerificationWorkerFailureClassWireV1::ResourceProfile
        }
        VerificationWorkerFailureClass::Cancelled => {
            VerificationWorkerFailureClassWireV1::Cancelled
        }
        VerificationWorkerFailureClass::Internal => VerificationWorkerFailureClassWireV1::Internal,
    }
}

fn failure_class_from_wire(
    value: VerificationWorkerFailureClassWireV1,
) -> Result<VerificationWorkerFailureClass, VerificationWorkerProtocolError> {
    match value {
        VerificationWorkerFailureClassWireV1::Unspecified => {
            Err(VerificationWorkerProtocolError::InvalidResult)
        }
        VerificationWorkerFailureClassWireV1::InvalidRequest => {
            Ok(VerificationWorkerFailureClass::InvalidRequest)
        }
        VerificationWorkerFailureClassWireV1::ProofRejected => {
            Ok(VerificationWorkerFailureClass::ProofRejected)
        }
        VerificationWorkerFailureClassWireV1::ResourceProfile => {
            Ok(VerificationWorkerFailureClass::ResourceProfile)
        }
        VerificationWorkerFailureClassWireV1::Cancelled => {
            Ok(VerificationWorkerFailureClass::Cancelled)
        }
        VerificationWorkerFailureClassWireV1::Internal => {
            Ok(VerificationWorkerFailureClass::Internal)
        }
    }
}
