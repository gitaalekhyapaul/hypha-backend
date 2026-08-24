use std::error::Error;
use std::fmt::{self, Debug, Formatter, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use prost::Message;
use prost_types::{Duration, Timestamp};
use raziel_authorization_core::{capability_scope_hash, issuance_nullifier};
use raziel_proto::raziel::v1::{
    ProofCapability, ProofMetrics, ProofSystem, RelationshipSnapshotDescriptor,
};
use raziel_prover::{Sp1Verifier, TrustedProgramKey};
use raziel_verifier::{
    LocalRegistry, RegistryLoader, default_registry_config, root_record_to_proto,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::admission::LOCAL_PLONK_ADMISSION_WINDOW_SECONDS;
use crate::one_shot::{OneShotMetrics, OneShotSuccess};
use crate::private_envelope::PrivateCapabilityEnvelope;

pub const TERMINAL_REGISTRY_PATH_ENV: &str = "RAZIEL_REGISTRY_PATH";
pub const TERMINAL_TRUST_DOMAIN_ID_ENV: &str = "RAZIEL_TRUST_DOMAIN_ID";
pub const TERMINAL_VERIFICATION_KEY_ID_ENV: &str = "RAZIEL_EXPECTED_VERIFICATION_KEY_ID";
pub const TERMINAL_SP1_VKEY_HASH_ENV: &str = "RAZIEL_EXPECTED_SP1_VKEY_HASH";

const PROOF_CAPABILITY_FORMAT_VERSION: u32 = 1;
const TERMINAL_RESULT_FORMAT_VERSION: u32 = 1;
const MAX_REGISTRY_PATH_BYTES: usize = 4_096;
const MAX_TERMINAL_CAPABILITY_BYTES: usize = 1024 * 1024 + 64 * 1024;
const MAX_TERMINAL_RESULT_JSON_BYTES: usize = 2_300_000;

/// Fixed owner configuration for independent result validation inside the supervisor.
pub struct TerminalValidatorConfig {
    registry_path: PathBuf,
    trust_domain_id: String,
    trusted_program_key: TrustedProgramKey,
}

impl TerminalValidatorConfig {
    pub fn from_env() -> Result<Self, TerminalValidationError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, TerminalValidationError> {
        let registry_path = lookup(TERMINAL_REGISTRY_PATH_ENV)
            .map(PathBuf::from)
            .filter(|path| valid_absolute_path(path))
            .ok_or(TerminalValidationError::Configuration)?;
        let trust_domain_id = lookup(TERMINAL_TRUST_DOMAIN_ID_ENV)
            .filter(|value| valid_identifier(value))
            .ok_or(TerminalValidationError::Configuration)?;
        let verification_key_id = lookup(TERMINAL_VERIFICATION_KEY_ID_ENV)
            .ok_or(TerminalValidationError::Configuration)?;
        let sp1_vkey_hash =
            lookup(TERMINAL_SP1_VKEY_HASH_ENV).ok_or(TerminalValidationError::Configuration)?;
        let trusted_program_key =
            TrustedProgramKey::from_registry_record(&verification_key_id, &sp1_vkey_hash)
                .map_err(|_| TerminalValidationError::Configuration)?;
        Ok(Self {
            registry_path,
            trust_domain_id,
            trusted_program_key,
        })
    }
}

impl Debug for TerminalValidatorConfig {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalValidatorConfig")
            .field("trusted_fields", &"<redacted>")
            .finish()
    }
}

/// Private in-memory context retained only until one child result is validated.
#[derive(Clone)]
pub(crate) struct TerminalIssuanceContext {
    envelope: PrivateCapabilityEnvelope,
    issued_at_unix_seconds: i64,
}

impl TerminalIssuanceContext {
    pub(crate) const fn new(
        envelope: PrivateCapabilityEnvelope,
        issued_at_unix_seconds: i64,
    ) -> Self {
        Self {
            envelope,
            issued_at_unix_seconds,
        }
    }
}

impl Debug for TerminalIssuanceContext {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalIssuanceContext")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

/// Validated public capability and safe proof metrics ready for BullMQ retention.
#[derive(Clone, PartialEq, Eq)]
pub struct TerminalCapabilityResult {
    capability: ProofCapability,
    metrics: OneShotMetrics,
}

impl TerminalCapabilityResult {
    fn new(
        capability: ProofCapability,
        metrics: OneShotMetrics,
    ) -> Result<Self, TerminalValidationError> {
        let bytes = capability.encode_to_vec();
        if bytes.len() > MAX_TERMINAL_CAPABILITY_BYTES {
            return Err(TerminalValidationError::ResultTooLarge);
        }
        let result = Self {
            capability,
            metrics,
        };
        result.to_public_value()?;
        Ok(result)
    }

    pub fn capability(&self) -> &ProofCapability {
        &self.capability
    }

    pub const fn metrics(&self) -> OneShotMetrics {
        self.metrics
    }

    pub fn proof_job_metrics(&self) -> ProofMetrics {
        ProofMetrics {
            execution_duration: Some(milliseconds_duration(self.metrics.execute_millis)),
            proving_duration: Some(milliseconds_duration(self.metrics.prove_millis)),
            execution_cycle_count: 0,
            proof_size_bytes: self.metrics.artifact_bytes,
        }
    }

    pub fn to_public_value(&self) -> Result<Value, TerminalValidationError> {
        let wire = TerminalResultWireV1 {
            format_version: TERMINAL_RESULT_FORMAT_VERSION,
            capability_protobuf_hex: lowercase_hex(&self.capability.encode_to_vec()),
            execute_millis: self.metrics.execute_millis,
            prove_millis: self.metrics.prove_millis,
            artifact_bytes: self.metrics.artifact_bytes,
            peak_memory_bytes: self.metrics.peak_memory_bytes,
            peak_swap_bytes: self.metrics.peak_swap_bytes,
        };
        let bytes = serde_json::to_vec(&wire).map_err(|_| TerminalValidationError::Encoding)?;
        if bytes.len() > MAX_TERMINAL_RESULT_JSON_BYTES {
            return Err(TerminalValidationError::ResultTooLarge);
        }
        serde_json::to_value(wire).map_err(|_| TerminalValidationError::Encoding)
    }

    /// Strictly decodes the public terminal value retained by BullMQ.
    pub(crate) fn from_public_value(
        job_id: &str,
        value: &Value,
    ) -> Result<Self, TerminalValidationError> {
        let bytes = serde_json::to_vec(value).map_err(|_| TerminalValidationError::Decoding)?;
        if bytes.len() > MAX_TERMINAL_RESULT_JSON_BYTES {
            return Err(TerminalValidationError::ResultTooLarge);
        }
        let wire: TerminalResultWireV1 =
            serde_json::from_value(value.clone()).map_err(|_| TerminalValidationError::Decoding)?;
        if wire.format_version != TERMINAL_RESULT_FORMAT_VERSION {
            return Err(TerminalValidationError::Decoding);
        }

        let capability_bytes = decode_lowercase_hex(&wire.capability_protobuf_hex)?;
        if capability_bytes.is_empty() || capability_bytes.len() > MAX_TERMINAL_CAPABILITY_BYTES {
            return Err(TerminalValidationError::ResultTooLarge);
        }
        let capability = ProofCapability::decode(capability_bytes.as_slice())
            .map_err(|_| TerminalValidationError::Decoding)?;
        if capability.encode_to_vec() != capability_bytes
            || capability.format_version != PROOF_CAPABILITY_FORMAT_VERSION
            || capability.capability_id != job_id
            || lowercase_hex(&capability.capability_request_commitment) != job_id
            || wire.artifact_bytes != capability.zk_proof.len() as u64
        {
            return Err(TerminalValidationError::Decoding);
        }

        let metrics = OneShotMetrics {
            execute_millis: wire.execute_millis,
            prove_millis: wire.prove_millis,
            artifact_bytes: wire.artifact_bytes,
            peak_memory_bytes: wire.peak_memory_bytes,
            peak_swap_bytes: wire.peak_swap_bytes,
        };
        validate_retained_metrics(metrics)?;
        Self::new(capability, metrics)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        capability: ProofCapability,
        metrics: OneShotMetrics,
    ) -> Result<Self, TerminalValidationError> {
        Self::new(capability, metrics)
    }
}

impl Debug for TerminalCapabilityResult {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalCapabilityResult")
            .field("capability", &"<redacted-proof>")
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TerminalResultWireV1 {
    format_version: u32,
    capability_protobuf_hex: String,
    execute_millis: u64,
    prove_millis: u64,
    artifact_bytes: u64,
    peak_memory_bytes: u64,
    peak_swap_bytes: u64,
}

pub(crate) trait CapabilityResultValidator: Send + Sync + 'static {
    fn validate(
        &self,
        context: TerminalIssuanceContext,
        result: OneShotSuccess,
    ) -> Result<TerminalCapabilityResult, TerminalValidationError>;
}

/// Production validator that loads signed snapshot trust and runs direct Plonk verification.
pub struct TerminalResultValidator {
    trust: Arc<dyn TerminalTrust>,
}

impl TerminalResultValidator {
    pub fn new(config: TerminalValidatorConfig) -> Result<Self, TerminalValidationError> {
        let registry_config = default_registry_config(&config.trust_domain_id)
            .map_err(|_| TerminalValidationError::Configuration)?;
        let verifier = Sp1Verifier::from_trusted_key(config.trusted_program_key);
        Ok(Self {
            trust: Arc::new(LocalTerminalTrust {
                registry_path: config.registry_path,
                registry_config,
                verifier,
            }),
        })
    }

    #[cfg(test)]
    fn from_trust(trust: Arc<dyn TerminalTrust>) -> Self {
        Self { trust }
    }
}

impl CapabilityResultValidator for TerminalResultValidator {
    fn validate(
        &self,
        context: TerminalIssuanceContext,
        result: OneShotSuccess,
    ) -> Result<TerminalCapabilityResult, TerminalValidationError> {
        let request = context.envelope.request();
        let public_values = result.public_values();
        let expected_expires_at = context
            .issued_at_unix_seconds
            .checked_add(LOCAL_PLONK_ADMISSION_WINDOW_SECONDS)
            .ok_or(TerminalValidationError::ClaimsMismatch)?;
        let expected_scope_hash =
            capability_scope_hash(request, context.issued_at_unix_seconds, expected_expires_at)
                .map_err(|_| TerminalValidationError::ClaimsMismatch)?;
        let expected_nullifier =
            issuance_nullifier(context.envelope.capability_request_commitment())
                .map_err(|_| TerminalValidationError::ClaimsMismatch)?;
        if public_values.capability_request_commitment
            != *context.envelope.capability_request_commitment()
            || public_values.capability_scope_hash != expected_scope_hash
            || public_values.issued_at_unix_seconds != context.issued_at_unix_seconds
            || public_values.expires_at_unix_seconds != expected_expires_at
            || public_values.expires_at_unix_seconds > request.requested_expires_at_unix_seconds
            || public_values.issuance_nullifier != expected_nullifier
        {
            return Err(TerminalValidationError::ClaimsMismatch);
        }

        let snapshot = self
            .trust
            .resolve_snapshot(&request.snapshot_id, context.issued_at_unix_seconds)?;
        if snapshot.snapshot_id != request.snapshot_id
            || snapshot.relationship_root != public_values.relationship_snapshot_root
            || snapshot.policy_hash != public_values.policy_hash
        {
            return Err(TerminalValidationError::TrustedState);
        }
        self.trust.verify_proof(
            result.proof_artifact(),
            result.verification_key_id(),
            result.public_values_bytes(),
        )?;

        let capability = ProofCapability {
            format_version: PROOF_CAPABILITY_FORMAT_VERSION,
            capability_id: lowercase_hex(context.envelope.capability_request_commitment()),
            agent_key_thumbprint: request.agent_key_thumbprint.to_vec(),
            task_id: request.task_id.clone(),
            purpose: request.purpose.clone(),
            action: request.action.clone(),
            resource_type: request.resource_type.clone(),
            resource_scope: request.resource_scope.clone(),
            policy_hash: public_values.policy_hash.to_vec(),
            relationship_snapshot_root: public_values.relationship_snapshot_root.to_vec(),
            snapshot_id: request.snapshot_id.clone(),
            capability_request_commitment: public_values.capability_request_commitment.to_vec(),
            issuance_nullifier: public_values.issuance_nullifier.to_vec(),
            issued_at: Some(timestamp(public_values.issued_at_unix_seconds)),
            expires_at: Some(timestamp(public_values.expires_at_unix_seconds)),
            proof_system: ProofSystem::Sp1 as i32,
            verification_key_id: result.verification_key_id().to_owned(),
            zk_proof: result.proof_artifact().to_vec(),
            snapshot_attestation: Some(snapshot.descriptor),
        };
        TerminalCapabilityResult::new(capability, result.metrics())
    }
}

struct TrustedSnapshot {
    snapshot_id: String,
    relationship_root: [u8; 32],
    policy_hash: [u8; 32],
    descriptor: RelationshipSnapshotDescriptor,
}

trait TerminalTrust: Send + Sync + 'static {
    fn resolve_snapshot(
        &self,
        snapshot_id: &str,
        observed_now_unix_seconds: i64,
    ) -> Result<TrustedSnapshot, TerminalValidationError>;

    fn verify_proof(
        &self,
        artifact: &[u8],
        verification_key_id: &str,
        expected_public_values: &[u8],
    ) -> Result<(), TerminalValidationError>;
}

struct LocalTerminalTrust {
    registry_path: PathBuf,
    registry_config: raziel_verifier::RegistryConfig,
    verifier: Sp1Verifier,
}

impl TerminalTrust for LocalTerminalTrust {
    fn resolve_snapshot(
        &self,
        snapshot_id: &str,
        observed_now_unix_seconds: i64,
    ) -> Result<TrustedSnapshot, TerminalValidationError> {
        let generation = RegistryLoader::new(self.registry_config.clone())
            .load_current(&self.registry_path, observed_now_unix_seconds)
            .map_err(|_| TerminalValidationError::TrustedState)?;
        let mut registry = LocalRegistry::new(self.registry_config.clone());
        registry
            .install_generation(generation)
            .map_err(|_| TerminalValidationError::TrustedState)?;
        let resolved = registry
            .resolve_snapshot_exact(snapshot_id, observed_now_unix_seconds)
            .map_err(|_| TerminalValidationError::TrustedState)?;
        let root_claims = resolved.root_record.claims.claims();
        let descriptor = root_record_to_proto(&resolved.root_record)
            .descriptor
            .ok_or(TerminalValidationError::TrustedState)?;
        Ok(TrustedSnapshot {
            snapshot_id: root_claims.snapshot_id.clone(),
            relationship_root: root_claims.relationship_snapshot_root,
            policy_hash: root_claims.policy_hash,
            descriptor,
        })
    }

    fn verify_proof(
        &self,
        artifact: &[u8],
        verification_key_id: &str,
        expected_public_values: &[u8],
    ) -> Result<(), TerminalValidationError> {
        self.verifier
            .verify(artifact, verification_key_id, expected_public_values)
            .map(|_| ())
            .map_err(|_| TerminalValidationError::ProofRejected)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalValidationError {
    Configuration,
    ClaimsMismatch,
    TrustedState,
    ProofRejected,
    ResultTooLarge,
    Encoding,
    Decoding,
}

impl fmt::Display for TerminalValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("proof supervisor terminal result is invalid")
    }
}

impl Error for TerminalValidationError {}

const fn timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn milliseconds_duration(milliseconds: u64) -> Duration {
    Duration {
        seconds: i64::try_from(milliseconds / 1_000).unwrap_or(i64::MAX),
        nanos: i32::try_from((milliseconds % 1_000) * 1_000_000).unwrap_or(0),
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn decode_lowercase_hex(encoded: &str) -> Result<Vec<u8>, TerminalValidationError> {
    if !encoded.len().is_multiple_of(2)
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(TerminalValidationError::Decoding);
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = decode_hex_nibble(pair[0])?;
            let low = decode_hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

const fn decode_hex_nibble(value: u8) -> Result<u8, TerminalValidationError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(TerminalValidationError::Decoding),
    }
}

fn validate_retained_metrics(metrics: OneShotMetrics) -> Result<(), TerminalValidationError> {
    let maximum_duration_millis = u64::from(crate::one_shot::PROOF_DEADLINE_SECONDS) * 1_000;
    let total_duration_millis = metrics
        .execute_millis
        .checked_add(metrics.prove_millis)
        .ok_or(TerminalValidationError::Decoding)?;
    if total_duration_millis > maximum_duration_millis
        || metrics.peak_memory_bytes > crate::one_shot::PROOF_MEMORY_LIMIT_BYTES
        || metrics.peak_swap_bytes != 0
    {
        return Err(TerminalValidationError::Decoding);
    }
    Ok(())
}

fn valid_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().as_encoded_bytes().len() <= MAX_REGISTRY_PATH_BYTES
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use raziel_authorization_core::{
        AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationPublicValuesV1, encode_public_values,
    };

    use super::*;
    use crate::one_shot::tests::private_envelope;

    const TRUSTED_NOW: i64 = 1_800_000_000;
    const POLICY_HASH: [u8; 32] = [0x11; 32];
    const RELATIONSHIP_ROOT: [u8; 32] = [0x22; 32];

    #[test]
    fn terminal_validator_builds_the_exact_public_capability() {
        let envelope = private_envelope();
        let validator = validator(false, envelope.request().snapshot_id.clone());
        let result = validator
            .validate(
                TerminalIssuanceContext::new(envelope.clone(), TRUSTED_NOW),
                success_for(&envelope, |_| {}),
            )
            .unwrap();

        let capability = result.capability();
        assert_eq!(capability.format_version, PROOF_CAPABILITY_FORMAT_VERSION);
        assert_eq!(
            capability.capability_id,
            lowercase_hex(envelope.capability_request_commitment())
        );
        assert_eq!(
            capability.agent_key_thumbprint,
            envelope.request().agent_key_thumbprint
        );
        assert_eq!(capability.task_id, envelope.request().task_id);
        assert_eq!(capability.policy_hash, POLICY_HASH);
        assert_eq!(capability.relationship_snapshot_root, RELATIONSHIP_ROOT);
        assert_eq!(capability.proof_system, ProofSystem::Sp1 as i32);
        assert_eq!(capability.zk_proof, b"validated-proof");
        assert_eq!(
            capability
                .snapshot_attestation
                .as_ref()
                .unwrap()
                .snapshot_id,
            envelope.request().snapshot_id
        );
        assert_eq!(result.proof_job_metrics().proof_size_bytes, 15);

        let public = result.to_public_value().unwrap();
        let encoded = serde_json::to_vec(&public).unwrap();
        assert!(encoded.len() < MAX_TERMINAL_RESULT_JSON_BYTES);
        assert!(!format!("{result:?}").contains("validated-proof"));
    }

    #[test]
    fn terminal_validator_rejects_every_bound_public_value_mutation() {
        let envelope = private_envelope();
        let validator = validator(false, envelope.request().snapshot_id.clone());
        let mutations: [fn(&mut AuthorizationPublicValuesV1); 7] = [
            |value| value.policy_hash[0] ^= 1,
            |value| value.relationship_snapshot_root[0] ^= 1,
            |value| value.capability_request_commitment[0] ^= 1,
            |value| value.capability_scope_hash[0] ^= 1,
            |value| value.issued_at_unix_seconds += 1,
            |value| value.expires_at_unix_seconds += 1,
            |value| value.issuance_nullifier[0] ^= 1,
        ];
        for mutate in mutations {
            assert!(
                validator
                    .validate(
                        TerminalIssuanceContext::new(envelope.clone(), TRUSTED_NOW),
                        success_for(&envelope, mutate),
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn terminal_validator_rejects_a_proof_verifier_failure() {
        let envelope = private_envelope();
        let validator = validator(true, envelope.request().snapshot_id.clone());
        assert_eq!(
            validator.validate(
                TerminalIssuanceContext::new(envelope.clone(), TRUSTED_NOW),
                success_for(&envelope, |_| {}),
            ),
            Err(TerminalValidationError::ProofRejected)
        );
    }

    #[test]
    fn terminal_validator_configuration_is_fixed_and_redacted() {
        let mut values = HashMap::from([
            (TERMINAL_REGISTRY_PATH_ENV, "/var/lib/raziel/registry"),
            (TERMINAL_TRUST_DOMAIN_ID_ENV, "raziel-local-dev"),
            (
                TERMINAL_VERIFICATION_KEY_ID_ENV,
                "sp1-vk-v1:abababababababababababababababababababababababababababababababab",
            ),
            (
                TERMINAL_SP1_VKEY_HASH_ENV,
                "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
            ),
        ]);
        let config =
            TerminalValidatorConfig::from_lookup(|name| values.get(name).map(ToString::to_string))
                .unwrap();
        assert!(format!("{config:?}").contains("<redacted>"));

        values.insert(TERMINAL_REGISTRY_PATH_ENV, "relative/path");
        assert!(matches!(
            TerminalValidatorConfig::from_lookup(|name| {
                values.get(name).map(ToString::to_string)
            }),
            Err(TerminalValidationError::Configuration)
        ));
    }

    fn validator(reject_proof: bool, snapshot_id: String) -> TerminalResultValidator {
        TerminalResultValidator::from_trust(Arc::new(FakeTerminalTrust {
            reject_proof,
            snapshot_id,
        }))
    }

    fn success_for(
        envelope: &PrivateCapabilityEnvelope,
        mutate: impl FnOnce(&mut AuthorizationPublicValuesV1),
    ) -> OneShotSuccess {
        let expires_at = TRUSTED_NOW + LOCAL_PLONK_ADMISSION_WINDOW_SECONDS;
        let mut public_values = AuthorizationPublicValuesV1 {
            format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
            policy_hash: POLICY_HASH,
            relationship_snapshot_root: RELATIONSHIP_ROOT,
            capability_request_commitment: *envelope.capability_request_commitment(),
            capability_scope_hash: capability_scope_hash(
                envelope.request(),
                TRUSTED_NOW,
                expires_at,
            )
            .unwrap(),
            issued_at_unix_seconds: TRUSTED_NOW,
            expires_at_unix_seconds: expires_at,
            issuance_nullifier: issuance_nullifier(envelope.capability_request_commitment())
                .unwrap(),
        };
        mutate(&mut public_values);
        let artifact = b"validated-proof".to_vec();
        OneShotSuccess::new(
            artifact.clone(),
            encode_public_values(&public_values).unwrap(),
            format!("sp1-vk-v1:{}", "ab".repeat(32)),
            OneShotMetrics {
                execute_millis: 400,
                prove_millis: 1_700_000,
                artifact_bytes: artifact.len() as u64,
                peak_memory_bytes: 46_301_016_064,
                peak_swap_bytes: 0,
            },
        )
        .unwrap()
    }

    struct FakeTerminalTrust {
        reject_proof: bool,
        snapshot_id: String,
    }

    impl TerminalTrust for FakeTerminalTrust {
        fn resolve_snapshot(
            &self,
            snapshot_id: &str,
            _observed_now_unix_seconds: i64,
        ) -> Result<TrustedSnapshot, TerminalValidationError> {
            if snapshot_id != self.snapshot_id {
                return Err(TerminalValidationError::TrustedState);
            }
            Ok(TrustedSnapshot {
                snapshot_id: self.snapshot_id.clone(),
                relationship_root: RELATIONSHIP_ROOT,
                policy_hash: POLICY_HASH,
                descriptor: RelationshipSnapshotDescriptor {
                    snapshot_id: self.snapshot_id.clone(),
                    relationship_snapshot_root: RELATIONSHIP_ROOT.to_vec(),
                    policy_hash: POLICY_HASH.to_vec(),
                    ..RelationshipSnapshotDescriptor::default()
                },
            })
        }

        fn verify_proof(
            &self,
            artifact: &[u8],
            verification_key_id: &str,
            expected_public_values: &[u8],
        ) -> Result<(), TerminalValidationError> {
            if self.reject_proof
                || artifact != b"validated-proof"
                || verification_key_id
                    != "sp1-vk-v1:abababababababababababababababababababababababababababababababab"
                || expected_public_values.is_empty()
            {
                return Err(TerminalValidationError::ProofRejected);
            }
            Ok(())
        }
    }
}
