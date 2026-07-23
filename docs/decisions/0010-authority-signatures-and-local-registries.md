# ADR-0010: Authority Signatures and Local Registries

- Status: proposed for maintainer review
- Date: 2026-07-23
- Milestone: M2
- Maintainer approval: pending
- Resolves after approval: O-004 and O-006

## Context

Raziel has fixed canonical PolicyIR and relationship-root commitments. M2 still needs
two provenance claims:

- A policy authority approved the exact `policy_hash`.
- A snapshot authority produced the exact relationship root from one stated SpiceDB
  revision.

The ZK proof does not establish either claim. It proves correct evaluation against the
committed inputs. Authority signatures establish provenance for those inputs.

The current Protobuf contracts carry policy and snapshot signatures, key identifiers,
validity fields, and mutable registry status. They do not define signature algorithms,
canonical signed bytes, key roles, key storage, rotation, or registry conflict rules.
Signing raw Protobuf or JSON would create more than one byte representation for the
same logical claim.

The baseline trust model gives `verifierd` local policy and accepted-root registries.
It must verify those records without calling `razield`. `razield` can sign snapshots,
but it cannot approve its own PolicyIR.

## Proposed Decision

### Authority roles

Version 1 defines two separate authority roles:

```text
POLICY_APPROVAL
SNAPSHOT
```

Each role uses a distinct Ed25519 key pair and key identifier. One administrator can
control both roles in the PoC, but the same private key cannot serve both roles.

`razield` receives only the snapshot signing key. It never receives the policy
approval private key. Policy approval occurs through offline maintainer-controlled
tooling. `verifierd` receives public trust records for both roles and no authority
private key.

This resolves O-006 with separate policy and snapshot authorities at the protocol
level.

Each deployment has one configured `trust_domain_id`. The value identifies the local
authorization deployment, such as `raziel-local-dev`. Policy approvals, snapshot
descriptors, and public trust records must carry the same value. This blocks artifact
replay across deployments that happen to reuse a key.

### Signature algorithm

Version 1 uses pure Ed25519 message signing with the `ed25519-dalek` 3.0.0 strict
acceptance rules. It does not use Ed25519ph, batch verification, or a custom digest
mode.

Ed25519 signs the complete domain-framed canonical message. It does not sign raw
Protobuf, JSON, hex text, or a caller-provided digest.

Signatures must contain exactly 64 bytes. Public keys must contain exactly 32 bytes.
The verifier rejects weak public keys during trust-store loading. It rejects
non-canonical, weak-key, malleable, or invalid signatures through
`VerifyingKey::verify_strict`.

### Key identifiers

Derive each key identifier from the raw Ed25519 public key:

```text
key_id = "ed25519:" || lowercase_hex(SHA-256(public_key_bytes))
```

The identifier has exactly 72 ASCII characters. The verifier recalculates it from the
trusted public key and rejects a mismatch.

The signed payload includes `signing_key_id`. A trust record binds that identifier to
one authority role. A policy key cannot validate a snapshot signature, and a snapshot
key cannot validate a policy approval.

### Canonical signature framing

Use the ADR-0008 frame:

```text
signature_message =
  u32be(len(domain)) ||
  domain ||
  u32be(len(payload)) ||
  payload
```

Every length must fit in `u32`. The message enters Ed25519 directly.

Use separate ASCII domains:

```text
raziel/policy-approval-signature/v1
raziel/snapshot-descriptor-signature/v1
```

Raw Protobuf and JSON bytes never enter either message.

### Canonical policy approval payload

Version 1 signs:

```text
PolicyApprovalClaimsV1 {
  format_version = 1
  trust_domain_id
  record_id
  policy_name
  policy_version
  approval_epoch: u64
  policy_hash: [u8; 32]
  valid_from_unix_seconds: i64
  valid_until_unix_seconds: i64
  approved_by
  signing_key_id
}
```

Encode fields in declaration order with ADR-0008 integers and length-prefixed strings
and bytes.

