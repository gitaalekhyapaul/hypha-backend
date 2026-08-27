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
    policy_authority::PolicyCandidateManifestWire,
    projection,
};

const MAX_CANDIDATE_LIFETIME_SECONDS: i64 = 24 * 60 * 60;

pub fn inspect(arguments: &Arguments) -> Result<(), AdminError> {
    arguments.require_known(&[
        "--policy",
        "--candidate-id",
        "--record-id",
        "--policy-name",
        "--policy-version",
        "--resource-type",
        "--permission",
        "--rule-summary",
        "--expected-approval-epoch",
        "--valid-from",
        "--valid-until",
        "--created-at",
        "--expires-at",
        "--output",
        "--state-output",
    ])?;
    let policy_path = arguments.required_path("--policy")?;
    let candidate_id = bounded_public_identifier(arguments.required("--candidate-id")?)?;
    let record_id = bounded_public_identifier(arguments.required("--record-id")?)?;
    let policy_name = bounded_public_identifier(arguments.required("--policy-name")?)?;
    let policy_version = canonical_policy_version(arguments.required("--policy-version")?)?;
    let resource_type = schema_identifier(arguments.required("--resource-type")?)?;
    let permission = schema_identifier(arguments.required("--permission")?)?;
    let rule_summary = bounded_summary(arguments.required("--rule-summary")?)?;
    let expected_approval_epoch = arguments.required_u64("--expected-approval-epoch")?;
    if expected_approval_epoch == 0 {
        return Err(AdminError::InvalidCandidate);
    }
    let valid_from_unix_seconds = arguments.required_i64("--valid-from")?;
    let valid_until_unix_seconds = arguments.required_i64("--valid-until")?;
    validate_window(
        valid_from_unix_seconds,
        valid_until_unix_seconds,
        MAX_ARTIFACT_LIFETIME_SECONDS,
    )?;
    let created_at_unix_seconds = arguments.required_i64("--created-at")?;
    let expires_at_unix_seconds = arguments.required_i64("--expires-at")?;
    validate_window(
        created_at_unix_seconds,
        expires_at_unix_seconds,
        MAX_CANDIDATE_LIFETIME_SECONDS as u64,
    )?;
    let output = arguments.required_path("--output")?;
    let state_output = arguments.required_path("--state-output")?;
    if output.parent() != state_output.parent()
        || policy_path.parent() != state_output.parent()
        || output == state_output
        || policy_path.file_name().and_then(|value| value.to_str()) != Some("policy.pb")
        || state_output.file_name().and_then(|value| value.to_str()) != Some("candidate.pb")
    {
        return Err(AdminError::InvalidCandidate);
    }
    if output.exists() || state_output.exists() {
        return Err(AdminError::FileWrite);
    }

    let policy_bytes = read_bounded(&policy_path, MAX_RECORD_BYTES)?;
    let policy =
        decode_policy_program_strict(&policy_bytes, MAX_RECORD_BYTES, &validation_ceilings())?;
    let program = policy.program();
    if !program
        .permissions
        .iter()
        .any(|entry| entry.object_type == resource_type && entry.name == permission)
    {
        return Err(AdminError::InvalidCandidate);
    }
    let policy_hash = policy_hash(&policy)?;
    let review = projection::PolicyCandidateProjection {
        candidate_id,
        compiler_version: &program.compiler_version,
        record_id,
        policy_name,
        policy_version,
        resource_type,
        permission,
        rule_summary,
        source_schema_digest: &program.source_schema_digest,
        policy_hash: &policy_hash,
        expected_approval_epoch,
        valid_from_unix_seconds,
        valid_until_unix_seconds,
        created_at_unix_seconds,
        expires_at_unix_seconds,
    };
    let manifest = PolicyCandidateManifestWire {
        format_version: 1,
        candidate_id: candidate_id.into(),
        record_id: record_id.into(),
        policy_name: policy_name.into(),
        policy_version: policy_version.into(),
        resource_type: resource_type.into(),
        permission: permission.into(),
        rule_summary: rule_summary.into(),
        source_schema_digest: program.source_schema_digest.to_vec(),
        policy_hash: policy_hash.to_vec(),
        expected_approval_epoch,
        valid_from_unix_seconds,
        valid_until_unix_seconds,
        created_at_unix_seconds,
        expires_at_unix_seconds,
    };
    atomic_write_new(&state_output, &manifest.encode_to_vec())?;
    if let Err(error) = projection::write_new(&output, &projection::policy_candidate(&review)) {
        let _ = std::fs::remove_file(&state_output);
        return Err(error);
    }

    println!("candidate_id={candidate_id}");
    println!("policy_hash={}", lowercase_hex(&policy_hash));
    Ok(())
}

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

fn bounded_public_identifier(value: &str) -> Result<&str, AdminError> {
    if value.is_empty()
        || value.len() > MAX_STRING_BYTES as usize
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(value)
}

fn schema_identifier(value: &str) -> Result<&str, AdminError> {
    if value.is_empty()
        || value.len() > 63
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'_'))
        })
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(value)
}

fn canonical_policy_version(value: &str) -> Result<&str, AdminError> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| AdminError::InvalidCandidate)?;
    if parsed == 0 || value != parsed.to_string() {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(value)
}

