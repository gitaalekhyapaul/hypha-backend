use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};

use raziel_policy_ir::CaveatValue;

use crate::{CanonicalEncoder, CanonicalError};

pub const CANONICAL_RELATIONSHIP_ENCODING_VERSION: u32 = 1;

#[derive(Clone, PartialEq, Eq)]
pub struct RelationshipRecord {
    pub resource_type: String,
    pub resource_id: String,
    pub relation: String,
    pub subject_type: String,
    pub subject_id: String,
    pub subject_relation: Option<String>,
    pub stored_caveat: Option<StoredCaveat>,
}

impl Debug for RelationshipRecord {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelationshipRecord")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct StoredCaveat {
    pub name: String,
    pub context: Vec<StoredContextEntry>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct StoredContextEntry {
    pub name: String,
    pub value: CaveatValue,
}

pub fn compare_relationship_identity(
    left: &RelationshipRecord,
    right: &RelationshipRecord,
) -> Ordering {
    compare_str(&left.resource_type, &right.resource_type)
        .then_with(|| compare_str(&left.resource_id, &right.resource_id))
        .then_with(|| compare_str(&left.relation, &right.relation))
        .then_with(|| compare_str(&left.subject_type, &right.subject_type))
        .then_with(|| compare_str(&left.subject_id, &right.subject_id))
        .then_with(|| compare_optional_str(&left.subject_relation, &right.subject_relation))
}

pub fn encode_relationship(relationship: &RelationshipRecord) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(CANONICAL_RELATIONSHIP_ENCODING_VERSION);
    encoder.write_str(&relationship.resource_type)?;
    encoder.write_str(&relationship.resource_id)?;
    encoder.write_str(&relationship.relation)?;
    encoder.write_str(&relationship.subject_type)?;
    encoder.write_str(&relationship.subject_id)?;
    encoder.write_optional_str(relationship.subject_relation.as_deref())?;
    encode_optional_caveat(&mut encoder, relationship.stored_caveat.as_ref())?;
    let payload = encoder.finish();
    u32::try_from(payload.len()).map_err(|_| CanonicalError::LengthOverflow)?;
    Ok(payload)
}

fn encode_optional_caveat(
    encoder: &mut CanonicalEncoder,
    caveat: Option<&StoredCaveat>,
) -> Result<(), CanonicalError> {
    match caveat {
        None => encoder.write_u8(0x00),
        Some(caveat) => {
            encoder.write_u8(0x01);
            encoder.write_str(&caveat.name)?;
            encoder.write_collection(&caveat.context, encode_context_entry)?;
        }
    }
    Ok(())
}

fn encode_context_entry(
    encoder: &mut CanonicalEncoder,
    entry: &StoredContextEntry,
) -> Result<(), CanonicalError> {
    encoder.write_str(&entry.name)?;
    match &entry.value {
        CaveatValue::Bool(value) => {
            encoder.write_u8(0x01);
            encoder.write_bool(*value);
        }
        CaveatValue::Int(value) => {
            encoder.write_u8(0x02);
            encoder.write_i64(*value);
        }
        CaveatValue::String(value) => {
            encoder.write_u8(0x03);
            encoder.write_str(value)?;
        }
    }
    Ok(())
}

fn compare_str(left: &str, right: &str) -> Ordering {
    left.as_bytes().cmp(right.as_bytes())
}

fn compare_optional_str(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => compare_str(left, right),
    }
}

#[cfg(test)]
mod tests {
    use raziel_policy_ir::CaveatValue;

    use super::{RelationshipRecord, StoredCaveat, StoredContextEntry, encode_relationship};

    #[test]
    fn encodes_every_relationship_field_and_scalar_tag() {
        let encoded = encode_relationship(&RelationshipRecord {
            resource_type: "dataset".into(),
            resource_id: "synthetic-private-test".into(),
            relation: "query".into(),
            subject_type: "agent".into(),
            subject_id: "synthetic-agent".into(),
            subject_relation: Some("member".into()),
            stored_caveat: Some(StoredCaveat {
                name: "limits".into(),
                context: vec![
                    StoredContextEntry {
                        name: "approved".into(),
                        value: CaveatValue::Bool(true),
                    },
                    StoredContextEntry {
                        name: "count".into(),
                        value: CaveatValue::Int(-1),
                    },
                    StoredContextEntry {
                        name: "label".into(),
                        value: CaveatValue::String("synthetic".into()),
                    },
                ],
            }),
        })
        .unwrap();

        assert_eq!(&encoded[..4], &[0, 0, 0, 1]);
        assert!(encoded.windows(2).any(|bytes| bytes == [0x02, 0xff]));
        assert!(encoded.windows(5).any(|bytes| bytes == [0x03, 0, 0, 0, 9]));
    }
}
