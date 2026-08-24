use std::collections::BTreeMap;

use raziel_canonical::{
    CanonicalEncoder, encode_policy, framed_hash, policy_hash as compute_policy_hash,
};
use raziel_policy_ir::{
    CaveatInputSource, CaveatValue, CaveatValueType, ContextEntry, EvaluationRequest, ObjectRef,
    RelationshipCaveat, RelationshipResolver, ResolvedRelationship, ResolverError,
    ValidatedPolicyProgram, evaluate_with_usage,
};
use raziel_snapshot::{Digest, RelationshipWitnessSetV1, verify_witness_set};

use crate::{
    AUTHORIZATION_GUEST_INPUT_VERSION, AUTHORIZATION_PUBLIC_VALUES_VERSION, AuthorizationError,
    AuthorizationGuestInputV1, AuthorizationPublicValuesV1, AuthorizationResult,
    CapabilityRequestBindingV1, CapabilityScopeBindingV1, MAX_CANONICAL_POLICY_BYTES,
    TRUSTED_POLICY_CEILINGS, TRUSTED_SNAPSHOT_LIMITS, encode_public_values,
};

pub const CAPABILITY_REQUEST_DOMAIN: &[u8] = b"raziel/capability-request/v1";
pub const CAPABILITY_SCOPE_DOMAIN: &[u8] = b"raziel/capability-scope/v1";
pub const ISSUANCE_NULLIFIER_DOMAIN: &[u8] = b"raziel/issuance-nullifier/v1";

pub fn authorize(
    input: &AuthorizationGuestInputV1,
) -> Result<AuthorizationResult, AuthorizationError> {
    validate_input_header(input)?;
    let policy = ValidatedPolicyProgram::new(input.policy.clone(), &TRUSTED_POLICY_CEILINGS)
        .map_err(|_| AuthorizationError::InvalidPolicy)?;
    let canonical_policy =
        encode_policy(&policy).map_err(|_| AuthorizationError::CanonicalEncoding)?;
    if canonical_policy.len() > MAX_CANONICAL_POLICY_BYTES {
        return Err(AuthorizationError::PolicyTooLarge);
    }
    let policy_hash =
        compute_policy_hash(&policy).map_err(|_| AuthorizationError::CanonicalEncoding)?;
    if policy_hash != input.expected_policy_hash {
        return Err(AuthorizationError::PolicyHashMismatch);
    }

    verify_witness_set(
        &policy,
        &input.relationship_witness_set,
        &input.relationship_snapshot_root,
        TRUSTED_SNAPSHOT_LIMITS,
    )
    .map_err(|_| AuthorizationError::InvalidWitness)?;

    let resolver = VerifiedWitnessResolver::new(&input.relationship_witness_set);
    let request = authorization_evaluation_request(
        &policy,
        &input.request,
        input.issued_at_unix_seconds,
        input.expires_at_unix_seconds,
    )?;
    let outcome = evaluate_with_usage(&policy, &request, &resolver)
        .map_err(|_| AuthorizationError::EvaluationFailed)?;
    if !outcome.allowed {
        return Err(AuthorizationError::AuthorizationDenied);
    }
    let supplied_ids: Vec<_> = input
        .relationship_witness_set
        .entries
        .iter()
        .map(|entry| entry.leaf_index)
        .collect();
    if outcome.used_witness_ids != supplied_ids {
        return Err(AuthorizationError::InvalidWitnessUsage);
    }

    let capability_request_commitment = capability_request_commitment(&input.request)?;
    let capability_scope_hash = capability_scope_hash(
        &input.request,
        input.issued_at_unix_seconds,
        input.expires_at_unix_seconds,
    )?;
    let issuance_nullifier = issuance_nullifier(&capability_request_commitment)?;
    let public_values = AuthorizationPublicValuesV1 {
        format_version: AUTHORIZATION_PUBLIC_VALUES_VERSION,
        policy_hash,
        relationship_snapshot_root: input.relationship_snapshot_root,
        capability_request_commitment,
        capability_scope_hash,
        issued_at_unix_seconds: input.issued_at_unix_seconds,
        expires_at_unix_seconds: input.expires_at_unix_seconds,
        issuance_nullifier,
    };
    let public_values_bytes = encode_public_values(&public_values)?;
    Ok(AuthorizationResult {
        public_values,
        public_values_bytes,
    })
}