fn bounded_summary(value: &str) -> Result<&str, AdminError> {
    if value.is_empty()
        || value.len() > MAX_STRING_BYTES as usize
        || value.chars().any(char::is_control)
    {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(value)
}

fn validate_window(start: i64, end: i64, maximum_seconds: u64) -> Result<(), AdminError> {
    let lifetime = end
        .checked_sub(start)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(AdminError::InvalidCandidate)?;
    if lifetime == 0 || lifetime > maximum_seconds {
        return Err(AdminError::InvalidCandidate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use prost::Message;
    use raziel_canonical::policy_hash;
    use raziel_policy_ir::decode_policy_program_strict;

    use super::inspect;
    use crate::{
        args::Arguments,
        config::{MAX_RECORD_BYTES, validation_ceilings},
        error::AdminError,
        io::lowercase_hex,
        policy_authority::PolicyCandidateManifestWire,
    };

    const POLICY_BYTES: &[u8] =
        include_bytes!("../../spicedb-ir-compiler/internal/adapter/testdata/supported.policy.pb");

    #[test]
    fn inspection_writes_only_the_safe_candidate_projection() {
        let root = temp_root();
        let policy = root.join("policy.pb");
        let output = root.join("review.json");
        let state = root.join("candidate.pb");
        std::fs::write(&policy, POLICY_BYTES).unwrap();

        inspect(&arguments(&policy, &output, &state)).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        let review: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let decoded =
            decode_policy_program_strict(POLICY_BYTES, MAX_RECORD_BYTES, &validation_ceilings())
                .unwrap();
        let expected_hash = lowercase_hex(&policy_hash(&decoded).unwrap());

        assert_eq!(review["candidate_id"], "candidate-001");
        assert_eq!(review["resource_type"], "payment_batch");
        assert_eq!(review["permission"], "view");
        assert_eq!(review["policy_hash"], expected_hash);
        assert_eq!(review["expected_approval_epoch"], "1");
        assert_eq!(review["status"], "COMPILED");
        let manifest =
            PolicyCandidateManifestWire::decode(std::fs::read(&state).unwrap().as_slice()).unwrap();
        assert_eq!(manifest.candidate_id, "candidate-001");
        assert_eq!(manifest.policy_hash, policy_hash(&decoded).unwrap());
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(&state).unwrap().permissions().mode() & 0o777,
            0o600
        );
        for forbidden in [
            "policy_ir",
            "definitions",
            "permissions",
            "caveats",
            "authority_signature",
            "relationship",
            policy.to_str().unwrap(),
        ] {
            assert!(!String::from_utf8_lossy(&bytes).contains(forbidden));
        }

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspection_rejects_malformed_policy_and_mismatched_entrypoint() {
        let root = temp_root();
        let policy = root.join("policy.pb");
        let output = root.join("review.json");
        let state = root.join("candidate.pb");
        std::fs::write(&policy, [0xff]).unwrap();
        assert!(matches!(
            inspect(&arguments(&policy, &output, &state)),
            Err(AdminError::PolicyIngress(_))
        ));
        assert!(!output.exists());

        std::fs::write(&policy, POLICY_BYTES).unwrap();
        let values = base_arguments(&policy, &output, &state)
            .into_iter()
            .map(|value| {
                if value == "view" {
                    "missing".to_owned()
                } else {
                    value
                }
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            inspect(&Arguments::parse(values).unwrap()),
            Err(AdminError::InvalidCandidate)
        ));
        assert!(!output.exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspection_rejects_invalid_window_unknown_option_and_existing_output() {
        let root = temp_root();
        let policy = root.join("policy.pb");
        let output = root.join("review.json");
        let state = root.join("candidate.pb");
        std::fs::write(&policy, POLICY_BYTES).unwrap();

        let mut invalid_window = base_arguments(&policy, &output, &state);
        replace_option(&mut invalid_window, "--valid-until", "1700000000");
        assert!(matches!(
            inspect(&Arguments::parse(invalid_window).unwrap()),
            Err(AdminError::InvalidCandidate)
        ));

        let mut unknown_option = base_arguments(&policy, &output, &state);
        unknown_option.extend(["--key".into(), "policy.pk8".into()]);
        assert!(matches!(
            inspect(&Arguments::parse(unknown_option).unwrap()),
            Err(AdminError::UnknownOption)
        ));

        std::fs::write(&output, b"preserve").unwrap();
        assert!(matches!(
            inspect(&arguments(&policy, &output, &state)),
            Err(AdminError::FileWrite)
        ));
        assert_eq!(std::fs::read(&output).unwrap(), b"preserve");

        std::fs::remove_dir_all(root).unwrap();
    }

    fn arguments(
        policy: &std::path::Path,
        output: &std::path::Path,
        state: &std::path::Path,
    ) -> Arguments {
        Arguments::parse(base_arguments(policy, output, state)).unwrap()
    }

    fn base_arguments(
        policy: &std::path::Path,
        output: &std::path::Path,
        state: &std::path::Path,
    ) -> Vec<String> {
        [
            "inspect-policy",
            "--policy",
            policy.to_str().unwrap(),
            "--candidate-id",
            "candidate-001",
            "--record-id",
            "policy-reconciliation-001",
            "--policy-name",
            "reconciliation-query",
            "--policy-version",
            "1",
            "--resource-type",
            "payment_batch",
            "--permission",
            "view",
            "--rule-summary",
            "ALL(viewer, owner)",
            "--expected-approval-epoch",
            "1",
            "--valid-from",
            "1700000000",
            "--valid-until",
            "1700086400",
            "--created-at",
            "1700000000",
            "--expires-at",
            "1700086400",
            "--output",
            output.to_str().unwrap(),
            "--state-output",
            state.to_str().unwrap(),
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    fn replace_option(values: &mut [String], option: &str, replacement: &str) {
        let index = values.iter().position(|value| value == option).unwrap();
        values[index + 1] = replacement.into();
    }

    fn temp_root() -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "raziel-admin-inspect-policy-{}-{unique}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }
}
