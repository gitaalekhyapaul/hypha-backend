use prost_types::value::Kind;
use raziel_policy_ir::CaveatValue;
use raziel_snapshot::{RelationshipRecord, StoredCaveat, StoredContextEntry};
use spicedb_rs_proto::authzed::api::v1 as authzed;

use super::SpiceDbSnapshotError;

const MAX_EXACT_JSON_INTEGER: f64 = 9_007_199_254_740_991.0;

pub(super) fn relationship_from_api(
    value: &authzed::Relationship,
) -> Result<RelationshipRecord, SpiceDbSnapshotError> {
    if value.optional_expires_at.is_some() {
        return Err(SpiceDbSnapshotError::UnsupportedExpiration);
    }
    let resource = value
        .resource
        .as_ref()
        .ok_or(SpiceDbSnapshotError::MalformedRelationship)?;
    let subject = value
        .subject
        .as_ref()
        .ok_or(SpiceDbSnapshotError::MalformedRelationship)?;
    let subject_object = subject
        .object
        .as_ref()
        .ok_or(SpiceDbSnapshotError::MalformedRelationship)?;
    if subject_object.object_id == "*" {
        return Err(SpiceDbSnapshotError::UnsupportedWildcard);
    }

    Ok(RelationshipRecord {
        resource_type: resource.object_type.clone(),
        resource_id: resource.object_id.clone(),
        relation: value.relation.clone(),
        subject_type: subject_object.object_type.clone(),
        subject_id: subject_object.object_id.clone(),
        subject_relation: (!subject.optional_relation.is_empty())
            .then(|| subject.optional_relation.clone()),
        stored_caveat: value
            .optional_caveat
            .as_ref()
            .map(caveat_from_api)
            .transpose()?,
    })
}

fn caveat_from_api(
    value: &authzed::ContextualizedCaveat,
) -> Result<StoredCaveat, SpiceDbSnapshotError> {
    let context = value
        .context
        .as_ref()
        .map(|context| {
            context
                .fields
                .iter()
                .map(|(name, value)| {
                    Ok(StoredContextEntry {
                        name: name.clone(),
                        value: caveat_value_from_api(value)?,
                    })
                })
                .collect::<Result<Vec<_>, SpiceDbSnapshotError>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(StoredCaveat {
        name: value.caveat_name.clone(),
        context,
    })
}

fn caveat_value_from_api(value: &prost_types::Value) -> Result<CaveatValue, SpiceDbSnapshotError> {
    match value.kind.as_ref() {
        Some(Kind::BoolValue(value)) => Ok(CaveatValue::Bool(*value)),
        Some(Kind::StringValue(value)) => Ok(CaveatValue::String(value.clone())),
        Some(Kind::NumberValue(value))
            if value.is_finite()
                && value.fract() == 0.0
                && value.abs() <= MAX_EXACT_JSON_INTEGER =>
        {
            Ok(CaveatValue::Int(*value as i64))
        }
        _ => Err(SpiceDbSnapshotError::UnsupportedCaveatValue),
    }
}