pub fn agent_subject(agent_key_thumbprint: &Digest) -> ObjectRef {
    ObjectRef {
        object_type: "agent".into(),
        object_id: lowercase_hex(agent_key_thumbprint),
    }
}

pub fn capability_request_commitment(
    request: &CapabilityRequestBindingV1,
) -> Result<Digest, AuthorizationError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(request.format_version);
    write_digest(&mut encoder, &request.agent_key_thumbprint)?;
    write_string(&mut encoder, &request.task_id)?;
    write_string(&mut encoder, &request.purpose)?;
    write_string(&mut encoder, &request.action)?;
    write_string(&mut encoder, &request.resource_type)?;
    write_string(&mut encoder, &request.resource_scope)?;
    encoder.write_i64(request.requested_expires_at_unix_seconds);
    write_digest(&mut encoder, &request.issuance_nonce)?;
    write_string(&mut encoder, &request.snapshot_id)?;
    encoder
        .write_collection(&request.request_context, encode_context_entry)
        .map_err(|_| AuthorizationError::CanonicalEncoding)?;
    framed_hash(CAPABILITY_REQUEST_DOMAIN, &encoder.finish())
        .map_err(|_| AuthorizationError::CanonicalEncoding)
}

pub fn capability_scope_hash(
    request: &CapabilityRequestBindingV1,
    issued_at_unix_seconds: i64,
    expires_at_unix_seconds: i64,
) -> Result<Digest, AuthorizationError> {
    capability_scope_hash_from_binding(&CapabilityScopeBindingV1 {
        format_version: request.format_version,
        agent_key_thumbprint: request.agent_key_thumbprint,
        task_id: request.task_id.clone(),
        purpose: request.purpose.clone(),
        action: request.action.clone(),
        resource_type: request.resource_type.clone(),
        resource_scope: request.resource_scope.clone(),
        snapshot_id: request.snapshot_id.clone(),
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    })
}

pub fn capability_scope_hash_from_binding(
    binding: &CapabilityScopeBindingV1,
) -> Result<Digest, AuthorizationError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(binding.format_version);
    write_digest(&mut encoder, &binding.agent_key_thumbprint)?;
    write_string(&mut encoder, &binding.task_id)?;
    write_string(&mut encoder, &binding.purpose)?;
    write_string(&mut encoder, &binding.action)?;
    write_string(&mut encoder, &binding.resource_type)?;
    write_string(&mut encoder, &binding.resource_scope)?;
    write_string(&mut encoder, &binding.snapshot_id)?;
    encoder.write_i64(binding.issued_at_unix_seconds);
    encoder.write_i64(binding.expires_at_unix_seconds);
    framed_hash(CAPABILITY_SCOPE_DOMAIN, &encoder.finish())
        .map_err(|_| AuthorizationError::CanonicalEncoding)
}

pub fn issuance_nullifier(
    capability_request_commitment: &Digest,
) -> Result<Digest, AuthorizationError> {
    let mut encoder = CanonicalEncoder::new();
    write_digest(&mut encoder, capability_request_commitment)?;
    framed_hash(ISSUANCE_NULLIFIER_DOMAIN, &encoder.finish())
        .map_err(|_| AuthorizationError::CanonicalEncoding)
}

fn validate_input_header(input: &AuthorizationGuestInputV1) -> Result<(), AuthorizationError> {
    if input.format_version != AUTHORIZATION_GUEST_INPUT_VERSION {
        return Err(AuthorizationError::UnsupportedInputVersion);
    }
    validate_capability_request_binding(&input.request)?;

    let request = &input.request;
    if input.issued_at_unix_seconds < 0
        || input.issued_at_unix_seconds >= input.expires_at_unix_seconds
        || input.expires_at_unix_seconds > request.requested_expires_at_unix_seconds
    {
        return Err(AuthorizationError::InvalidTimestamp);
    }
    Ok(())
}

