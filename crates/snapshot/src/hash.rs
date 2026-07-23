use raziel_canonical::{CanonicalEncoder, CanonicalError, framed_hash, framed_hash_preimage};

use crate::Digest;

pub const RELATIONSHIP_TREE_VERSION: u32 = 1;
pub const RELATIONSHIP_LEAF_DOMAIN: &[u8] = b"raziel/relationship-leaf/v1";
pub const RELATIONSHIP_NODE_DOMAIN: &[u8] = b"raziel/relationship-node/v1";
pub const RELATIONSHIP_EMPTY_DOMAIN: &[u8] = b"raziel/relationship-empty/v1";
pub const RELATIONSHIP_ROOT_DOMAIN: &[u8] = b"raziel/relationship-root/v1";

pub fn relationship_leaf_preimage(
    leaf_index: u32,
    snapshot_salt: &Digest,
    canonical_relationship: &[u8],
) -> Result<Vec<u8>, CanonicalError> {
    let payload = relationship_leaf_payload(leaf_index, snapshot_salt, canonical_relationship)?;
    framed_hash_preimage(RELATIONSHIP_LEAF_DOMAIN, &payload)
}

pub fn relationship_leaf_digest(
    leaf_index: u32,
    snapshot_salt: &Digest,
    canonical_relationship: &[u8],
) -> Result<Digest, CanonicalError> {
    let payload = relationship_leaf_payload(leaf_index, snapshot_salt, canonical_relationship)?;
    framed_hash(RELATIONSHIP_LEAF_DOMAIN, &payload)
}

pub fn relationship_node_preimage(
    left: &Digest,
    right: &Digest,
) -> Result<Vec<u8>, CanonicalError> {
    let payload = relationship_node_payload(left, right)?;
    framed_hash_preimage(RELATIONSHIP_NODE_DOMAIN, &payload)
}

pub fn relationship_node_digest(left: &Digest, right: &Digest) -> Result<Digest, CanonicalError> {
    let payload = relationship_node_payload(left, right)?;
    framed_hash(RELATIONSHIP_NODE_DOMAIN, &payload)
}

pub fn empty_tree_preimage() -> Result<Vec<u8>, CanonicalError> {
    let payload = empty_tree_payload();
    framed_hash_preimage(RELATIONSHIP_EMPTY_DOMAIN, &payload)
}

pub fn empty_tree_digest() -> Result<Digest, CanonicalError> {
    let payload = empty_tree_payload();
    framed_hash(RELATIONSHIP_EMPTY_DOMAIN, &payload)
}

pub fn relationship_root_preimage(
    leaf_count: u32,
    snapshot_salt: &Digest,
    tree_digest: &Digest,
) -> Result<Vec<u8>, CanonicalError> {
    let payload = relationship_root_payload(leaf_count, snapshot_salt, tree_digest)?;
    framed_hash_preimage(RELATIONSHIP_ROOT_DOMAIN, &payload)
}

pub fn relationship_root(
    leaf_count: u32,
    snapshot_salt: &Digest,
    tree_digest: &Digest,
) -> Result<Digest, CanonicalError> {
    let payload = relationship_root_payload(leaf_count, snapshot_salt, tree_digest)?;
    framed_hash(RELATIONSHIP_ROOT_DOMAIN, &payload)
}

fn relationship_leaf_payload(
    leaf_index: u32,
    snapshot_salt: &Digest,
    canonical_relationship: &[u8],
) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(RELATIONSHIP_TREE_VERSION);
    encoder.write_u32(leaf_index);
    encoder.write_bytes(snapshot_salt)?;
    encoder.write_bytes(canonical_relationship)?;
    Ok(encoder.finish())
}

fn relationship_node_payload(left: &Digest, right: &Digest) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(RELATIONSHIP_TREE_VERSION);
    encoder.write_bytes(left)?;
    encoder.write_bytes(right)?;
    Ok(encoder.finish())
}

fn empty_tree_payload() -> Vec<u8> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(RELATIONSHIP_TREE_VERSION);
    encoder.finish()
}

fn relationship_root_payload(
    leaf_count: u32,
    snapshot_salt: &Digest,
    tree_digest: &Digest,
) -> Result<Vec<u8>, CanonicalError> {
    let mut encoder = CanonicalEncoder::new();
    encoder.write_u32(RELATIONSHIP_TREE_VERSION);
    encoder.write_u32(leaf_count);
    encoder.write_bytes(snapshot_salt)?;
    encoder.write_bytes(tree_digest)?;
    Ok(encoder.finish())
}
