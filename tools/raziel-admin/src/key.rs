use prost::Message;
use raziel_authority::{
    AUTHORITY_KEY_FORMAT_VERSION, AuthorityKeyRecordV1, AuthorityKeyStatus, AuthorityRole,
    AuthoritySigner, PolicyApprovalRole, RoleMarker, SnapshotRole, TrustedAuthorityKey,
};
use raziel_verifier::authority_key_to_proto;

use crate::{
    args::Arguments,
    config::registry_config,
    error::AdminError,
    io::{atomic_write_new, lowercase_hex},
    projection,
};

pub fn inspect(arguments: &Arguments) -> Result<(), AdminError> {
    arguments.require_known(&[
        "--key",
        "--role",
        "--trust-domain",
        "--valid-from",
        "--valid-until",
        "--generation",
        "--output",
        "--json-output",
    ])?;
    let role = arguments.required("--role")?;
    match role {
        "policy" => inspect_typed::<PolicyApprovalRole>(arguments, AuthorityRole::PolicyApproval),
        "snapshot" => inspect_typed::<SnapshotRole>(arguments, AuthorityRole::Snapshot),
        _ => Err(AdminError::Usage("--role must be policy or snapshot")),
    }
}

fn inspect_typed<R: RoleMarker>(
    arguments: &Arguments,
    role: AuthorityRole,
) -> Result<(), AdminError> {
    let key_path = arguments.required_path("--key")?;
    let trust_domain_id = arguments.required("--trust-domain")?;
    let valid_from_unix_seconds = arguments.required_i64("--valid-from")?;
    let valid_until_unix_seconds = arguments.required_i64("--valid-until")?;
    let registry_generation = arguments.required_u64("--generation")?;
    let output = arguments.required_path("--output")?;
    let json_output = arguments.required_path("--json-output")?;

    let signer = AuthoritySigner::<R>::from_read_only_pkcs8_file(&key_path)?;
    let trusted = TrustedAuthorityKey::from_record(
        AuthorityKeyRecordV1 {
            format_version: AUTHORITY_KEY_FORMAT_VERSION,
            trust_domain_id: trust_domain_id.into(),
            key_id: signer.key_id().into(),
            role,
            public_key: signer.public_key_bytes(),
            status: AuthorityKeyStatus::Active,
            valid_from_unix_seconds,
            valid_until_unix_seconds,
            revoked_at_unix_seconds: None,
            revocation_reason: None,
            registry_generation,
        },
        &registry_config(trust_domain_id)?.trust_domain_id,
    )?;
    let wire = authority_key_to_proto(&trusted);
    atomic_write_new(&output, &wire.encode_to_vec())?;
    projection::write_new(&json_output, &projection::authority_key(&wire))?;

    println!("key_id={}", trusted.key_id());
    println!("public_key={}", lowercase_hex(&trusted.record().public_key));
    Ok(())
}
