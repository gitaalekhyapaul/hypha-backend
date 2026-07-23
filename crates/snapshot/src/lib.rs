//! Policy-bound relationship records, blinded snapshot trees, and private openings.

#![forbid(unsafe_code)]

mod descriptor;
mod error;
mod hash;
mod tree;
mod validation;
mod witness;

pub use descriptor::{
    SNAPSHOT_DESCRIPTOR_FORMAT_VERSION, SNAPSHOT_DESCRIPTOR_SIGNATURE_DOMAIN,
    SnapshotDescriptorClaimsV1, SnapshotDescriptorError, SnapshotDescriptorLimits,
    ValidatedSnapshotDescriptorClaimsV1, encode_snapshot_descriptor_claims,
    snapshot_descriptor_signature_message,
};
pub use error::{PolicyReference, RelationshipField, SnapshotError, SnapshotLimit};
pub use hash::{
    RELATIONSHIP_EMPTY_DOMAIN, RELATIONSHIP_LEAF_DOMAIN, RELATIONSHIP_NODE_DOMAIN,
    RELATIONSHIP_ROOT_DOMAIN, RELATIONSHIP_TREE_VERSION, empty_tree_digest, empty_tree_preimage,
    relationship_leaf_digest, relationship_leaf_preimage, relationship_node_digest,
    relationship_node_preimage, relationship_root, relationship_root_preimage,
};
pub use raziel_canonical::{
    CANONICAL_RELATIONSHIP_ENCODING_VERSION, RelationshipRecord, StoredCaveat, StoredContextEntry,
    compare_relationship_identity, encode_relationship,
};
pub use tree::{Digest, SnapshotLimits, SnapshotTree, derived_tree_depth};
pub use witness::{
    RELATIONSHIP_WITNESS_SET_VERSION, RelationshipOpeningV1, RelationshipWitnessSetV1,
    VerifiedWitnessMetrics, verify_opening, verify_witness_set,
};
