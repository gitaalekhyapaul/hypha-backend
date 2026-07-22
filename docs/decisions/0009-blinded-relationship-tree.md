# ADR-0009: Blinded Relationship Tree

- Status: proposed
- Date: 2026-07-22
- Milestone: M2
- Maintainer approval: pending

## Context

Raziel must commit one exact SpiceDB relationship snapshot without publishing tuples,
stored caveat values, or Merkle openings. The snapshot root becomes a public proof
input. The tuple records, one positive path, and its membership openings stay private
to Raziel and the SP1 guest.

The architecture requires a private per-snapshot salt in every leaf. It also requires
leaf ordering by an unblinded canonical relationship key. The current technical
specification does not fix record bytes, tree shape, empty trees, duplicate handling,
or how an opening proves its index belongs to the committed leaf range.

A duplicate-last tree without a committed leaf count has an index alias. For an odd
last leaf, the duplicated digest can validate as both the real even index and a phantom
adjacent odd index. A guest that checks only duplicate indices can then count the same
tuple twice under two index values.

## Proposed Decision

### Hash framing

Use SHA-256 with the ADR-0008 frame for every relationship-tree hash:

```text
H(domain, payload) = SHA-256(
  u32be(len(domain)) ||
  domain ||
  u32be(len(payload)) ||
  payload
)
```

Every length must fit in `u32`. Each payload starts with `u32be(format_version = 1)`.
Raw Protobuf and JSON bytes never enter a commitment.

Use four independent ASCII domains:

```text
raziel/relationship-leaf/v1
raziel/relationship-node/v1
raziel/relationship-empty/v1
raziel/relationship-root/v1
```

### Canonical relationship record

Version 1 represents one relationship as:

```text
RelationshipRecord {
  resource_type
  resource_id
  relation
  subject_type
  subject_id
  subject_relation?
  stored_caveat?
}

StoredCaveat {
  name
  context[]
}

StoredContextEntry {
  name
  value: bool | i64 | string
}
```

Encode fields in the declaration order above:

```text
u32be(canonical_relationship_version = 1)
string(resource_type)
string(resource_id)
string(relation)
string(subject_type)
string(subject_id)
optional(string(subject_relation))
optional(
  string(caveat_name) ||
  collection(context_entries)
)
```

Each context entry encodes its name, then one value tag and payload:

| Tag | Value payload |
| --- | --- |
| `01` | Boolean byte `00` or `01` |
| `02` | Eight-byte two's-complement big-endian integer |
| `03` | Length-prefixed exact UTF-8 string |

Strings and collections use ADR-0008 `u32` length and count prefixes. Option tags are
`00` for absent and `01` for present. Unknown tags and fields fail closed. Version 1
performs no Unicode normalization.

Version 1 does not represent relationship expiration or other SpiceDB traits. A
snapshot containing an unsupported trait fails rather than dropping it.

The canonical integer payload covers all `i64` values. The baseline SpiceDB snapshot
ingress uses `google.protobuf.Value` numbers and accepts stored integers only in the
exact JSON range `-(2^53 - 1)` through `2^53 - 1`. It rejects non-finite,
non-integral, or larger source numbers rather than guessing the original integer.
Primitive canonical tests still cover the complete `i64` encoding.

### Relationship validation

Tree construction accepts relationships only after validation against the exact
approved `ValidatedPolicyProgram` bound to the snapshot:

- Object types, relation names, subject types, subject relations, and caveat names must
  resolve through PolicyIR.
- Object identifiers must be non-empty valid UTF-8, contain no NUL byte, and fit the
  policy string limit.
- Schema identifiers use the approved lowercase ASCII identifier grammar.
- The subject type, optional subject relation, and optional caveat must match one
  allowed subject form for the relation.
- A subject relation and caveat cannot appear together in the baseline subset.
- Stored context entries are strictly sorted by parameter name and contain no
  duplicate name.
- Every stored entry must map to a relationship-context caveat parameter with the same
  scalar type.
- Every relationship-context parameter declared by that caveat must be present.
- Request-context and capability-claim parameters must not be stored in the tuple.
- Context count and string size must stay within the approved PolicyIR limits.
- Canonical record size, relationship count, total canonical bytes, and tree depth must
  stay within explicit caller ceilings.

The library returns typed value-safe errors. Errors and logs must not include object
identifiers, caveat values, canonical record bytes, salts, leaves, or openings.

### Ordering and duplicate identity

Context entries sort by exact UTF-8 parameter-name bytes.

The relationship sort and duplicate key is this structural identity:

```text
resource_type
resource_id
relation
subject_type
subject_id
subject_relation?
```

Compare each string by exact UTF-8 bytes in declaration order. Order an absent subject
relation before a present subject relation. Relationship input must be strictly
increasing by this comparison. The tree builder does not silently repair order.

Caveat name and context are committed tuple data but do not alter identity. Two
records with the same identity are duplicates even if their caveat or stored context
differs. Duplicate identities fail before hashing.

### Explicit limits and derived accounting

Tree construction requires caller-supplied `SnapshotLimits` with no production
default:

```text
SnapshotLimits {
  max_relationships: u32
  max_relationship_bytes: u32
  max_total_canonical_bytes: u64
}
```

The builder checks each canonical record length, the checked sum of all canonical
record lengths, and relationship count before allocating tree levels. It derives tree
depth from the exact count and rejects a depth above the approved PolicyIR
`max_merkle_depth`.

The future guest derives Merkle depth and private byte use from decoded records,
shared witness metadata, and exact sibling counts. It never trusts caller-reported
`merkle_depth` or `private_input_bytes` fields.

`PolicyLimits.max_total_private_input_bytes` applies to the selected witness set, not
the complete snapshot tree. The guest includes canonical selected records, the shared
salt and count, indices, and sibling digests in checked private-byte accounting.

B-0008 vectors and tests use `256` relationships, `4096` bytes per relationship, and
`1_048_576` total canonical bytes as test-only ceilings. Production ceilings remain
deferred until M3 cycle and memory measurements.

### Snapshot salt

The production snapshot builder creates one fresh 32-byte salt from an operating
system cryptographic random source for each snapshot. It stores that salt only with
private snapshot material. Salt reuse is a security error.

The deterministic tree library accepts an exact `[u8; 32]` supplied by its caller. It
cannot measure randomness or uniqueness. Fixed salts are permitted only in synthetic
tests and published test vectors.

Tree construction consumes the salt by reference. The public tree result contains no
salt accessor and stores no salt after root construction. A private witness set stores
the salt once for all selected openings from that snapshot.

### Leaf hash

For canonical relationship bytes `R`, zero-based position `i`, and snapshot salt `S`:

```text
leaf_payload =
  u32be(1) ||
  u32be(i) ||
  bytes(S) ||
  bytes(R)

leaf = H("raziel/relationship-leaf/v1", leaf_payload)
```

The two byte strings include their `u32` lengths. The salt is identical for every leaf
in one snapshot. Distinct snapshots use distinct salts. Index binding makes the leaf's
position explicit and prevents the same record digest from occupying two positions.

### Internal nodes and left-balanced shape

For child digests `L` and `R`:

```text
node_payload =
  u32be(1) ||
  bytes(L) ||
  bytes(R)

node = H("raziel/relationship-node/v1", node_payload)
```

Both child values are exact `[u8; 32]` digests. Child order is semantic and never
sorted.

Use one recursive left-balanced shape with no padding or duplicated leaf:

1. A one-leaf subtree has that leaf as its tree digest.
2. For a subtree with `n > 1` leaves, choose `k` as the largest power of two strictly
   less than `n`.
3. Build the left child from the first `k` leaves and the right child from the
   remaining `n - k` leaves.
4. Hash the ordered left and right child digests through the node domain.

This shape is deterministic for every count. It has no phantom padded position. A
count of three forms `node(node(leaf_0, leaf_1), leaf_2)`.

### Empty tree and count-bound root

An empty relationship snapshot is valid but has no membership opening. Its private
tree digest is:

```text
empty_payload =
  u32be(1)

tree_digest = H("raziel/relationship-empty/v1", empty_payload)
```

For empty and non-empty trees, bind the exact leaf count into the public root:

```text
root_payload =
  u32be(1) ||
  u32be(leaf_count) ||
  bytes(snapshot_salt) ||
  bytes(tree_digest)

relationship_snapshot_root =
  H("raziel/relationship-root/v1", root_payload)
```

`leaf_count` must fit in `u32`. The salt and count remain private witness metadata. The
descriptor publishes only the exact `[u8; 32]` root. Direct salt binding requires one
common snapshot salt across every selected opening. Count binding commits the tree
shape without exposing the count directly.

`RelationshipSnapshotDescriptor.canonicalization_version` equals this complete
relationship-tree protocol version, `1`.

### Membership opening

A private witness stores shared snapshot metadata once:

```text
RelationshipWitnessSetV1 {
  format_version = 1
  snapshot_secret_salt: [u8; 32]
  leaf_count: u32
  entries[]
}

RelationshipOpeningV1 {
  canonical_relationship
  leaf_index: u32
  sibling_hashes: [u8; 32][]
}
```

Entries must be strictly ordered by `leaf_index`. Duplicate indices fail. Directions
are not carried. The structural path derived from `leaf_index` and `leaf_count`
determines every child side and the exact sibling count.

Opening verification performs these steps:

1. Require witness-set version 1, `leaf_count > 0`, and `leaf_index < leaf_count`.
2. Validate and canonically encode the relationship.
3. Recompute its salted, index-bound leaf.
4. Recursively derive the unique left-balanced path from `leaf_index` and
   `leaf_count`. At each subtree, use the largest power of two below the subtree count
   to decide the child side and next relative index.
5. Require one exact 32-byte sibling for each derived path step. Reject missing, extra,
   or malformed siblings and paths above the trusted depth ceiling.
6. Consume siblings from leaf to root and hash each ordered child pair.
7. Bind the shared salt, supplied `leaf_count`, and final tree digest through the root
   domain.
8. Compare all 32 root bytes.

