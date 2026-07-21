use std::fmt::{self, Display, Formatter};

use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Value};
use raziel_proto::raziel::v1 as proto;

use crate::{PolicyError, ValidatedPolicyProgram, ValidationCeilings};

const POLICY_PROGRAM_NAME: &str = "raziel.v1.PolicyProgram";
const POLICY_IR_V1_DESCRIPTOR: &[u8] = include_bytes!("../descriptors/raziel-policy-ir-v1.binpb");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyIngressError {
    InputTooLarge,
    InvalidDescriptor,
    MissingMessageDescriptor,
    MalformedProtobuf,
    UnknownField,
    ExtensionField,
    NonCanonicalProtobuf,
    InvalidPolicy(PolicyError),
}

impl Display for PolicyIngressError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputTooLarge => "PolicyIR Protobuf exceeds its byte ceiling",
            Self::InvalidDescriptor => "frozen PolicyIR descriptor is invalid",
            Self::MissingMessageDescriptor => {
                "frozen PolicyIR descriptor does not contain PolicyProgram"
            }
            Self::MalformedProtobuf => "PolicyIR Protobuf is malformed",
            Self::UnknownField => "PolicyIR Protobuf contains an unknown field",
            Self::ExtensionField => "PolicyIR Protobuf contains an extension field",
            Self::NonCanonicalProtobuf => "PolicyIR Protobuf uses a noncanonical wire spelling",
            Self::InvalidPolicy(_) => "PolicyIR fails semantic validation",
        })
    }
}

impl std::error::Error for PolicyIngressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPolicy(source) => Some(source),
            _ => None,
        }
    }
}

pub fn decode_policy_program_strict(
    bytes: &[u8],
    max_bytes: u64,
    ceilings: &ValidationCeilings,
) -> Result<ValidatedPolicyProgram, PolicyIngressError> {
    if u64::try_from(bytes.len()).map_or(true, |length| length > max_bytes) {
        return Err(PolicyIngressError::InputTooLarge);
    }

    let pool = DescriptorPool::decode(POLICY_IR_V1_DESCRIPTOR)
        .map_err(|_| PolicyIngressError::InvalidDescriptor)?;
    let descriptor = pool
        .get_message_by_name(POLICY_PROGRAM_NAME)
        .ok_or(PolicyIngressError::MissingMessageDescriptor)?;
    let dynamic = DynamicMessage::decode(descriptor, bytes)
        .map_err(|_| PolicyIngressError::MalformedProtobuf)?;
    validate_dynamic_message(&dynamic)?;

    let wire =
        proto::PolicyProgram::decode(bytes).map_err(|_| PolicyIngressError::MalformedProtobuf)?;
    if wire.encode_to_vec() != bytes {
        return Err(PolicyIngressError::NonCanonicalProtobuf);
    }

    ValidatedPolicyProgram::from_trusted_proto(wire, ceilings)
        .map_err(PolicyIngressError::InvalidPolicy)
}

fn validate_dynamic_message(message: &DynamicMessage) -> Result<(), PolicyIngressError> {
    if message.unknown_fields().next().is_some() {
        return Err(PolicyIngressError::UnknownField);
    }
    if message.extensions().next().is_some() {
        return Err(PolicyIngressError::ExtensionField);
    }

    for (_, value) in message.fields() {
        validate_dynamic_value(value)?;
    }
    Ok(())
}

fn validate_dynamic_value(value: &Value) -> Result<(), PolicyIngressError> {
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
