use std::cmp::Ordering;

use raziel_canonical::{
    RelationshipRecord, StoredCaveat, compare_relationship_identity, encode_relationship,
};
use raziel_policy_ir::{
    CaveatDefinition, CaveatInputSource, CaveatValue, CaveatValueType, ObjectDefinition,
    RelationDefinition, ValidatedPolicyProgram,
};

use crate::{PolicyReference, RelationshipField, SnapshotError, SnapshotLimit, SnapshotLimits};

pub(crate) fn validate_relationships(
    policy: &ValidatedPolicyProgram,
    relationships: &[RelationshipRecord],
    limits: SnapshotLimits,
) -> Result<Vec<Vec<u8>>, SnapshotError> {
    let count = u32::try_from(relationships.len())
        .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::Relationships))?;
    if count > limits.max_relationships {
        return Err(SnapshotError::LimitExceeded(SnapshotLimit::Relationships));
    }

    let mut canonical = Vec::with_capacity(relationships.len());
    let mut total_bytes = 0_u64;
    let mut previous: Option<&RelationshipRecord> = None;

    for relationship in relationships {
        if let Some(previous) = previous {
            match compare_relationship_identity(previous, relationship) {
                Ordering::Less => {}
                Ordering::Equal => {
                    return Err(SnapshotError::DuplicateRelationshipIdentity);
                }
                Ordering::Greater => {
                    return Err(SnapshotError::RelationshipsNotStrictlySorted);
                }
            }
        }

        validate_relationship(policy, relationship)?;
        let encoded = encode_relationship(relationship)?;
        let encoded_length = u32::try_from(encoded.len())
            .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::RelationshipBytes))?;
        if encoded_length > limits.max_relationship_bytes {
            return Err(SnapshotError::LimitExceeded(
                SnapshotLimit::RelationshipBytes,
            ));
        }
        total_bytes = total_bytes
            .checked_add(u64::from(encoded_length))
            .ok_or(SnapshotError::ArithmeticOverflow)?;
        if total_bytes > limits.max_total_canonical_bytes {
            return Err(SnapshotError::LimitExceeded(
                SnapshotLimit::TotalCanonicalBytes,
            ));
        }
        canonical.push(encoded);
        previous = Some(relationship);
    }

    Ok(canonical)
}

pub(crate) fn validate_and_encode_opening_relationship(
    policy: &ValidatedPolicyProgram,
    relationship: &RelationshipRecord,
    limits: SnapshotLimits,
) -> Result<Vec<u8>, SnapshotError> {
    validate_relationship(policy, relationship)?;
    let canonical = encode_relationship(relationship)?;
    let encoded_length = u32::try_from(canonical.len())
        .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::RelationshipBytes))?;
    if encoded_length > limits.max_relationship_bytes {
        return Err(SnapshotError::LimitExceeded(
            SnapshotLimit::RelationshipBytes,
        ));
    }
    Ok(canonical)
}

fn validate_relationship(
    policy: &ValidatedPolicyProgram,
    relationship: &RelationshipRecord,
) -> Result<(), SnapshotError> {
    let program = policy.program();
    let limits = program.limits;

    validate_schema_identifier(
        &relationship.resource_type,
        RelationshipField::ResourceType,
        limits.max_string_bytes,
    )?;
    validate_object_identifier(
        &relationship.resource_id,
        RelationshipField::ResourceId,
        limits.max_string_bytes,
    )?;
    validate_schema_identifier(
        &relationship.relation,
        RelationshipField::Relation,
        limits.max_string_bytes,
    )?;
    validate_schema_identifier(
        &relationship.subject_type,
        RelationshipField::SubjectType,
        limits.max_string_bytes,
    )?;
    validate_object_identifier(
        &relationship.subject_id,
        RelationshipField::SubjectId,
        limits.max_string_bytes,
    )?;

    let resource_definition = find_definition(policy, &relationship.resource_type).ok_or(
        SnapshotError::MissingPolicyReference(PolicyReference::ResourceType),
    )?;
    let relation = find_relation(resource_definition, &relationship.relation).ok_or(
        SnapshotError::MissingPolicyReference(PolicyReference::Relation),
    )?;
    let subject_definition = find_definition(policy, &relationship.subject_type).ok_or(
        SnapshotError::MissingPolicyReference(PolicyReference::SubjectType),
    )?;

    if let Some(subject_relation) = &relationship.subject_relation {
        validate_schema_identifier(
            subject_relation,
            RelationshipField::SubjectRelation,
            limits.max_string_bytes,
        )?;
        find_relation(subject_definition, subject_relation).ok_or(
            SnapshotError::MissingPolicyReference(PolicyReference::SubjectRelation),
        )?;
    }

    if relationship.subject_relation.is_some() && relationship.stored_caveat.is_some() {
        return Err(SnapshotError::UnsupportedRelationshipShape);
    }

    let caveat_name = relationship
        .stored_caveat
        .as_ref()
        .map(|caveat| caveat.name.as_str());
    if !relation.allowed_subject_types.iter().any(|allowed| {
        allowed.object_type == relationship.subject_type
            && allowed.subject_relation.as_deref() == relationship.subject_relation.as_deref()
            && allowed.caveat_name.as_deref() == caveat_name
    }) {
        return Err(SnapshotError::UnsupportedRelationshipShape);
    }

    if let Some(caveat) = &relationship.stored_caveat {
        validate_caveat(policy, caveat)?;
    }
    Ok(())
}

