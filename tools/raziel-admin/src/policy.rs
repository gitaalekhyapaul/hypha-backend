use prost::Message;
use raziel_authority::{AuthoritySigner, PolicyApprovalRole};
use raziel_canonical::{
    AUTHORITY_CLAIMS_FORMAT_VERSION, PolicyApprovalClaimsV1, PolicyApprovalLimits,
    ValidatedPolicyApprovalClaimsV1, policy_hash,
};
use raziel_policy_ir::decode_policy_program_strict;
use raziel_verifier::{PolicyRecordStatus, PolicyRegistryRecordV1, policy_record_to_proto};

use crate::{
    args::Arguments,
    config::{
        MAX_ARTIFACT_LIFETIME_SECONDS, MAX_RECORD_BYTES, MAX_STRING_BYTES, validation_ceilings,
    },
    error::AdminError,
    io::{atomic_write_new, lowercase_hex, read_bounded},
    projection,
};

pub fn approve(arguments: &Arguments) -> Result<(), AdminError> {
    arguments.require_known(&[
        "--policy",
        "--key",
        "--trust-domain",
        "--record-id",
        "--policy-name",
        "--policy-version",
        "--approval-epoch",
        "--valid-from",
        "--valid-until",
        "--approved-by",
        "--registered-at",
        "--generation",
        "--output",
        "--json-output",
    ])?;
    let policy_path = arguments.required_path("--policy")?;
    let key_path = arguments.required_path("--key")?;
    let trust_domain_id = arguments.required("--trust-domain")?;
    let record_id = arguments.required("--record-id")?;
    let policy_name = arguments.required("--policy-name")?;
    let policy_version = arguments.required("--policy-version")?;
    let approval_epoch = arguments.required_u64("--approval-epoch")?;
    let valid_from_unix_seconds = arguments.required_i64("--valid-from")?;
    let valid_until_unix_seconds = arguments.required_i64("--valid-until")?;
    let approved_by = arguments.required("--approved-by")?;
    let registered_at_unix_seconds = arguments.required_i64("--registered-at")?;
    let registry_generation = arguments.required_u64("--generation")?;
    let output = arguments.required_path("--output")?;
    let json_output = arguments.required_path("--json-output")?;

    let policy_bytes = read_bounded(&policy_path, MAX_RECORD_BYTES)?;
    let policy =
        decode_policy_program_strict(&policy_bytes, MAX_RECORD_BYTES, &validation_ceilings())?;
    let policy_hash = policy_hash(&policy)?;
    let signer = AuthoritySigner::<PolicyApprovalRole>::from_read_only_pkcs8_file(&key_path)?;
    let claims = ValidatedPolicyApprovalClaimsV1::new(
        PolicyApprovalClaimsV1 {
            format_version: AUTHORITY_CLAIMS_FORMAT_VERSION,
            trust_domain_id: trust_domain_id.into(),
            record_id: record_id.into(),
            policy_name: policy_name.into(),
            policy_version: policy_version.into(),
            approval_epoch,
            policy_hash,
            valid_from_unix_seconds,
            valid_until_unix_seconds,
            approved_by: approved_by.into(),
            signing_key_id: signer.key_id().into(),
        },
        PolicyApprovalLimits {
            max_string_bytes: MAX_STRING_BYTES,
            max_lifetime_seconds: MAX_ARTIFACT_LIFETIME_SECONDS,
        },
    )?;
    let authority_signature = signer.sign_policy_approval(&claims)?;
    let record = PolicyRegistryRecordV1 {
        claims,
        policy,
        canonical_policy_ir_bytes: policy_bytes,
        authority_signature,
        status: PolicyRecordStatus::Approved,
        registered_at_unix_seconds,
        revoked_at_unix_seconds: None,
        revocation_reason: None,
        registry_generation,
    };
    let wire = policy_record_to_proto(&record)?;
    atomic_write_new(&output, &wire.encode_to_vec())?;
    projection::write_new(&json_output, &projection::policy(&wire))?;

    println!("record_id={record_id}");
    println!("policy_hash={}", lowercase_hex(&policy_hash));
    Ok(())
}