The guest derives depth and private byte use from the witness data. It rejects
duplicate indices and unused entries across the full authorization witness.
Single-opening verification does not decide authorization. The exact rule that binds
entry use to an evaluator path belongs to the M3 witness-plan decision.

### Public and private values

Public:

- 32-byte `relationship_snapshot_root`
- tree, leaf, node, empty, and root algorithms and domains
- canonical relationship format and validation rules

Private:

- relationship records and object identifiers
- stored caveat context
- snapshot salt
- leaf count
- tree levels, leaf digests, sibling hashes, and selected indices

The tree library may expose the public root and private membership-opening objects to
trusted Rust callers. No Protobuf response, capability, agent SDK, log, or audit record
receives private tree material.

## Vector Contract

Add a versioned synthetic vector under `tests/vectors/`. It contains:

- human-readable relationship records with explicit private-test labels
- one fixed 32-byte test salt
- canonical relationship bytes
- leaf preimages and digests
- each tree level, count-bound root preimage, and expected root
- empty, one-leaf, even-width, and odd-width trees
- one valid opening for every leaf position
- Rust result and test-only TypeScript parser result

Negative cases mutate each record field, option tag, value tag, length, salt, leaf,
node order, leaf count, leaf index, sibling, depth, root, relationship order, and
duplicate identity. They include the phantom index-3 attack against a three-leaf tree
and a mixed-salt witness attempt. Every mutation must fail or produce the recorded
distinct digest.

Rust remains the only hashing authority. TypeScript parses the vector and validates
its internal framing. It does not calculate relationship leaves, nodes, or roots.

## Dependency Changes

Add only local normal dependencies from `raziel-snapshot` to `raziel-canonical` and
`raziel-policy-ir`. Reuse the pinned SHA-256 implementation already owned by
`raziel-canonical`.

Reuse workspace `serde` and `serde_json` as test dependencies for vectors. Extend the
existing test-only TypeScript vector parser with no new package. Add no external
package, CLI, runtime, or system tool.

## Trust-Boundary Effect

- The public root binds the exact validated relationship records, their structural
  order, positions, count, and one private per-snapshot salt.
- The snapshot authority still attests that it read those records from the claimed
  SpiceDB revision. The root alone does not prove source provenance or freshness.
- The SP1 guest can reject forged, aliased, duplicate, malformed, mixed-salt, and
  out-of-range membership paths without learning unrelated tuples.
- The verifier and agent receive no tuple, salt, count, context value, or opening.
- Policy authorization and dataset relevance keep separate commitments.

## Migration Cost

No accepted snapshot registry, proof, or external consumer exists yet. The first
accepted vectors establish relationship tree version 1. A later field, trait, hash,
tree-shape, salt, or count-binding change needs new domains or versions plus registry
and vector migration.

## Test Plan

- Canonical bytes for every record field, option state, scalar tag, and integer edge.
- Policy-bound validation for direct, subject-set, and caveated relationships.
- Rejection for wrong policy references, provenance, type, missing stored input,
  unsupported trait, duplicate identity, bad order, malformed identifier, and limits.
- Stable leaves for a fixed salt and changed leaves for a one-byte salt or record
  mutation.
- Stable roots for empty, one, two, three, five, and larger leaf counts.
- Count-binding tests that reject a phantom duplicate-last index.
- Positive openings for every leaf of even and odd left-balanced trees.
- Mutated index, count, salt, sibling, depth, child order, and root rejection.
- Exact `[u8; 32]` enforcement for salt, leaf, node, sibling, and root boundaries.
- Checked canonical and private-byte accounting with one-over-limit rejection.
- Deterministic vector regeneration and strict test-only TypeScript parsing.
- Workspace format, lint, typecheck, test, build, generation, and live equivalence
  gates.
- Private-data and diagnostic scans.

## Rejected Options

### Publish leaf count in the snapshot descriptor

The guest needs a count bound to the root, but the verifier and agent do not. Keeping
the count private avoids a direct relationship-volume disclosure.

### Leave count outside the root

Opening shape and index range depend on count. Leaving it outside the root lets
uncommitted caller metadata select those rules. Root binding makes count an explicit
part of the versioned snapshot commitment.

### Duplicate the unmatched last digest

Duplicate-last padding creates phantom positions unless every verifier implements
count and odd-sibling checks correctly. The left-balanced shape has no padded leaf and
derives one unique path from index and count.

### Use a constant final empty root

A public constant identifies and links every empty snapshot. Version 1 uses one
constant empty tree digest, then binds the private snapshot salt into the final root so
empty roots remain snapshot-specific.

### Sort blinded leaf hashes

The private salt would change tuple positions between snapshots. Sorting the canonical
unblinded relationship identities gives stable, reviewable positions before blinding.

### Use one random salt per leaf

Per-leaf random state expands private snapshot storage and witness construction. One
fresh 32-byte snapshot salt already prevents direct unsalted dictionary checks and is
required by the baseline architecture.

### Hash Protobuf relationship bytes

Protobuf has multiple valid wire spellings and unknown-field behavior. Explicit
canonical bytes keep commitments independent from transport code generation.
