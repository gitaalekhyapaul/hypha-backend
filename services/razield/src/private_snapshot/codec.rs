use prost::Message;
use raziel_policy_ir::{CaveatValue, ValidatedPolicyProgram};
use raziel_snapshot::{RelationshipRecord, StoredCaveat, StoredContextEntry};

use super::wire::{
    PrivateRelationshipSnapshotWireV1, PrivateRelationshipWireV1, PrivateStoredCaveatWireV1,
    PrivateStoredContextEntryWireV1, private_stored_context_entry_wire_v1,
};
use super::{
    MAX_PRIVATE_SNAPSHOT_BYTES, PRIVATE_SNAPSHOT_FORMAT_VERSION, PrivateRelationshipSnapshot,
    PrivateSnapshotError,
};

pub(super) fn encode(
    snapshot: &PrivateRelationshipSnapshot,
) -> Result<Vec<u8>, PrivateSnapshotError> {
    let wire = PrivateRelationshipSnapshotWireV1 {
        format_version: PRIVATE_SNAPSHOT_FORMAT_VERSION,
        snapshot_id: snapshot.snapshot_id.clone(),
        relationship_root: snapshot.relationship_root.to_vec(),
        snapshot_secret_salt: snapshot.snapshot_secret_salt.to_vec(),
        relationships: snapshot
            .relationships
            .iter()
            .map(relationship_to_wire)
            .collect(),
    };
    if wire.encoded_len() > MAX_PRIVATE_SNAPSHOT_BYTES {
        return Err(PrivateSnapshotError::SnapshotTooLarge);
    }
    let bytes = wire.encode_to_vec();
    if bytes.len() > MAX_PRIVATE_SNAPSHOT_BYTES {
        return Err(PrivateSnapshotError::SnapshotTooLarge);
    }
    Ok(bytes)
}

pub(super) fn decode(
    policy: &ValidatedPolicyProgram,
    expected_snapshot_id: &str,
    expected_root: &[u8; 32],
    bytes: &[u8],
) -> Result<PrivateRelationshipSnapshot, PrivateSnapshotError> {
    if bytes.is_empty() {
        return Err(PrivateSnapshotError::MalformedSnapshot);
    }
    if bytes.len() > MAX_PRIVATE_SNAPSHOT_BYTES {
        return Err(PrivateSnapshotError::SnapshotTooLarge);
    }
    let wire = PrivateRelationshipSnapshotWireV1::decode(bytes)
        .map_err(|_| PrivateSnapshotError::MalformedSnapshot)?;
    if wire.encode_to_vec() != bytes {
        return Err(PrivateSnapshotError::MalformedSnapshot);
    }
    if wire.format_version != PRIVATE_SNAPSHOT_FORMAT_VERSION {
        return Err(PrivateSnapshotError::UnsupportedSnapshotVersion);
    }
    if wire.snapshot_id != expected_snapshot_id {
        return Err(PrivateSnapshotError::SnapshotIdMismatch);
    }
    let relationship_root = wire
        .relationship_root
        .as_slice()
        .try_into()
        .map_err(|_| PrivateSnapshotError::InvalidSnapshot)?;
    if &relationship_root != expected_root {
        return Err(PrivateSnapshotError::RootMismatch);
    }
    let snapshot_secret_salt = wire
        .snapshot_secret_salt
        .as_slice()
        .try_into()
        .map_err(|_| PrivateSnapshotError::InvalidSnapshot)?;
    let relationships = wire
        .relationships
        .iter()
        .map(relationship_from_wire)
        .collect::<Result<Vec<_>, _>>()?;

    PrivateRelationshipSnapshot::new(
        policy,
        wire.snapshot_id,
        relationship_root,
        snapshot_secret_salt,
        relationships,
    )
}

fn relationship_to_wire(value: &RelationshipRecord) -> PrivateRelationshipWireV1 {
    PrivateRelationshipWireV1 {
        resource_type: value.resource_type.clone(),
        resource_id: value.resource_id.clone(),
        relation: value.relation.clone(),
        subject_type: value.subject_type.clone(),
        subject_id: value.subject_id.clone(),
        subject_relation: value.subject_relation.clone(),
        stored_caveat: value.stored_caveat.as_ref().map(caveat_to_wire),
    }
}

fn relationship_from_wire(
    value: &PrivateRelationshipWireV1,
) -> Result<RelationshipRecord, PrivateSnapshotError> {
    Ok(RelationshipRecord {
        resource_type: value.resource_type.clone(),
        resource_id: value.resource_id.clone(),
        relation: value.relation.clone(),
        subject_type: value.subject_type.clone(),
        subject_id: value.subject_id.clone(),
        subject_relation: value.subject_relation.clone(),
        stored_caveat: value
            .stored_caveat
            .as_ref()
            .map(caveat_from_wire)
            .transpose()?,
    })
}

fn caveat_to_wire(value: &StoredCaveat) -> PrivateStoredCaveatWireV1 {
    PrivateStoredCaveatWireV1 {
        name: value.name.clone(),
        context: value.context.iter().map(context_to_wire).collect(),
    }
}

fn caveat_from_wire(
    value: &PrivateStoredCaveatWireV1,
) -> Result<StoredCaveat, PrivateSnapshotError> {
    Ok(StoredCaveat {
        name: value.name.clone(),
        context: value
            .context
            .iter()
            .map(context_from_wire)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn context_to_wire(entry: &StoredContextEntry) -> PrivateStoredContextEntryWireV1 {
    let value = match &entry.value {
        CaveatValue::Bool(value) => private_stored_context_entry_wire_v1::Value::Bool(*value),
        CaveatValue::Int(value) => private_stored_context_entry_wire_v1::Value::Int(*value),
        CaveatValue::String(value) => {
            private_stored_context_entry_wire_v1::Value::String(value.clone())
        }
    };
    PrivateStoredContextEntryWireV1 {
        name: entry.name.clone(),
        value: Some(value),
    }
}

fn context_from_wire(
    value: &PrivateStoredContextEntryWireV1,
) -> Result<StoredContextEntry, PrivateSnapshotError> {
    let caveat_value = match value
        .value
        .as_ref()
        .ok_or(PrivateSnapshotError::InvalidSnapshot)?
    {
        private_stored_context_entry_wire_v1::Value::Bool(value) => CaveatValue::Bool(*value),
        private_stored_context_entry_wire_v1::Value::Int(value) => CaveatValue::Int(*value),
        private_stored_context_entry_wire_v1::Value::String(value) => {
            CaveatValue::String(value.clone())
        }
    };
    Ok(StoredContextEntry {
        name: value.name.clone(),
        value: caveat_value,
    })
}