The canonical PolicyIR does not enter this signature a second time. The verifier
recalculates `policy_hash` from the validated PolicyIR and compares all 32 bytes before
signature verification.

`PolicyRecord.status` and `registered_at` are mutable local registry metadata. They do
not enter the authority signature.

### Canonical snapshot descriptor payload

Version 1 signs:

```text
SnapshotDescriptorClaimsV1 {
  format_version = 1
  trust_domain_id
  snapshot_id
  relationship_snapshot_root: [u8; 32]
  policy_record_id
  policy_hash: [u8; 32]
  spicedb_revision
  epoch: u64
  issued_at_unix_seconds: i64
  expires_at_unix_seconds: i64
  canonicalization_version
  signing_key_id
}
```

Encode fields in declaration order with ADR-0008 integers and length-prefixed strings
and bytes. `canonicalization_version` must equal relationship-tree version 1 from
ADR-0009.

Version 1 encodes every `u64` as exactly eight unsigned big-endian bytes. This rule
applies to policy approval epochs and snapshot epochs. It covers the full range from
zero through `2^64 - 1`.

Registry status, registration time, revocation time, and revocation reason do not
enter the snapshot authority signature. They remain mutable verifier state.

### Field validation

Both claim types require:

- `format_version = 1`
- exact match with the configured `trust_domain_id`
- exact 32-byte hashes and roots
- non-empty ASCII identifiers without control characters
- `valid_from < valid_until` for policy approvals
- `issued_at < expires_at` for snapshot descriptors
- Protobuf timestamp nanoseconds equal to zero
- timestamps inside the Protobuf timestamp range
- `epoch > 0`
- caller-supplied ceilings for string bytes and artifact lifetime

The verifier receives one trusted UTC Unix-second value for each operation. Tests pass
that value directly. Production services read it once from the system clock at the
request boundary.

Every validity interval is half-open. A value is active only when
`start <= now < end`. The verifier rejects a future issue time beyond the configured
clock-skew ceiling. A snapshot expiry cannot exceed its policy validity or
snapshot-key validity.

The PoC trusts the host UTC clock across process restarts. One process keeps a
nondecreasing observed-time floor and never revives an expired value after a backward
clock step during that process lifetime. Production clock rollback protection remains
deployment work.

### Private key storage

The PoC stores each private key as an unencrypted PKCS#8 DER file outside the
repository working tree. Docker mounts the one required file read-only into the one
process allowed to sign with it. The host file uses owner-only permissions. The
signing process runs as a non-root user and rejects a writable key mount.

The snapshot key path enters `razield` as a configuration reference. The key bytes do
not enter an environment variable, command-line argument, log, error, test vector, or
Protobuf message.

The policy approval key stays outside `razield` and `verifierd`. Offline approval
tooling receives its own read-only file mount.

The Docker build context excludes `secrets/`. Runtime services must not gain private
keys through the shared source-tree mount. Synthetic test keys remain checked-in only
as explicit test fixtures.

The loader moves file bytes into a zeroizing secret document before parsing. Loaded
signing keys zeroize owned secret material on drop. The PoC makes no memory-locking,
hardware-backed key, or process-dump protection claim.

### Public trust records

`verifierd` loads public authority trust records from a mounted local directory:

```text
AuthorityKeyRecordV1 {
  format_version = 1
  trust_domain_id
  key_id
  role
  public_key: [u8; 32]
  status: ACTIVE | REVOKED
  valid_from_unix_seconds
  valid_until_unix_seconds
}
```

The local filesystem and deployment configuration are the trust root for these
records. The records contain public keys only.

The loader rejects:

- unknown versions, roles, or status values
- a trust-domain mismatch
- duplicate key identifiers
- a derived key identifier mismatch
- invalid public key bytes
- weak public keys
- zero-length or reversed validity windows
- one public key assigned to both authority roles

Key status is checked at each cold verification. Revocation invalidates new
verification of existing artifacts.

### Key rotation

Rotation creates a new key pair and a new derived key identifier. Trust-store entries
are immutable by key identifier. A new record never replaces another key's bytes or
role.

