mod error;
mod ingress;
mod model;

use ed25519_dalek::{Signature, VerifyingKey};
use prost::Message;
use prost_types::Timestamp;
use raziel_authority::derive_ed25519_key_thumbprint;
use raziel_authorization_core::{
    AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationPublicValuesV1, CapabilityScopeBindingV1,
    TRUSTED_POLICY_CEILINGS, capability_scope_hash_from_binding, encode_public_values,
    issuance_nullifier,
};
use raziel_canonical::{
    PROTECTED_REQUEST_FORMAT_VERSION, ProtectedRequestSignatureInputV1,
    canonicalize_request_method, canonicalize_request_target, protected_request_signature_message,
};
use raziel_proto::raziel::v1::{
    ProofCapability, ProofSystem, RelationshipSnapshotDescriptor, VerifiedClaims,
    VerifyCapabilityRequest,
};
use raziel_prover::{MAX_PROOF_ARTIFACT_BYTES, TrustedProgramKey};
use raziel_verifier::{
    InstallOutcome, LocalRegistry, RegistryError, RegistryGenerationV1, root_record_to_proto,
};
use sha2::{Digest, Sha256};

pub use error::VerificationAdmissionError;
pub use ingress::{VERIFICATION_DESCRIPTOR_SHA256, decode_verify_capability_request_strict};
pub use model::{AdmittedVerificationRequest, VerificationWorkerRequest};

pub const MAX_VERIFICATION_SERVICE_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_REQUEST_AGE_SECONDS: i64 = 5 * 60;
pub const MAX_REQUEST_FUTURE_SKEW_SECONDS: i64 = 30;

const PROOF_CAPABILITY_FORMAT_VERSION: u32 = 1;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;
const DIGEST_BYTES: usize = 32;
const PROTOBUF_TIMESTAMP_MAX_SECONDS: i64 = 253_402_300_799;

/// Verifier-owned policy, snapshot, authority, and program-key state.
pub struct LocalVerificationTrust {
    registry: LocalRegistry,
    trusted_program_key: TrustedProgramKey,
}

impl LocalVerificationTrust {
    #[must_use]
    pub const fn new(registry: LocalRegistry, trusted_program_key: TrustedProgramKey) -> Self {
        Self {
            registry,
            trusted_program_key,
        }
    }

    pub fn install_generation(
        &mut self,
        generation: RegistryGenerationV1,
    ) -> Result<InstallOutcome, RegistryError> {
        self.registry.install_generation(generation)
    }
}

#[derive(Clone)]
struct TrustedCapabilityState {
    observed_now_unix_seconds: i64,
    policy_record_id: String,
    policy_valid_from_unix_seconds: i64,
    policy_valid_until_unix_seconds: i64,
    snapshot_issued_at_unix_seconds: i64,
    snapshot_expires_at_unix_seconds: i64,
    policy_hash: [u8; 32],
    relationship_snapshot_root: [u8; 32],
    descriptor: RelationshipSnapshotDescriptor,
}

trait VerificationTrust {
    fn resolve_snapshot(
        &mut self,
        snapshot_id: &str,
        trusted_now_unix_seconds: i64,
    ) -> Result<TrustedCapabilityState, VerificationAdmissionError>;

    fn trusted_program_key(&self) -> &TrustedProgramKey;
}