/// Validates the proof-critical fields shared by service admission and the guest.
pub fn validate_capability_request_binding(
    request: &CapabilityRequestBindingV1,
) -> Result<(), AuthorizationError> {
    if request.format_version != AUTHORIZATION_GUEST_INPUT_VERSION {
        return Err(AuthorizationError::UnsupportedRequestVersion);
    }
    if request.agent_key_thumbprint == [0; 32] || request.issuance_nonce == [0; 32] {
        return Err(AuthorizationError::InvalidRequest);
    }
    validate_nonempty_string(&request.task_id)?;
    validate_nonempty_string(&request.purpose)?;
    validate_identifier(&request.action)?;
    validate_identifier(&request.resource_type)?;
    validate_nonempty_string(&request.resource_scope)?;
    validate_nonempty_string(&request.snapshot_id)?;
    validate_context(&request.request_context)
}

/// Maps one validated capability request into the PolicyIR evaluation model.
///
/// Service callers must obtain the request from the strict admission and private
/// envelope path. `authorize` validates the same request again inside the guest
/// statement.
pub fn authorization_evaluation_request(
    policy: &ValidatedPolicyProgram,
    request: &CapabilityRequestBindingV1,
    issued_at_unix_seconds: i64,
    expires_at_unix_seconds: i64,
) -> Result<EvaluationRequest, AuthorizationError> {
    Ok(EvaluationRequest {
        resource: ObjectRef {
            object_type: request.resource_type.clone(),
            object_id: request.resource_scope.clone(),
        },
        subject: agent_subject(&request.agent_key_thumbprint),
        permission: request.action.clone(),
        request_context: request.request_context.clone(),
        capability_claims: derive_capability_claims(
            policy,
            request,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
        )?,
    })
}

fn derive_capability_claims(
    policy: &ValidatedPolicyProgram,
    request: &CapabilityRequestBindingV1,
    issued_at_unix_seconds: i64,
    expires_at_unix_seconds: i64,
) -> Result<Vec<ContextEntry>, AuthorizationError> {
    let mut required = BTreeMap::new();
    for caveat in &policy.program().caveats {
        for parameter in caveat
            .parameters
            .iter()
            .filter(|parameter| parameter.source == CaveatInputSource::CapabilityClaim)
        {
            if let Some(existing) = required.insert(parameter.name.as_str(), parameter.value_type)
                && existing != parameter.value_type
            {
                return Err(AuthorizationError::InvalidPolicy);
            }
        }
    }

    required
        .into_iter()
        .map(|(name, value_type)| {
            let value = match name {
                "action" => string_claim(&request.action, value_type)?,
                "agent_key_thumbprint" => {
                    string_claim(&lowercase_hex(&request.agent_key_thumbprint), value_type)?
                }
                "expires_at" => int_claim(expires_at_unix_seconds, value_type)?,
                "issued_at" => int_claim(issued_at_unix_seconds, value_type)?,
                "purpose" => string_claim(&request.purpose, value_type)?,
                "requested_expires_at" => {
                    int_claim(request.requested_expires_at_unix_seconds, value_type)?
                }
                "resource_scope" => string_claim(&request.resource_scope, value_type)?,
                "resource_type" => string_claim(&request.resource_type, value_type)?,
                "snapshot_id" => string_claim(&request.snapshot_id, value_type)?,
                "task_id" => string_claim(&request.task_id, value_type)?,
                _ => return Err(AuthorizationError::InvalidPolicy),
            };
            Ok(ContextEntry {
                name: name.into(),
                value,
            })
        })
        .collect()
}

fn string_claim(
    value: &str,
    value_type: CaveatValueType,
) -> Result<CaveatValue, AuthorizationError> {
    if value_type == CaveatValueType::String {
        Ok(CaveatValue::String(value.into()))
    } else {
        Err(AuthorizationError::InvalidPolicy)
    }
}