An artifact must fit inside its signing key validity window. The old public key stays
in the trust store until every intended artifact expires. Removing or revoking it
invalidates those artifacts.

Policy approval rotation issues a new record with `approval_epoch` equal to the
trusted high watermark plus one for that `policy_name`. It can approve the same
unchanged `policy_hash` under a new key. The registry resolves only the highest active
approval epoch for a policy name.

The baseline does not support signature thresholding, cross-signing, or key rollover
certificates.

### Policy registry

The local policy registry stores:

- validated canonical PolicyIR
- recalculated `policy_hash`
- signed policy approval claims
- authority signature
- mutable status
- registration time

Registration performs these checks in order:

1. Parse and validate the PolicyIR through strict ingress.
2. Recalculate `policy_hash`.
3. Validate policy approval claim fields and limits.
4. Resolve one active `POLICY_APPROVAL` public key.
5. Verify the Ed25519 signature.
6. Require `approval_epoch` to equal the trusted policy high watermark plus one.
7. Reject duplicate record IDs or conflicting records at one approval epoch.
8. Store the record as `APPROVED`.

Stored mutable status is `APPROVED` or `REVOKED`. `EXPIRED` is derived from the trusted
current time and is not a persisted authority claim.

Revocation is one-way. A revoked record never returns to `APPROVED`. Loaders derive
the policy high watermark from every persisted record, including revoked records,
without depending on directory iteration order.

Resolution by `policy_hash` succeeds only for one approved, unexpired record with an
active authority key and valid signature. A caller can require an exact
`policy_record_id`. The registry recalculates the policy hash and rechecks the
signature on load. More than one approval can carry the same policy hash after key
rotation. Only the highest active approval epoch for its policy name can resolve.

The registry can cache immutable parsing, canonical bytes, hashes, and successful
signature checks. It checks the current policy, root, and authority-key status against
one current registry generation on every verification.

### Accepted-root registry

The accepted-root registry stores:

- the signed immutable snapshot descriptor
- mutable status
- registration time
- optional revocation metadata kept out of client responses

Registration performs these checks in order:

1. Validate descriptor fields and limits.
2. Resolve one active `SNAPSHOT` public key.
3. Verify the Ed25519 signature.
4. Resolve the exact signed `policy_record_id` locally and match its policy hash.
5. Require the snapshot lifetime to fit inside the policy and key validity windows.
6. Reject duplicate snapshot IDs or relationship roots.
7. Require the new epoch to equal the trusted snapshot high watermark plus one for the
   same policy hash.
8. Store the root as `ACCEPTED`.

Stored mutable status is `ACCEPTED` or `REVOKED`. `EXPIRED` is derived from the trusted
current time.

Root resolution succeeds only for one accepted, unexpired descriptor whose signature,
key, and referenced policy remain valid. Revocation takes effect on the next
verification attempt.

The snapshot publication authority allocates the next epoch from trusted local state.
It rejects overflow and gaps. A client cannot select the signed epoch.

Snapshot revocation is one-way. A revoked root never returns to `ACCEPTED`. Loaders
derive the snapshot high watermark from every persisted descriptor, including revoked
descriptors, without depending on directory iteration order.

Policy, root, and authority-key revocation invalidates new cold capability
verification. A gateway-cached capability remains usable until its short capability
expiry in the baseline. M5 can add cache invalidation if the acceptance scenario needs
faster revocation.

### Local persistence

The PoC persists registry inputs as binary Protobuf files. Inspectable JSON projections
are output-only and never load back into trusted state. Protobuf remains a transport
and operator format only.

Add Protobuf messages for:

- `AuthorityKeyRecord`
- `AcceptedRootRecord`
- one complete `RegistryGenerationManifest`

Extend `PolicyRecord` with the mutable revocation and registry-generation fields
needed by the same loader contract.

All binary inputs use frozen-descriptor strict parsing before typed conversion.
Unknown fields, duplicate singular fields, alternate encodings, malformed
timestamps, and trailing bytes fail closed.

