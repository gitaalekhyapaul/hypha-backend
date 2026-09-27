use std::collections::BTreeSet;

use prost::Message;
use raziel_policy_ir::ValidatedPolicyProgram;
use spicedb_rs_proto::authzed::api::v1 as authzed;

use super::SpiceDbSnapshotError;

const MAX_COMPOSITION_BYTES: usize = 1024 * 1024;

/// Owner-only handoff from validated installed composition and exact grant revision.
/// The caller must resolve the retained package and prove its inclusion through
/// the trusted composer. This is not an agent/browser DTO or policy approval.
#[derive(Clone)]
pub struct RequestSnapshotSource {
    pub receipt_key: String,
    pub proposal_generation: u64,
    pub policy_record_id: String,
    pub zed_token: String,
    pub source_schema_digest: [u8; 32],
    pub accepted_composition_schema: String,
}

impl std::fmt::Debug for RequestSnapshotSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RequestSnapshotSource")
            .field("owner_handoff", &"<redacted>")
            .finish()
    }
}

impl RequestSnapshotSource {
    pub(crate) fn validate(
        &self,
        policy: &ValidatedPolicyProgram,
    ) -> Result<(), SpiceDbSnapshotError> {
        if !valid_receipt_key(&self.receipt_key) || self.proposal_generation == 0 {
            return Err(SpiceDbSnapshotError::InvalidConfiguration);
        }
        validate_schema(&self.accepted_composition_schema)?;
        super::validate_zed_token(&self.zed_token)?;
        if policy.program().source_schema_digest != self.source_schema_digest {
            return Err(SpiceDbSnapshotError::SchemaMismatch);
        }
        Ok(())
    }
}

pub(crate) fn valid_receipt_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
}

pub(super) fn validate_schema(schema: &str) -> Result<(), SpiceDbSnapshotError> {
    if schema.len() > MAX_COMPOSITION_BYTES {
        return Err(SpiceDbSnapshotError::CompositionLimitExceeded);
    }
    if schema.trim().is_empty() || schema.contains('\0') {
        return Err(SpiceDbSnapshotError::SchemaMismatch);
    }
    Ok(())
}

pub(super) fn validate_reflection(
    reflected: &authzed::ReflectSchemaResponse,
    policy: &ValidatedPolicyProgram,
) -> Result<(), SpiceDbSnapshotError> {
    if reflected.encoded_len() > MAX_COMPOSITION_BYTES {
        return Err(SpiceDbSnapshotError::CompositionLimitExceeded);
    }
    let definitions: BTreeSet<_> = reflected
        .definitions
        .iter()
        .map(|value| value.name.as_str())
        .collect();
    let caveats: BTreeSet<_> = reflected
        .caveats
        .iter()
        .map(|value| value.name.as_str())
        .collect();
    if definitions.len() != reflected.definitions.len()
        || caveats.len() != reflected.caveats.len()
        || definitions.contains("")
        || caveats.contains("")
        || !definitions.is_disjoint(&caveats)
        || policy
            .program()
            .definitions
            .iter()
            .any(|definition| !definitions.contains(definition.name.as_str()))
        || policy
            .program()
            .caveats
            .iter()
            .any(|caveat| !caveats.contains(caveat.name.as_str()))
    {
        return Err(SpiceDbSnapshotError::SchemaMismatch);
    }
    Ok(())
}

pub(super) fn validate_diff(
    diff: &authzed::DiffSchemaResponse,
) -> Result<(), SpiceDbSnapshotError> {
    if diff.encoded_len() > MAX_COMPOSITION_BYTES {
        return Err(SpiceDbSnapshotError::CompositionLimitExceeded);
    }
    // Reflection omits permission expressions. The pinned server compares their
    // compiled forms at the requested revision. Reject every difference, including
    // documentation-only changes, rather than reconstructing a partial schema.
    if !diff.diffs.is_empty() {
        return Err(SpiceDbSnapshotError::SchemaMismatch);
    }
    Ok(())
}
