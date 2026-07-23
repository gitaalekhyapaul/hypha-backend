use std::path::Path;

use raziel_proto::raziel::v1 as proto;
use serde_json::{Value, json};

use crate::{
    error::AdminError,
    io::{atomic_write, atomic_write_new, lowercase_hex},
};

pub fn authority_key(value: &proto::AuthorityKeyRecord) -> Value {
    json!({
        "format_version": value.format_version,
        "trust_domain_id": value.trust_domain_id,
        "key_id": value.key_id,
        "role": proto::AuthorityRole::try_from(value.role)
            .map_or("UNKNOWN", |role| role.as_str_name()),
        "public_key_hex": lowercase_hex(&value.public_key),
        "status": proto::AuthorityKeyStatus::try_from(value.status)
            .map_or("UNKNOWN", |status| status.as_str_name()),
        "valid_from": timestamp(value.valid_from.as_ref()),
        "valid_until": timestamp(value.valid_until.as_ref()),
        "revoked_at": timestamp(value.revoked_at.as_ref()),
        "revocation_reason": optional_text(&value.revocation_reason),
        "registry_generation": value.registry_generation.to_string(),
    })
}

pub fn policy(value: &proto::PolicyRecord) -> Value {
    let program = value.canonical_policy_ir.as_ref();
    json!({
        "format_version": value.format_version,
        "trust_domain_id": value.trust_domain_id,
        "record_id": value.record_id,
        "policy_name": value.policy_name,
        "policy_version": value.policy_version,
        "approval_epoch": value.approval_epoch.to_string(),
        "policy_hash_hex": lowercase_hex(&value.policy_hash),
        "status": proto::PolicyStatus::try_from(value.status)
            .map_or("UNKNOWN", |status| status.as_str_name()),
        "valid_from": timestamp(value.valid_from.as_ref()),
        "valid_until": timestamp(value.valid_until.as_ref()),
        "approved_by": value.approved_by,
        "signing_key_id": value.signing_key_id,
        "authority_signature_hex": lowercase_hex(&value.authority_signature),
        "registered_at": timestamp(value.registered_at.as_ref()),
        "revoked_at": timestamp(value.revoked_at.as_ref()),
        "revocation_reason": optional_text(&value.revocation_reason),
        "registry_generation": value.registry_generation.to_string(),
        "policy_ir": program.map(|program| json!({
            "ir_version": program.ir_version,
            "compiler_name": program.compiler_name,
            "compiler_version": program.compiler_version,
            "source_schema_digest_hex": lowercase_hex(&program.source_schema_digest),
            "definitions": program.definitions.iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            "permissions": program.permissions.iter()
                .map(|permission| format!("{}#{}", permission.object_type, permission.name))
                .collect::<Vec<_>>(),
            "caveats": program.caveats.iter()
                .map(|caveat| caveat.name.as_str())
                .collect::<Vec<_>>(),
        })),
    })
}

pub fn accepted_root(value: &proto::AcceptedRootRecord) -> Value {
    let descriptor = value.descriptor.as_ref();
    json!({
        "format_version": value.format_version,
        "status": proto::SnapshotStatus::try_from(value.status)
            .map_or("UNKNOWN", |status| status.as_str_name()),
        "registered_at": timestamp(value.registered_at.as_ref()),
        "revoked_at": timestamp(value.revoked_at.as_ref()),
        "revocation_reason": optional_text(&value.revocation_reason),
        "registry_generation": value.registry_generation.to_string(),
        "descriptor": descriptor.map(|descriptor| json!({
            "format_version": descriptor.format_version,
            "trust_domain_id": descriptor.trust_domain_id,
            "snapshot_id": descriptor.snapshot_id,
            "relationship_snapshot_root_hex":
                lowercase_hex(&descriptor.relationship_snapshot_root),
            "policy_record_id": descriptor.policy_record_id,
            "policy_hash_hex": lowercase_hex(&descriptor.policy_hash),
            "spicedb_revision": descriptor.spicedb_revision,
            "epoch": descriptor.epoch.to_string(),
            "issued_at": timestamp(descriptor.issued_at.as_ref()),
            "expires_at": timestamp(descriptor.expires_at.as_ref()),
            "canonicalization_version": descriptor.canonicalization_version,
            "signing_key_id": descriptor.signing_key_id,
            "authority_signature_hex": lowercase_hex(&descriptor.authority_signature),
        })),
    })
}

pub fn manifest(value: &proto::RegistryGenerationManifest) -> Value {
    json!({
        "format_version": value.format_version,
        "trust_domain_id": value.trust_domain_id,
        "generation": value.generation.to_string(),
        "published_at": timestamp(value.published_at.as_ref()),
        "authority_keys": files(&value.authority_keys),
        "policies": files(&value.policies),
        "accepted_roots": files(&value.accepted_roots),
    })
}

pub fn write_new(path: &Path, value: &Value) -> Result<(), AdminError> {
    atomic_write_new(path, &serialize(value)?)
}

pub fn write_replace(path: &Path, value: &Value) -> Result<(), AdminError> {
    atomic_write(path, &serialize(value)?)
}

fn files(values: &[proto::RegistryFile]) -> Vec<Value> {
    values
        .iter()
        .map(|value| {
            json!({
                "relative_path": value.relative_path,
                "sha256_hex": lowercase_hex(&value.sha256),
            })
        })
        .collect()
}

fn timestamp(value: Option<&prost_types::Timestamp>) -> Value {
    value.map_or(Value::Null, |value| {
        json!({
            "seconds": value.seconds.to_string(),
            "nanos": value.nanos,
        })
    })
}

fn optional_text(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.into())
    }
}

fn serialize(value: &Value) -> Result<Vec<u8>, AdminError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| AdminError::FileWrite)?;
    bytes.push(b'\n');
    Ok(bytes)
}