Check in each frozen descriptor set with its SHA-256 checksum and one reviewed
regeneration command. A descriptor change requires a checksum update and strict
ingress mutation tests.

Registry directories live under ignored local state and mount read-only into
`verifierd`. State mutation tooling writes a new file and atomically renames it into
place. Readers never accept a partially written record.

Each complete registry publication has an immutable increasing generation. Revocation
tombstones and policy and snapshot high watermarks never decrease inside one accepted
generation chain. `verifierd` rejects a generation below the highest value observed
during its process lifetime.

The PoC treats the host administrator and mounted registry storage as trusted.
Privileged replacement of the complete registry and its generation history is outside
the adversarial threat model. Atomic publication protects against torn writes and
operator mistakes, not a hostile host administrator.

### Rust ownership

Add a host-only `crates/authority` crate for:

- Ed25519 key loading, signing, and strict verification
- authority roles and public trust records
- derived key identifiers
- value-safe authority errors

The crate does not enter the SP1 guest dependency graph.

`crates/canonical` owns policy approval payload encoding.
`crates/snapshot` owns snapshot descriptor payload encoding and intrinsic field
validation. It has no dependency on `crates/authority`.
`crates/verifier` owns policy-window, key-window, trust-domain, status, and local
registry rules.

`razield` composes canonical snapshot bytes with `crates/authority` signing.
`verifierd` composes snapshot, canonical, and authority verification through
`crates/verifier`. Neither process depends on the other.

The accepted contract needs these Protobuf corrections:

- Add `trust_domain_id` and `approval_epoch` to `PolicyRecord`.
- Add `trust_domain_id` and `policy_record_id` to
  `RelationshipSnapshotDescriptor`.
- Add authority-key, accepted-root, and registry-generation messages.
- Add one-way revocation metadata to persisted policy and root records.

`BuildSnapshotRequest.snapshot_epoch` becomes `expected_previous_epoch`. The snapshot
authority checks that value against trusted publication state and allocates the signed
epoch as `expected_previous_epoch + 1`. No external consumer exists yet.

A Rust binary under `tools/raziel-admin` owns offline policy approval, public-key
inspection, atomic registry publication, and one-way revocation. It does not generate
private keys.

Operators provision Ed25519 keys outside Raziel. One valid OpenSSL command is:

```bash
openssl genpkey -algorithm ED25519 -outform DER -out snapshot-authority.pk8
```

The same command with a separate output path creates the policy approval key. Raziel
does not need a random-number dependency for key generation.

## Dependency Approval

The concrete Ed25519 implementation needs one new direct Rust dependency:

```toml
ed25519-dalek = {
  version = "=3.0.0",
  default-features = false,
  features = ["alloc", "pkcs8", "signature", "zeroize"]
}
```

`ed25519-dalek` provides the Ed25519 key, signing, strict verification, PKCS#8, and
zeroizing key types needed by this decision. Version 3.0.0 supports the repository's
Rust version and edition.

After `crates/authority` exists, the dependency fetch and compile command is:

```bash
./scripts/dev cargo check -p raziel-authority
```

This unlocks concrete signing, verification, PKCS#8 loading, and deterministic
signature vectors.

Without this dependency, work can define canonical payloads and abstract signer
traits. It cannot meet the M2 signed-descriptor or authority-verification gate.

No TypeScript, Go, or runtime service dependency is proposed. OpenSSL is an optional
external provisioning tool, not a repository bootstrap dependency. Raziel can accept a
valid PKCS#8 DER key produced by another trusted tool.

## Trust-Boundary Effect

- Policy approval remains independent from `razield`.
- Signed artifacts remain bound to one deployment trust domain.
- The snapshot authority attests root provenance, revision, epoch, and validity.
- `verifierd` trusts mounted public-key and registry state, not `razield`.
- Agents can receive signed descriptors but never authority private keys, PolicyIR,
  relationship tuples, salts, counts, openings, or stored caveat values.
- Registry status can revoke an otherwise valid signed artifact.
- New cold verification sees the current registry generation. Gateway cache
  invalidation remains an M5 concern.
