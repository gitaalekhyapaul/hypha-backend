use raziel_canonical::RelationshipRecord;
use raziel_policy_ir::ValidatedPolicyProgram;

use crate::{
    SnapshotError, SnapshotLimit, empty_tree_digest, relationship_leaf_digest,
    relationship_node_digest, relationship_root, validation::validate_relationships,
    witness::RelationshipOpeningV1,
};

pub type Digest = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotLimits {
    pub max_relationships: u32,
    pub max_relationship_bytes: u32,
    pub max_total_canonical_bytes: u64,
}

pub struct SnapshotTree {
    root: Digest,
    relationships: Vec<RelationshipRecord>,
    tree: Option<TreeNode>,
}

impl SnapshotTree {
    pub fn build(
        policy: &ValidatedPolicyProgram,
        relationships: Vec<RelationshipRecord>,
        snapshot_salt: &Digest,
        limits: SnapshotLimits,
    ) -> Result<Self, SnapshotError> {
        let canonical = validate_relationships(policy, &relationships, limits)?;
        let leaf_count = u32::try_from(relationships.len())
            .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::Relationships))?;
        let depth = derived_tree_depth(leaf_count);
        if depth > policy.program().limits.max_merkle_depth {
            return Err(SnapshotError::LimitExceeded(SnapshotLimit::MerkleDepth));
        }

        let leaf_digests = canonical
            .iter()
            .enumerate()
            .map(|(index, relationship)| {
                let index = u32::try_from(index)
                    .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::Relationships))?;
                relationship_leaf_digest(index, snapshot_salt, relationship)
                    .map_err(SnapshotError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tree = if leaf_digests.is_empty() {
            None
        } else {
            Some(TreeNode::build(&leaf_digests)?)
        };
        let tree_digest = match &tree {
            Some(tree) => tree.digest(),
            None => empty_tree_digest()?,
        };
        let root = relationship_root(leaf_count, snapshot_salt, &tree_digest)?;

        Ok(Self {
            root,
            relationships,
            tree,
        })
    }

    pub const fn root(&self) -> Digest {
        self.root
    }

    pub fn opening(&self, leaf_index: u32) -> Result<RelationshipOpeningV1, SnapshotError> {
        let Some(tree) = &self.tree else {
            return Err(SnapshotError::EmptySnapshotOpening);
        };
        let count = u32::try_from(self.relationships.len())
            .map_err(|_| SnapshotError::LimitExceeded(SnapshotLimit::Relationships))?;
        if leaf_index >= count {
            return Err(SnapshotError::LeafIndexOutOfRange);
        }

        let mut sibling_hashes = Vec::with_capacity(derived_tree_depth(count) as usize);
        tree.collect_siblings(leaf_index, count, &mut sibling_hashes)?;
        let relationship = self
            .relationships
            .get(leaf_index as usize)
            .ok_or(SnapshotError::LeafIndexOutOfRange)?
            .clone();
        Ok(RelationshipOpeningV1 {
            canonical_relationship: relationship,
            leaf_index,
            sibling_hashes,
        })
    }
}

enum TreeNode {
    Leaf(Digest),
    Branch {
        digest: Digest,
        left: Box<Self>,
        right: Box<Self>,
    },
}

impl TreeNode {
    fn build(leaves: &[Digest]) -> Result<Self, SnapshotError> {
        if leaves.len() == 1 {
            return Ok(Self::Leaf(leaves[0]));
        }
        let split = largest_power_of_two_less_than(leaves.len());
        let left = Box::new(Self::build(&leaves[..split])?);
        let right = Box::new(Self::build(&leaves[split..])?);
        let digest = relationship_node_digest(&left.digest(), &right.digest())?;
        Ok(Self::Branch {
            digest,
            left,
            right,
        })
    }

    const fn digest(&self) -> Digest {
        match self {
            Self::Leaf(digest) | Self::Branch { digest, .. } => *digest,
        }
    }

    fn collect_siblings(
        &self,
        relative_index: u32,
        leaf_count: u32,
        siblings: &mut Vec<Digest>,
    ) -> Result<(), SnapshotError> {
        match self {
            Self::Leaf(_) => {
                if leaf_count != 1 || relative_index != 0 {
                    return Err(SnapshotError::LeafIndexOutOfRange);
                }
                Ok(())
            }
            Self::Branch { left, right, .. } => {
                let split = largest_power_of_two_less_than(leaf_count as usize) as u32;
                if relative_index < split {
                    left.collect_siblings(relative_index, split, siblings)?;
                    siblings.push(right.digest());
                } else {
                    right.collect_siblings(relative_index - split, leaf_count - split, siblings)?;
                    siblings.push(left.digest());
                }
                Ok(())
            }
        }
    }
}

pub const fn derived_tree_depth(leaf_count: u32) -> u32 {
    if leaf_count <= 1 {
        0
    } else {
        u32::BITS - (leaf_count - 1).leading_zeros()
    }
}

pub(crate) fn largest_power_of_two_less_than(value: usize) -> usize {
    debug_assert!(value > 1);
    1_usize << (usize::BITS - 1 - (value - 1).leading_zeros())
}

#[cfg(test)]
mod tests {
    use super::{derived_tree_depth, largest_power_of_two_less_than};

    #[test]
    fn derives_left_balanced_splits_and_maximum_depths() {
        assert_eq!(largest_power_of_two_less_than(2), 1);
        assert_eq!(largest_power_of_two_less_than(3), 2);
        assert_eq!(largest_power_of_two_less_than(4), 2);
        assert_eq!(largest_power_of_two_less_than(5), 4);
        assert_eq!(derived_tree_depth(0), 0);
        assert_eq!(derived_tree_depth(1), 0);
        assert_eq!(derived_tree_depth(2), 1);
        assert_eq!(derived_tree_depth(3), 2);
        assert_eq!(derived_tree_depth(5), 3);
    }
}