fn validate_caveat(
    policy: &ValidatedPolicyProgram,
    caveat: &StoredCaveat,
) -> Result<(), SnapshotError> {
    let limits = policy.program().limits;
    validate_schema_identifier(
        &caveat.name,
        RelationshipField::CaveatName,
        limits.max_string_bytes,
    )?;
    let definition = find_caveat(policy, &caveat.name).ok_or(
        SnapshotError::MissingPolicyReference(PolicyReference::Caveat),
    )?;

    let count = u32::try_from(caveat.context.len())
        .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::ContextEntries))?;
    if count > limits.max_context_entries {
        return Err(SnapshotError::LimitExceeded(SnapshotLimit::ContextEntries));
    }
    if caveat
        .context
        .windows(2)
        .any(|pair| pair[0].name.as_bytes().cmp(pair[1].name.as_bytes()) != Ordering::Less)
    {
        return Err(SnapshotError::ContextNotStrictlySorted);
    }

    let relationship_parameters: Vec<_> = definition
        .parameters
        .iter()
        .filter(|parameter| parameter.source == CaveatInputSource::RelationshipContext)
        .collect();
    if relationship_parameters.len() != caveat.context.len() {
        return Err(SnapshotError::ContextParameterMismatch);
    }

    for (entry, parameter) in caveat.context.iter().zip(relationship_parameters) {
        validate_schema_identifier(
            &entry.name,
            RelationshipField::ContextName,
            limits.max_string_bytes,
        )?;
        if entry.name != parameter.name {
            return Err(SnapshotError::ContextParameterMismatch);
        }
        if caveat_value_type(&entry.value) != parameter.value_type {
            return Err(SnapshotError::ContextTypeMismatch);
        }
        if let CaveatValue::String(value) = &entry.value {
            check_string_limit(
                value,
                RelationshipField::ContextString,
                limits.max_string_bytes,
            )?;
        }
    }
    Ok(())
}

fn find_definition<'a>(
    policy: &'a ValidatedPolicyProgram,
    name: &str,
) -> Option<&'a ObjectDefinition> {
    policy
        .program()
        .definitions
        .iter()
        .find(|definition| definition.name == name)
}

fn find_relation<'a>(
    definition: &'a ObjectDefinition,
    name: &str,
) -> Option<&'a RelationDefinition> {
    definition
        .relations
        .iter()
        .find(|relation| relation.name == name)
}

fn find_caveat<'a>(policy: &'a ValidatedPolicyProgram, name: &str) -> Option<&'a CaveatDefinition> {
    policy
        .program()
        .caveats
        .iter()
        .find(|caveat| caveat.name == name)
}

fn validate_schema_identifier(
    value: &str,
    field: RelationshipField,
    max_bytes: u32,
) -> Result<(), SnapshotError> {
    check_string_limit(value, field, max_bytes)?;
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(SnapshotError::InvalidSchemaIdentifier(field));
    };
    if !first.is_ascii_lowercase()
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(SnapshotError::InvalidSchemaIdentifier(field));
    }
    Ok(())
}

fn validate_object_identifier(
    value: &str,
    field: RelationshipField,
    max_bytes: u32,
) -> Result<(), SnapshotError> {
    check_string_limit(value, field, max_bytes)?;
    if value.is_empty() || value.as_bytes().contains(&0) {
        return Err(SnapshotError::InvalidObjectIdentifier(field));
    }
    Ok(())
}

fn check_string_limit(
    value: &str,
    field: RelationshipField,
    max_bytes: u32,
) -> Result<(), SnapshotError> {
    if value.len() > max_bytes as usize {
        return Err(SnapshotError::LimitExceeded(SnapshotLimit::StringBytes));
    }
    if matches!(
        field,
        RelationshipField::ResourceId | RelationshipField::SubjectId
    ) && value.is_empty()
    {
        return Err(SnapshotError::InvalidObjectIdentifier(field));
    }
    Ok(())
}

fn caveat_value_type(value: &CaveatValue) -> CaveatValueType {
    match value {
        CaveatValue::Bool(_) => CaveatValueType::Bool,
        CaveatValue::Int(_) => CaveatValueType::Int,
        CaveatValue::String(_) => CaveatValueType::String,
    }
}
