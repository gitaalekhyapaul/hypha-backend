use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Value};
use raziel_proto::raziel::v1::VerifyCapabilityRequest;

use super::{MAX_VERIFICATION_SERVICE_MESSAGE_BYTES, VerificationAdmissionError};

const VERIFICATION_DESCRIPTOR: &[u8] =
    include_bytes!("../../descriptors/raziel-verification-v1.binpb");

pub const VERIFICATION_DESCRIPTOR_SHA256: [u8; 32] = [
    0x80, 0x77, 0x40, 0x13, 0x64, 0x9e, 0x64, 0xd7, 0xcf, 0xbe, 0x61, 0x5d, 0xe9, 0xec, 0xee, 0x41,
    0x80, 0xc1, 0x6a, 0x21, 0x4a, 0x93, 0xc4, 0x64, 0xf1, 0x4a, 0x53, 0x2a, 0x6f, 0x64, 0x95, 0x7d,
];

pub fn decode_verify_capability_request_strict(
    bytes: &[u8],
) -> Result<VerifyCapabilityRequest, VerificationAdmissionError> {
    if bytes.len() > MAX_VERIFICATION_SERVICE_MESSAGE_BYTES {
        return Err(VerificationAdmissionError::RequestTooLarge);
    }

    let pool = DescriptorPool::decode(VERIFICATION_DESCRIPTOR)
        .map_err(|_| VerificationAdmissionError::InvalidTransportDescriptor)?;
    let descriptor = pool
        .get_message_by_name("raziel.v1.VerifyCapabilityRequest")
        .ok_or(VerificationAdmissionError::InvalidTransportDescriptor)?;
    let dynamic = DynamicMessage::decode(descriptor, bytes)
        .map_err(|_| VerificationAdmissionError::MalformedProtobuf)?;
    validate_dynamic_message(&dynamic)?;

    let typed = VerifyCapabilityRequest::decode(bytes)
        .map_err(|_| VerificationAdmissionError::MalformedProtobuf)?;
    if typed.encode_to_vec() != bytes {
        return Err(VerificationAdmissionError::NonCanonicalProtobuf);
    }
    Ok(typed)
}

fn validate_dynamic_message(message: &DynamicMessage) -> Result<(), VerificationAdmissionError> {
    if message.unknown_fields().next().is_some() {
        return Err(VerificationAdmissionError::UnknownProtobufField);
    }
    if message.extensions().next().is_some() {
        return Err(VerificationAdmissionError::ProtobufExtension);
    }
    for (_, value) in message.fields() {
        validate_dynamic_value(value)?;
    }
    Ok(())
}

fn validate_dynamic_value(value: &Value) -> Result<(), VerificationAdmissionError> {
    match value {
        Value::Message(message) => validate_dynamic_message(message),
        Value::List(values) => {
            for value in values {
                validate_dynamic_value(value)?;
            }
            Ok(())
        }
        Value::Map(values) => {
            for value in values.values() {
                validate_dynamic_value(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use sha2::{Digest, Sha256};

    use super::*;
    #[test]
    fn frozen_descriptor_matches_approved_checksum() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(VERIFICATION_DESCRIPTOR)),
            VERIFICATION_DESCRIPTOR_SHA256
        );
    }

    #[test]
    fn rejects_unknown_duplicate_and_noncanonical_wire_fields() {
        let request = VerifyCapabilityRequest {
            request_method: "POST".into(),
            ..VerifyCapabilityRequest::default()
        };
        let canonical = request.encode_to_vec();
        assert_eq!(
            decode_verify_capability_request_strict(&canonical),
            Ok(request)
        );

        let mut unknown = canonical.clone();
        unknown.extend_from_slice(&[0x98, 0x06, 0x01]);
        assert_eq!(
            decode_verify_capability_request_strict(&unknown),
            Err(VerificationAdmissionError::UnknownProtobufField)
        );

        let duplicate = [canonical.as_slice(), &[0x1a, 0x03, b'G', b'E', b'T']].concat();
        assert_eq!(
            decode_verify_capability_request_strict(&duplicate),
            Err(VerificationAdmissionError::NonCanonicalProtobuf)
        );

        let explicit_default = [canonical.as_slice(), &[0x12, 0x00]].concat();
        assert_eq!(
            decode_verify_capability_request_strict(&explicit_default),
            Err(VerificationAdmissionError::NonCanonicalProtobuf)
        );
    }

    #[test]
    fn raw_ingress_applies_the_two_mib_ceiling() {
        assert_eq!(
            decode_verify_capability_request_strict(&vec![
                0;
                MAX_VERIFICATION_SERVICE_MESSAGE_BYTES
                    + 1
            ]),
            Err(VerificationAdmissionError::RequestTooLarge)
        );
    }
}