impl VerificationTrust for LocalVerificationTrust {
    fn resolve_snapshot(
        &mut self,
        snapshot_id: &str,
        trusted_now_unix_seconds: i64,
    ) -> Result<TrustedCapabilityState, VerificationAdmissionError> {
        let resolved = self
            .registry
            .resolve_snapshot_exact(snapshot_id, trusted_now_unix_seconds)?;
        let root_claims = resolved.root_record.claims.claims();
        let policy_claims = resolved.policy_record.claims.claims();
        let descriptor = root_record_to_proto(&resolved.root_record)
            .descriptor
            .ok_or(VerificationAdmissionError::Registry(
                RegistryError::MissingField,
            ))?;
        let observed_now_unix_seconds =
            self.registry
                .observed_time_floor()
                .ok_or(VerificationAdmissionError::Registry(
                    RegistryError::ClockRollback,
                ))?;
        Ok(TrustedCapabilityState {
            observed_now_unix_seconds,
            policy_record_id: policy_claims.record_id.clone(),
            policy_valid_from_unix_seconds: policy_claims.valid_from_unix_seconds,
            policy_valid_until_unix_seconds: policy_claims.valid_until_unix_seconds,
            snapshot_issued_at_unix_seconds: root_claims.issued_at_unix_seconds,
            snapshot_expires_at_unix_seconds: root_claims.expires_at_unix_seconds,
            policy_hash: root_claims.policy_hash,
            relationship_snapshot_root: root_claims.relationship_snapshot_root,
            descriptor,
        })
    }

    fn trusted_program_key(&self) -> &TrustedProgramKey {
        &self.trusted_program_key
    }
}

/// Validates one public verifier request without decoding or verifying its SP1 proof.
pub fn admit_verify_capability_request(
    transport: VerifyCapabilityRequest,
    trusted_now_unix_seconds: i64,
    trust: &mut LocalVerificationTrust,
) -> Result<AdmittedVerificationRequest, VerificationAdmissionError> {
    admit_with_trust(transport, trusted_now_unix_seconds, trust)
}