const fn int_claim(
    value: i64,
    value_type: CaveatValueType,
) -> Result<CaveatValue, AuthorizationError> {
    if matches!(value_type, CaveatValueType::Int) {
        Ok(CaveatValue::Int(value))
    } else {
        Err(AuthorizationError::InvalidPolicy)
    }
}

fn validate_context(context: &[ContextEntry]) -> Result<(), AuthorizationError> {
    if context.len() > TRUSTED_POLICY_CEILINGS.policy_limits.max_context_entries as usize
        || context.windows(2).any(|pair| pair[0].name >= pair[1].name)
    {
        return Err(AuthorizationError::InvalidContext);
    }
    for entry in context {
        validate_identifier(&entry.name).map_err(|_| AuthorizationError::InvalidContext)?;
        if let CaveatValue::String(value) = &entry.value {
            validate_string(value).map_err(|_| AuthorizationError::InvalidContext)?;
        }
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), AuthorizationError> {
    validate_string(value)?;
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(AuthorizationError::InvalidRequest);
    };
    if first.is_ascii_lowercase()
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        Ok(())
    } else {
        Err(AuthorizationError::InvalidRequest)
    }
}

fn validate_nonempty_string(value: &str) -> Result<(), AuthorizationError> {
    validate_string(value)?;
    if value.is_empty() {
        Err(AuthorizationError::InvalidRequest)
    } else {
        Ok(())
    }
}

fn validate_string(value: &str) -> Result<(), AuthorizationError> {
    if value.len() <= TRUSTED_POLICY_CEILINGS.policy_limits.max_string_bytes as usize {
        Ok(())
    } else {
        Err(AuthorizationError::InvalidRequest)
    }
}

fn encode_context_entry(
    encoder: &mut CanonicalEncoder,
    entry: &ContextEntry,
) -> Result<(), raziel_canonical::CanonicalError> {
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

fn write_digest(encoder: &mut CanonicalEncoder, digest: &Digest) -> Result<(), AuthorizationError> {
    encoder
        .write_bytes(digest)
        .map_err(|_| AuthorizationError::CanonicalEncoding)
}

fn write_string(encoder: &mut CanonicalEncoder, value: &str) -> Result<(), AuthorizationError> {
    encoder
        .write_str(value)
        .map_err(|_| AuthorizationError::CanonicalEncoding)
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

#[derive(Default)]
struct VerifiedWitnessResolver {
    relationships: BTreeMap<(ObjectRef, String), Vec<ResolvedRelationship>>,
}

impl VerifiedWitnessResolver {
    fn new(witness: &RelationshipWitnessSetV1) -> Self {
        let mut resolver = Self::default();
        for opening in &witness.entries {
            let relationship = &opening.canonical_relationship;
            let resolved = ResolvedRelationship {
                subject: ObjectRef {
                    object_type: relationship.subject_type.clone(),
                    object_id: relationship.subject_id.clone(),
                },
                subject_relation: relationship.subject_relation.clone(),
                caveat: relationship
                    .stored_caveat
                    .as_ref()
                    .map(|caveat| RelationshipCaveat {
                        name: caveat.name.clone(),
                        context: caveat
                            .context
                            .iter()
                            .map(|entry| ContextEntry {
                                name: entry.name.clone(),
                                value: entry.value.clone(),
                            })
                            .collect(),
                    }),
                witness_id: Some(opening.leaf_index),
            };
            resolver
                .relationships
                .entry((
                    ObjectRef {
                        object_type: relationship.resource_type.clone(),
                        object_id: relationship.resource_id.clone(),
                    },
                    relationship.relation.clone(),
                ))
                .or_default()
                .push(resolved);
        }
        resolver
    }
}

impl RelationshipResolver for VerifiedWitnessResolver {
    fn resolve<'a>(
        &'a self,
        resource: &ObjectRef,
        relation: &str,
    ) -> Result<&'a [ResolvedRelationship], ResolverError> {
        Ok(self
            .relationships
            .get(&(resource.clone(), relation.into()))
            .map_or(&[], Vec::as_slice))
    }
}
