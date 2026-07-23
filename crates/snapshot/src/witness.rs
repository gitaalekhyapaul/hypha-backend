use std::fmt::{self, Debug, Formatter};

use raziel_canonical::RelationshipRecord;
use raziel_policy_ir::ValidatedPolicyProgram;

use crate::{
    Digest, SnapshotError, SnapshotLimit, SnapshotLimits, derived_tree_depth,
    relationship_leaf_digest, relationship_node_digest, relationship_root,
    tree::largest_power_of_two_less_than, validation::validate_and_encode_opening_relationship,
};

pub const RELATIONSHIP_WITNESS_SET_VERSION: u32 = 1;
const WITNESS_SET_HEADER_BYTES: u64 = 4 + 32 + 4 + 4;

#[derive(Clone, PartialEq, Eq)]
pub struct RelationshipOpeningV1 {
    pub canonical_relationship: RelationshipRecord,
    pub leaf_index: u32,
    pub sibling_hashes: Vec<Digest>,
}

impl Debug for RelationshipOpeningV1 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelationshipOpeningV1")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RelationshipWitnessSetV1 {
    pub format_version: u32,
    pub snapshot_secret_salt: Digest,
    pub leaf_count: u32,
    pub entries: Vec<RelationshipOpeningV1>,
}

impl Debug for RelationshipWitnessSetV1 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelationshipWitnessSetV1")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedWitnessMetrics {
    pub relationship_count: u32,
    pub max_merkle_depth: u32,
    pub private_input_bytes: u32,
}

pub fn verify_witness_set(
    policy: &ValidatedPolicyProgram,
    witness: &RelationshipWitnessSetV1,
    expected_root: &Digest,
    snapshot_limits: SnapshotLimits,
) -> Result<VerifiedWitnessMetrics, SnapshotError> {
    if witness.format_version != RELATIONSHIP_WITNESS_SET_VERSION {
        return Err(SnapshotError::UnsupportedWitnessVersion);
    }
    if witness.entries.is_empty() {
        return Err(SnapshotError::EmptyWitnessSet);
    }
    let relationship_count = u32::try_from(witness.entries.len())
        .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::RelationshipWitnesses))?;
    if relationship_count > policy.program().limits.max_relationship_witnesses {
        return Err(SnapshotError::LimitExceeded(
            SnapshotLimit::RelationshipWitnesses,
        ));
    }
    if witness
        .entries
        .windows(2)
        .any(|pair| pair[0].leaf_index >= pair[1].leaf_index)
    {
        return Err(SnapshotError::WitnessEntriesNotStrictlySorted);
    }

    let mut private_bytes = WITNESS_SET_HEADER_BYTES;
    let mut context_entries = 0_u32;
    let mut max_depth = 0_u32;
    for entry in &witness.entries {
        let canonical = verify_opening_encoded(
            policy,
            &witness.snapshot_secret_salt,
            witness.leaf_count,
            entry,
            expected_root,
            snapshot_limits,
        )?;
        private_bytes = private_bytes
            .checked_add(opening_private_input_bytes(
                canonical.len(),
                entry.sibling_hashes.len(),
            )?)
            .ok_or(SnapshotError::ArithmeticOverflow)?;
        let entry_context_count = entry
            .canonical_relationship
            .stored_caveat
            .as_ref()
            .map_or(0, |caveat| caveat.context.len());
        let entry_context_count = u32::try_from(entry_context_count)
            .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::ContextEntries))?;
        context_entries = context_entries
            .checked_add(entry_context_count)
            .ok_or(SnapshotError::ArithmeticOverflow)?;
        if context_entries > policy.program().limits.max_context_entries {
            return Err(SnapshotError::LimitExceeded(SnapshotLimit::ContextEntries));
        }
        max_depth = max_depth.max(
            u32::try_from(entry.sibling_hashes.len())
                .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::MerkleDepth))?,
        );
    }

    enforce_private_input_limit(policy, private_bytes)?;
    let private_input_bytes =
        u32::try_from(private_bytes).map_err(|_| SnapshotError::ArithmeticOverflow)?;
    Ok(VerifiedWitnessMetrics {
        relationship_count,
        max_merkle_depth: max_depth,
        private_input_bytes,
    })
}