fn admit_with_trust<T: VerificationTrust>(
    transport: VerifyCapabilityRequest,
    trusted_now_unix_seconds: i64,
    trust: &mut T,
) -> Result<AdmittedVerificationRequest, VerificationAdmissionError> {
    if transport.encoded_len() > MAX_VERIFICATION_SERVICE_MESSAGE_BYTES {
        return Err(VerificationAdmissionError::RequestTooLarge);
    }
    validate_timestamp_seconds(trusted_now_unix_seconds)
        .map_err(|_| VerificationAdmissionError::InvalidRequestTimestamp)?;

    let VerifyCapabilityRequest {
        proof_capability,
        agent_public_key,
        request_method,
        canonical_request_target,
        request_body_hash,
        request_nonce,
        request_timestamp,
        agent_request_signature,
    } = transport;
    let capability = proof_capability.ok_or(VerificationAdmissionError::MissingCapability)?;
    let validated = validate_capability(capability, trust.trusted_program_key())?;

    let public_key: [u8; ED25519_PUBLIC_KEY_BYTES] = agent_public_key
        .as_slice()
        .try_into()
        .map_err(|_| VerificationAdmissionError::InvalidAgentPublicKeyLength)?;
    let verifying_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| VerificationAdmissionError::InvalidAgentPublicKey)?;
    if verifying_key.is_weak() {
        return Err(VerificationAdmissionError::WeakAgentPublicKey);
    }
    if derive_ed25519_key_thumbprint(&verifying_key.to_bytes()) != validated.agent_key_thumbprint {
        return Err(VerificationAdmissionError::AgentKeyMismatch);
    }

    if canonicalize_request_method(&request_method)
        .map_err(|_| VerificationAdmissionError::InvalidRequestMethod)?
        != request_method
    {
        return Err(VerificationAdmissionError::InvalidRequestMethod);
    }
    if canonicalize_request_target(&canonical_request_target)
        .map_err(|_| VerificationAdmissionError::InvalidRequestTarget)?
        != canonical_request_target
    {
        return Err(VerificationAdmissionError::InvalidRequestTarget);
    }
    let request_body_hash: [u8; DIGEST_BYTES] = request_body_hash
        .as_slice()
        .try_into()
        .map_err(|_| VerificationAdmissionError::InvalidBodyHash)?;
    let request_nonce: [u8; DIGEST_BYTES] = request_nonce
        .as_slice()
        .try_into()
        .map_err(|_| VerificationAdmissionError::InvalidRequestNonce)?;
    let request_timestamp = parse_timestamp(request_timestamp)?;
    let signature_message =
        protected_request_signature_message(&ProtectedRequestSignatureInputV1 {
            format_version: PROTECTED_REQUEST_FORMAT_VERSION,
            capability_id: &validated.capability_id,
            request_method: &request_method,
            canonical_request_target: &canonical_request_target,
            request_body_hash: &request_body_hash,
            request_nonce: &request_nonce,
            request_timestamp_unix_seconds: request_timestamp,
        })
        .map_err(|_| VerificationAdmissionError::InvalidRequestTarget)?;
    let signature_bytes: &[u8; ED25519_SIGNATURE_BYTES] = agent_request_signature
        .as_slice()
        .try_into()
        .map_err(|_| VerificationAdmissionError::InvalidRequestSignatureLength)?;
    verifying_key
        .verify_strict(&signature_message, &Signature::from_bytes(signature_bytes))
        .map_err(|_| VerificationAdmissionError::InvalidRequestSignature)?;

    let trusted = trust.resolve_snapshot(&validated.snapshot_id, trusted_now_unix_seconds)?;
    validate_request_time(
        request_timestamp,
        trusted.observed_now_unix_seconds,
        validated.issued_at_unix_seconds,
        validated.expires_at_unix_seconds,
    )?;
    validate_trusted_state(&validated, &trusted)?;

    let scope_hash = capability_scope_hash_from_binding(&CapabilityScopeBindingV1 {
        format_version: validated.format_version,
        agent_key_thumbprint: validated.agent_key_thumbprint,
        task_id: validated.task_id.clone(),
        purpose: validated.purpose.clone(),
        action: validated.action.clone(),
        resource_type: validated.resource_type.clone(),
        resource_scope: validated.resource_scope.clone(),
        snapshot_id: validated.snapshot_id.clone(),
        issued_at_unix_seconds: validated.issued_at_unix_seconds,
        expires_at_unix_seconds: validated.expires_at_unix_seconds,
    })
    .map_err(|_| VerificationAdmissionError::PublicValuesEncoding)?;
    let expected_public_values = encode_public_values(&AuthorizationPublicValuesV1 {
        format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
        policy_hash: validated.policy_hash,
        relationship_snapshot_root: validated.relationship_snapshot_root,
        capability_request_commitment: validated.capability_request_commitment,
        capability_scope_hash: scope_hash,
        issued_at_unix_seconds: validated.issued_at_unix_seconds,
        expires_at_unix_seconds: validated.expires_at_unix_seconds,
        issuance_nullifier: validated.issuance_nullifier,
    })
    .map_err(|_| VerificationAdmissionError::PublicValuesEncoding)?;
    let proof_digest = Sha256::digest(&validated.proof_artifact).into();
    let candidate_claims = validated.verified_claims();
    let snapshot_id = validated.snapshot_id.clone();
    let worker_request = VerificationWorkerRequest::new(
        validated.proof_artifact,
        expected_public_values,
        trust.trusted_program_key().clone(),
    );

    Ok(AdmittedVerificationRequest::new(
        worker_request,
        candidate_claims,
        trusted.policy_record_id,
        snapshot_id,
        proof_digest,
    ))
}

struct ValidatedCapability {
    format_version: u32,
    capability_id: String,
    agent_key_thumbprint: [u8; 32],
    task_id: String,
    purpose: String,
    action: String,
    resource_type: String,
    resource_scope: String,
    policy_hash: [u8; 32],
    relationship_snapshot_root: [u8; 32],
    snapshot_id: String,
    capability_request_commitment: [u8; 32],
    issuance_nullifier: [u8; 32],
    issued_at_unix_seconds: i64,
    expires_at_unix_seconds: i64,
    proof_artifact: Vec<u8>,
    snapshot_attestation: RelationshipSnapshotDescriptor,
}

