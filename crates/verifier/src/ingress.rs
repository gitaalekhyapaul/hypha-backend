use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Value};
use raziel_proto::raziel::v1 as proto;

use crate::RegistryError;

const REGISTRY_DESCRIPTOR: &[u8] = include_bytes!("../descriptors/raziel-registry-v1.binpb");

pub const REGISTRY_DESCRIPTOR_SHA256: [u8; 32] = [
    0x61, 0x0b, 0x68, 0x2a, 0xe0, 0x0a, 0x2c, 0xfa, 0x0f, 0x2e, 0xfa, 0x5b, 0xee, 0xb7, 0xe9, 0xe6,
    0xe4, 0x77, 0x35, 0xb0, 0xb2, 0x57, 0x0a, 0x97, 0x8a, 0x51, 0xa0, 0x17, 0xe5, 0x36, 0xaa, 0x28,
];

pub fn decode_authority_key_record_strict(
    bytes: &[u8],
    max_bytes: u64,
) -> Result<proto::AuthorityKeyRecord, RegistryError> {
    decode_strict(bytes, max_bytes, "raziel.v1.AuthorityKeyRecord")
}

pub fn decode_policy_record_strict(
    bytes: &[u8],
    max_bytes: u64,
) -> Result<proto::PolicyRecord, RegistryError> {
    decode_strict(bytes, max_bytes, "raziel.v1.PolicyRecord")
}

pub fn decode_accepted_root_record_strict(
    bytes: &[u8],
    max_bytes: u64,
) -> Result<proto::AcceptedRootRecord, RegistryError> {
    decode_strict(bytes, max_bytes, "raziel.v1.AcceptedRootRecord")
}

pub fn decode_registry_manifest_strict(
    bytes: &[u8],
    max_bytes: u64,
) -> Result<proto::RegistryGenerationManifest, RegistryError> {
    decode_strict(bytes, max_bytes, "raziel.v1.RegistryGenerationManifest")
}

fn decode_strict<M: Message + Default>(
    bytes: &[u8],
    max_bytes: u64,
    message_name: &str,
) -> Result<M, RegistryError> {
    if u64::try_from(bytes.len()).map_or(true, |length| length > max_bytes) {
        return Err(RegistryError::InputTooLarge);
    }

    let pool = DescriptorPool::decode(REGISTRY_DESCRIPTOR)
        .map_err(|_| RegistryError::InvalidDescriptor)?;
    let descriptor = pool
        .get_message_by_name(message_name)
        .ok_or(RegistryError::MissingMessageDescriptor)?;
    let dynamic =
        DynamicMessage::decode(descriptor, bytes).map_err(|_| RegistryError::MalformedProtobuf)?;
    validate_dynamic_message(&dynamic)?;

    let typed = M::decode(bytes).map_err(|_| RegistryError::MalformedProtobuf)?;
    if typed.encode_to_vec() != bytes {
        return Err(RegistryError::NonCanonicalProtobuf);
    }
    Ok(typed)
}

fn validate_dynamic_message(message: &DynamicMessage) -> Result<(), RegistryError> {
    if message.unknown_fields().next().is_some() {
        return Err(RegistryError::UnknownField);
    }
    if message.extensions().next().is_some() {
        return Err(RegistryError::ExtensionField);
    }
    for (_, value) in message.fields() {
        validate_dynamic_value(value)?;
    }
    Ok(())
}

fn validate_dynamic_value(value: &Value) -> Result<(), RegistryError> {
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

    use super::{
        REGISTRY_DESCRIPTOR, REGISTRY_DESCRIPTOR_SHA256, decode_accepted_root_record_strict,
        decode_authority_key_record_strict, decode_policy_record_strict,
        decode_registry_manifest_strict,
    };
    use crate::RegistryError;
    use raziel_proto::raziel::v1 as proto;

    #[test]
    fn frozen_descriptor_matches_approved_checksum() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(REGISTRY_DESCRIPTOR)),
            REGISTRY_DESCRIPTOR_SHA256
        );
    }

    #[test]
    fn rejects_unknown_and_noncanonical_wire_spellings() {
        let record = proto::AuthorityKeyRecord {
            format_version: 1,
            trust_domain_id: "raziel-test".into(),
            ..proto::AuthorityKeyRecord::default()
        };
        let canonical = record.encode_to_vec();
        assert_eq!(
            decode_authority_key_record_strict(&canonical, canonical.len() as u64),
            Ok(record)
        );

        let mut unknown = canonical.clone();
        unknown.extend_from_slice(&[0x98, 0x06, 0x01]);
        assert_eq!(
            decode_authority_key_record_strict(&unknown, unknown.len() as u64),
            Err(RegistryError::UnknownField)
        );

        let duplicate = [canonical.as_slice(), &[0x08, 0x01]].concat();
        assert_eq!(
            decode_authority_key_record_strict(&duplicate, duplicate.len() as u64),
            Err(RegistryError::NonCanonicalProtobuf)
        );

        let policy = proto::PolicyRecord {
            format_version: 1,
            ..proto::PolicyRecord::default()
        }
        .encode_to_vec();
        let root = proto::AcceptedRootRecord {
            format_version: 1,
            ..proto::AcceptedRootRecord::default()
        }
        .encode_to_vec();
        let manifest = proto::RegistryGenerationManifest {
            format_version: 1,
            ..proto::RegistryGenerationManifest::default()
        }
        .encode_to_vec();

        for result in [
            decode_policy_record_strict(
                &[policy.as_slice(), &[0x98, 0x06, 0x01]].concat(),
                (policy.len() + 3) as u64,
            )
            .map(|_| ()),
            decode_accepted_root_record_strict(
                &[root.as_slice(), &[0x98, 0x06, 0x01]].concat(),
                (root.len() + 3) as u64,
            )
            .map(|_| ()),
            decode_registry_manifest_strict(
                &[manifest.as_slice(), &[0x98, 0x06, 0x01]].concat(),
                (manifest.len() + 3) as u64,
            )
            .map(|_| ()),
        ] {
            assert_eq!(result, Err(RegistryError::UnknownField));
        }
    }
}