pub fn verify_opening(
    policy: &ValidatedPolicyProgram,
    snapshot_salt: &Digest,
    leaf_count: u32,
    opening: &RelationshipOpeningV1,
    expected_root: &Digest,
    snapshot_limits: SnapshotLimits,
) -> Result<(), SnapshotError> {
    let canonical = verify_opening_encoded(
        policy,
        snapshot_salt,
        leaf_count,
        opening,
        expected_root,
        snapshot_limits,
    )?;
    let private_bytes = WITNESS_SET_HEADER_BYTES
        .checked_add(opening_private_input_bytes(
            canonical.len(),
            opening.sibling_hashes.len(),
        )?)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    enforce_private_input_limit(policy, private_bytes)
}

fn verify_opening_encoded(
    policy: &ValidatedPolicyProgram,
    snapshot_salt: &Digest,
    leaf_count: u32,
    opening: &RelationshipOpeningV1,
    expected_root: &Digest,
    snapshot_limits: SnapshotLimits,
) -> Result<Vec<u8>, SnapshotError> {
    if leaf_count == 0 || opening.leaf_index >= leaf_count {
        return Err(SnapshotError::LeafIndexOutOfRange);
    }
    if leaf_count > snapshot_limits.max_relationships {
        return Err(SnapshotError::LimitExceeded(SnapshotLimit::Relationships));
    }
    let max_depth = derived_tree_depth(leaf_count);
    if max_depth > policy.program().limits.max_merkle_depth {
        return Err(SnapshotError::LimitExceeded(SnapshotLimit::MerkleDepth));
    }
    let path = derive_path(opening.leaf_index, leaf_count)?;
    if opening.sibling_hashes.len() != path.len() {
        return Err(SnapshotError::SiblingCountMismatch);
    }

    let canonical = validate_and_encode_opening_relationship(
        policy,
        &opening.canonical_relationship,
        snapshot_limits,
    )?;
    let mut digest = relationship_leaf_digest(opening.leaf_index, snapshot_salt, &canonical)?;
    for (current_is_left, sibling) in path.into_iter().zip(&opening.sibling_hashes) {
        digest = if current_is_left {
            relationship_node_digest(&digest, sibling)?
        } else {
            relationship_node_digest(sibling, &digest)?
        };
    }
    let root = relationship_root(leaf_count, snapshot_salt, &digest)?;
    if root != *expected_root {
        return Err(SnapshotError::RootMismatch);
    }
    Ok(canonical)
}

fn derive_path(mut relative_index: u32, mut leaf_count: u32) -> Result<Vec<bool>, SnapshotError> {
    let mut root_to_leaf = Vec::with_capacity(derived_tree_depth(leaf_count) as usize);
    while leaf_count > 1 {
        let split = u32::try_from(largest_power_of_two_less_than(leaf_count as usize))
            .map_err(|_| SnapshotError::ArithmeticOverflow)?;
        if relative_index < split {
            root_to_leaf.push(true);
            leaf_count = split;
        } else {
            root_to_leaf.push(false);
            relative_index -= split;
            leaf_count -= split;
        }
    }
    root_to_leaf.reverse();
    Ok(root_to_leaf)
}

fn opening_private_input_bytes(
    canonical_relationship_bytes: usize,
    sibling_count: usize,
) -> Result<u64, SnapshotError> {
    let record_bytes = u64::try_from(canonical_relationship_bytes)
        .map_err(|_| SnapshotError::ArithmeticOverflow)?;
    let sibling_bytes = u64::try_from(sibling_count)
        .map_err(|_| SnapshotError::ArithmeticOverflow)?
        .checked_mul(32)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    4_u64
        .checked_add(record_bytes)
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_add(sibling_bytes))
        .ok_or(SnapshotError::ArithmeticOverflow)
}

fn enforce_private_input_limit(
    policy: &ValidatedPolicyProgram,
    private_bytes: u64,
) -> Result<(), SnapshotError> {
    if private_bytes > u64::from(policy.program().limits.max_total_private_input_bytes) {
        return Err(SnapshotError::LimitExceeded(
            SnapshotLimit::TotalPrivateInputBytes,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::derive_path;

    #[test]
    fn derives_direction_free_paths_from_index_and_count() {
        assert_eq!(derive_path(0, 3).unwrap(), [true, true]);
        assert_eq!(derive_path(1, 3).unwrap(), [false, true]);
        assert_eq!(derive_path(2, 3).unwrap(), [false]);
        assert_eq!(derive_path(4, 5).unwrap(), [false]);
    }
}