- The ZK proof remains the evidence for authorization computation. Authority signatures
  do not replace it.
- Dataset relevance remains outside policy and snapshot registries.

## Public and Private Data

Public or verifier-visible:

- policy approval claims and signature
- snapshot descriptor claims and signature
- authority key identifiers and public keys
- registry status needed for verification

Private:

- authority private keys
- relationship records and stored caveat values
- snapshot salt and private leaf count
- Merkle openings and satisfying paths
- witness-construction traces

No private relationship or witness field enters a signature payload.

## Migration Cost

No accepted policy registry, root registry, signed snapshot, proof, or external
consumer exists yet. Version 1 establishes the first signature domains, payloads, and
key identifiers.

A later algorithm, payload field, timestamp rule, role rule, or key identifier format
needs a new format version and domain. Registry migration must keep old verification
logic until every accepted artifact from that version expires or is revoked.

The Protobuf additions use new field numbers. The snapshot request field keeps field
number 6 with its corrected `expected_previous_epoch` meaning. No accepted snapshot
client or registry exists.

## Test Plan

- Deterministic Ed25519 vectors from fixed synthetic PKCS#8 test keys.
- Exact canonical bytes and signatures for policy and snapshot claims.
- `u64` vectors for `0`, `1`, `2^63`, and `2^64 - 1`.
- One-field mutations for every signed field.
- Wrong role, wrong key, unknown key, malformed key, malformed signature, and derived
  key identifier mismatch.
- One-second boundaries for policy, snapshot, and key validity.
- Future issue time beyond the configured skew.
- Duplicate policy record ID, conflicting approval epoch, snapshot ID, root, and key
  ID.
- Same public key assigned to both roles.
- Weak public keys and strict-verification edge cases.
- Revoked policy, root, or authority key.
- Expiry derived from trusted current time.
- Snapshot expiry beyond policy or key expiry.
- Wrong trust domain.
- Policy approval epoch reuse, gap, rollback, and overflow.
- Snapshot epoch reuse, gap, rollback, and overflow.
- Key rotation that renews one unchanged policy hash.
- Cache checks showing current status on every cold verification.
- Clock equality boundaries and a backward step during one process lifetime.
- Registry generation rollback during one process lifetime.
- Strict Protobuf rejection for unknown, duplicate, alternate, and trailing fields.
- Debug and error output checks for private key and private witness material.
- Dependency-graph check that `ed25519-dalek` and `crates/authority` do not enter the
  SP1 guest.

M4 tests `verifierd` with `razield` stopped after both service processes exist.

## Rejected Options

### One key for policy and snapshots

One compromised online snapshot key could approve a new policy. Separate keys preserve
the policy-authority boundary.

### Caller-selected snapshot epochs

A caller can submit `u64::MAX` and block every later snapshot. Trusted publication
state allocates the exact next epoch instead.

### Raw Protobuf or JSON signatures

Both formats allow representation differences that do not change the logical claim.
Canonical typed payloads produce one signature message.

### HMAC

HMAC gives every verifier the power to forge authority artifacts. Public-key
verification preserves verifier independence.

### ECDSA P-256

P-256 is viable but needs strict DER or fixed-width signature rules and deterministic
nonce handling. Ed25519 gives fixed key and signature widths with a smaller protocol
surface for this PoC.

### JWT or COSE envelopes

These add representation and dependency scope that the local binary protocol does not
need. The existing Protobuf envelope can carry the canonical claims and raw signature.

### Private keys in environment variables

Environment values can leak through process inspection, crash capture, and deployment
diagnostics. Read-only mounted files reduce that exposure.

### Private keys under the repository mount

The current development containers share the source-tree mount. A key inside that tree
would become visible to processes that do not need it. Exact mounts from outside the
working tree preserve role isolation.

### Signing mutable status

Signed status would require the authority private key for every revocation. Local
registry status affects the next cold verification without rewriting the immutable
provenance claim.

### Trusting `razield` for registry resolution

That path breaks verifier independence and lets the prover influence accepted policy
or root state during verification.