impl ValidatedCapability {
    fn verified_claims(&self) -> VerifiedClaims {
        VerifiedClaims {
            format_version: self.format_version,
            capability_id: self.capability_id.clone(),
            agent_key_thumbprint: self.agent_key_thumbprint.to_vec(),
            task_id: self.task_id.clone(),
            purpose: self.purpose.clone(),
            action: self.action.clone(),
            resource_type: self.resource_type.clone(),
            resource_scope: self.resource_scope.clone(),
            policy_hash: self.policy_hash.to_vec(),
            relationship_snapshot_root: self.relationship_snapshot_root.to_vec(),
            snapshot_id: self.snapshot_id.clone(),
            capability_request_commitment: self.capability_request_commitment.to_vec(),
            issuance_nullifier: self.issuance_nullifier.to_vec(),
            issued_at: Some(timestamp(self.issued_at_unix_seconds)),
            expires_at: Some(timestamp(self.expires_at_unix_seconds)),
        }
    }
}

fn validate_capability(
    capability: ProofCapability,
    trusted_program_key: &TrustedProgramKey,
) -> Result<ValidatedCapability, VerificationAdmissionError> {
    if capability.format_version != PROOF_CAPABILITY_FORMAT_VERSION {
        return Err(VerificationAdmissionError::UnsupportedCapabilityVersion);
    }
    if capability.proof_system != ProofSystem::Sp1 as i32 {
        return Err(VerificationAdmissionError::UnsupportedProofSystem);
    }
    if capability.verification_key_id != trusted_program_key.verification_key_id() {
        return Err(VerificationAdmissionError::InvalidVerificationKey);
    }
    if capability.zk_proof.is_empty() {
        return Err(VerificationAdmissionError::EmptyProofArtifact);
    }
    if capability.zk_proof.len() > MAX_PROOF_ARTIFACT_BYTES {
        return Err(VerificationAdmissionError::ProofArtifactTooLarge);
    }

    let agent_key_thumbprint = fixed_digest(&capability.agent_key_thumbprint)?;
    if agent_key_thumbprint == [0; 32] {
        return Err(VerificationAdmissionError::InvalidCapabilityClaim);
    }
    let policy_hash = fixed_digest(&capability.policy_hash)?;
    let relationship_snapshot_root = fixed_digest(&capability.relationship_snapshot_root)?;
    let capability_request_commitment = fixed_digest(&capability.capability_request_commitment)?;
    let recorded_issuance_nullifier = fixed_digest(&capability.issuance_nullifier)?;
    if issuance_nullifier(&capability_request_commitment)
        .map_err(|_| VerificationAdmissionError::InvalidCapabilityClaim)?
        != recorded_issuance_nullifier
    {
        return Err(VerificationAdmissionError::InvalidCapabilityClaim);
    }
    if capability.capability_id != lowercase_hex(&capability_request_commitment) {
        return Err(VerificationAdmissionError::InvalidCapabilityId);
    }
    validate_scope_strings(&capability)?;
    let issued_at_unix_seconds = parse_capability_timestamp(capability.issued_at)?;
    let expires_at_unix_seconds = parse_capability_timestamp(capability.expires_at)?;
    if issued_at_unix_seconds >= expires_at_unix_seconds {
        return Err(VerificationAdmissionError::InvalidCapabilityClaim);
    }
    let snapshot_attestation = capability
        .snapshot_attestation
        .ok_or(VerificationAdmissionError::MissingSnapshotAttestation)?;

    Ok(ValidatedCapability {
        format_version: capability.format_version,
        capability_id: capability.capability_id,
        agent_key_thumbprint,
        task_id: capability.task_id,
        purpose: capability.purpose,
        action: capability.action,
        resource_type: capability.resource_type,
        resource_scope: capability.resource_scope,
        policy_hash,
        relationship_snapshot_root,
        snapshot_id: capability.snapshot_id,
        capability_request_commitment,
        issuance_nullifier: recorded_issuance_nullifier,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
        proof_artifact: capability.zk_proof,
        snapshot_attestation,
    })
}

fn validate_scope_strings(capability: &ProofCapability) -> Result<(), VerificationAdmissionError> {
    let max = TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize;
    for value in [
        &capability.task_id,
        &capability.purpose,
        &capability.resource_scope,
        &capability.snapshot_id,
    ] {
        if value.is_empty() || value.len() > max {
            return Err(VerificationAdmissionError::InvalidCapabilityClaim);
        }
    }
    for value in [&capability.action, &capability.resource_type] {
        if value.is_empty()
            || value.len() > max
            || !value
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(VerificationAdmissionError::InvalidCapabilityClaim);
        }
    }
    Ok(())
}

fn validate_trusted_state(
    capability: &ValidatedCapability,
    trusted: &TrustedCapabilityState,
) -> Result<(), VerificationAdmissionError> {
    if capability.policy_hash != trusted.policy_hash
        || capability.relationship_snapshot_root != trusted.relationship_snapshot_root
        || capability.snapshot_attestation != trusted.descriptor
    {
        return Err(VerificationAdmissionError::SnapshotAttestationMismatch);
    }
    if capability.issued_at_unix_seconds < trusted.policy_valid_from_unix_seconds
        || capability.expires_at_unix_seconds > trusted.policy_valid_until_unix_seconds
        || capability.issued_at_unix_seconds < trusted.snapshot_issued_at_unix_seconds
        || capability.expires_at_unix_seconds > trusted.snapshot_expires_at_unix_seconds
    {
        return Err(VerificationAdmissionError::CapabilityInactive);
    }
    Ok(())
}

fn validate_request_time(
    request_timestamp: i64,
    trusted_now: i64,
    capability_issued_at: i64,
    capability_expires_at: i64,
) -> Result<(), VerificationAdmissionError> {
    if request_timestamp < trusted_now.saturating_sub(MAX_REQUEST_AGE_SECONDS) {
        return Err(VerificationAdmissionError::RequestTooOld);
    }
    if request_timestamp > trusted_now.saturating_add(MAX_REQUEST_FUTURE_SKEW_SECONDS) {
        return Err(VerificationAdmissionError::RequestTooFarInFuture);
    }
    if request_timestamp < capability_issued_at
        || request_timestamp >= capability_expires_at
        || trusted_now < capability_issued_at
        || trusted_now >= capability_expires_at
    {
        return Err(VerificationAdmissionError::CapabilityInactive);
    }
    Ok(())
}

fn parse_timestamp(value: Option<Timestamp>) -> Result<i64, VerificationAdmissionError> {
    let value = value.ok_or(VerificationAdmissionError::MissingRequestTimestamp)?;
    if value.nanos != 0 {
        return Err(VerificationAdmissionError::InvalidRequestTimestamp);
    }
    validate_timestamp_seconds(value.seconds)?;
    Ok(value.seconds)
}

fn parse_capability_timestamp(value: Option<Timestamp>) -> Result<i64, VerificationAdmissionError> {
    let value = value.ok_or(VerificationAdmissionError::InvalidCapabilityClaim)?;
    if value.nanos != 0 || validate_timestamp_seconds(value.seconds).is_err() {
        return Err(VerificationAdmissionError::InvalidCapabilityClaim);
    }
    Ok(value.seconds)
}

fn validate_timestamp_seconds(seconds: i64) -> Result<(), VerificationAdmissionError> {
    if (0..=PROTOBUF_TIMESTAMP_MAX_SECONDS).contains(&seconds) {
        Ok(())
    } else {
        Err(VerificationAdmissionError::InvalidRequestTimestamp)
    }
}

fn fixed_digest(value: &[u8]) -> Result<[u8; 32], VerificationAdmissionError> {
    value
        .try_into()
        .map_err(|_| VerificationAdmissionError::InvalidCapabilityClaim)
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

const fn timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

#[cfg(test)]
mod tests;
