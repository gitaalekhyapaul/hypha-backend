# ADR-0016: Runtime Snapshot Publication and SpiceDB Adapter

- Status: accepted
- Date: 2026-08-24
- Milestone: M4
- Related decisions: ADR-0009, ADR-0010, ADR-0013, ADR-0015
- Related baton: B-0026
- Security review triggers: snapshot publication, registry trust, snapshot signing,
  SpiceDB credentials, stored caveat context, logs containing authorization context

## Context

The repository has the main snapshot primitives:

- ADR-0009 relationship canonicalization and blinded Merkle trees
- ADR-0010 snapshot signing and trusted registry validation
- ADR-0015 immutable private snapshot storage
- a public `SnapshotService` contract with build, read, and revoke operations

The production service does not implement that contract yet. It has no runtime
SpiceDB client. The current `razield` Compose service mounts the trusted registry as
read-only. The `raziel-admin` tool owns every atomic registry write.

This split protects a core invariant. `razield` cannot mark its own PolicyIR as
approved. Mounting the whole registry as writable inside `razield` would weaken that
boundary. A compromised process could replace authority-key and policy records, not
only append a signed snapshot record.

The API contract still needs one complete build operation. A data owner should call
`BuildSnapshot` with symbolic configuration references and receive the public signed
descriptor. The operation must read one exact SpiceDB revision, publish its private
snapshot, register the signed public root, and return only after all three steps
succeed.

The repository needs decisions for the SpiceDB API binding, registry write boundary,
secret references, snapshot identity, and failure ordering.

## Decision

### Runtime ownership

`razield` implements `BuildSnapshot`, `GetSnapshot`, and `RevokeSnapshot`.

`razield` keeps a read-only registry mount. It never writes registry files. A new
`raziel-registry-writer` binary stays inside the existing `raziel-admin` package. It
owns the writable registry mount and one owner-only Unix socket.

The writer accepts only two typed operations:

- append one signed `ACCEPTED` snapshot-root record
- replace one existing snapshot-root record with a `REVOKED` record

It rejects authority-key and policy-record mutations. Existing offline
`raziel-admin` commands remain responsible for key records, policy approval, manual
publication, and manual revocation.

The writer request carries a protocol version, operation, expected registry
generation, and either one signed accepted-root record or one snapshot ID plus a
bounded revocation reason. It carries no caller-selected registry path, generation
directory, publication time, registration time, or revocation time. The writer's own
trusted clock supplies all mutable registry timestamps.

The writer loads the active generation and applies all ADR-0010 validation before an
atomic generation update. The current manifest format requires every record's
`registry_generation` to equal the new manifest generation. The writer rewrites the
complete validated key, policy, and root set into a new immutable generation
directory, adds or revokes the requested root, validates the complete candidate, and
only then replaces `current.pb`.

It checks the expected registry generation supplied by `razield`. One process handles
requests serially. This prevents two snapshot requests from allocating the same epoch
or registry generation. A request based on stale state fails and leaves the newer
generation untouched.

The writer socket has no public network port. Its directory uses mode `0700`, and the
socket uses mode `0600`. The protocol uses one versioned, length-prefixed Protobuf
request and response. Both messages contain public registry data only. Unknown
versions, operations, fields, oversized frames, extra frames, and trailing bytes
fail closed.

The deployment runs offline `raziel-admin` policy commands before starting the
writer. Operator policy or key changes stop the writer, publish one complete
generation, and restart the service. M4 does not support concurrent CLI and runtime
registry writers.

The existing private-snapshot volume receives a separate owner-mode initialization
service. Starting the normal API profile must not run the 66.57 GB proof-host check.
That memory gate remains confined to the proof profile.

### SpiceDB API binding

Add this exact direct dependency:

```toml
spicedb-rs-proto = "=1.53.0"
```

The package provides generated Rust Protobuf and Tonic bindings from the official
Authzed API `v1.53.0` tag. Raziel uses the generated
`PermissionsServiceClient` directly. It does not adopt the package author's higher
level client wrapper.

The current Compose stack pins SpiceDB server `v1.54.0`. The Authzed API repository's
latest tagged API is `v1.53.0`. A live Compose conformance test must prove that
`ReadRelationships` works against the pinned server before B-0026 passes.

Add this exact direct dependency:

```toml
getrandom = "=0.4.3"
```

`getrandom` fills each fresh 32-byte snapshot salt from the operating system random
source. Version `0.4.3` already exists in `Cargo.lock` through another dependency,
but direct use still needs an explicit workspace dependency.

These packages add no host CLI or system package. Cargo builds them through the
pinned Rust toolchain. The normal toolchain image remains responsible for cached,
offline VM builds.

### Fixed endpoint and secret references

The request fields are symbolic references. They are not URLs or file paths.

Owner configuration maps one accepted endpoint reference to:

- a fixed SpiceDB gRPC endpoint
- a fixed bearer-token file

Owner configuration maps one accepted ZedToken reference to a fixed token file.
`BuildSnapshotRequest` must match both configured reference names exactly. Request
data cannot select a path, endpoint, credential, TLS setting, or alternate secret
store.

The PoC environment contract adds:

```text
RAZIEL_SPICEDB_ENDPOINT_REF
RAZIEL_SPICEDB_ENDPOINT
RAZIEL_SPICEDB_AUTH_TOKEN_PATH
RAZIEL_ZED_TOKEN_SECRET_REF
RAZIEL_ZED_TOKEN_PATH
RAZIEL_PRIVATE_SNAPSHOT_ROOT
RAZIEL_REGISTRY_WRITER_SOCKET_PATH
RAZIEL_REGISTRY_PATH
RAZIEL_SNAPSHOT_KEY_PATH
RAZIEL_TRUST_DOMAIN_ID
```

Secret files must be owner-controlled regular files with no group or public access.
Reads are bounded. Error values, debug output, logs, and queue records must not
contain the bearer token, ZedToken value, file paths, relationships, caveat values,
or snapshot salt.

The symbolic ZedToken reference stays private. The token value does not. ADR-0010
already places the exact opaque revision in the signed descriptor as
`spicedb_revision`, and `ProofCapability.snapshot_attestation` reaches the agent. This
record keeps that accepted public contract. Hiding the revision would require a new
descriptor version and is outside B-0026.

The first PoC profile uses plaintext gRPC only on the private Compose network. A
non-local endpoint requires TLS configuration in a later decision.

### Exact relationship read

`razield` first sends one `ReflectSchema` request with `at_exact_snapshot`
consistency. The returned `read_at` token must equal the configured ZedToken. This
proves that SpiceDB can still serve the revision, including the case where every
relationship stream is empty. The reflected schema is bounded transport data. It is
not a replacement for the reviewed `.zed` source or its ADR-0006 source digest.

The approved PolicyIR must pass the tighter guest `TRUSTED_POLICY_CEILINGS` before
any SpiceDB call. Its strictly sorted object definitions provide the bounded set of
resource-type filters. Version 1 sends one `ReadRelationships` stream per definition,
each with `at_exact_snapshot` consistency using the same configured ZedToken.

The first stream uses a limit of 257 records. Each later stream uses the remaining
portion of that aggregate 257-record budget. ADR-0015 caps a private snapshot at 256
relationships. Aggregate record 257 proves that the snapshot is too large and fails
the build. The service does not paginate these bounded reads. The guest ceiling
limits the number of resource-type streams to 16.

Every response must carry the requested revision token. Missing or different
revision evidence fails the build. The adapter converts each relationship into the
existing Rust canonical model, then sorts by ADR-0009 structural identity. It rejects
duplicates and malformed data before hashing.

The adapter rejects public-wildcard subjects and relationship expiration timestamps.
Both constructs are outside the approved PolicyIR subset and have no field in the
version 1 canonical relationship record.

The accepted stored caveat context types are Boolean, string, and integer. SpiceDB
encodes context through Protobuf `Struct`, whose numeric values use binary floating
point. The adapter accepts only finite whole numbers in the exact JSON integer range
from `-9007199254740991` through `9007199254740991`. It rejects lists, objects, null,
fractional values, and unsupported scalar types.

### Policy, key, and timestamp checks

Before calling SpiceDB, `razield` loads the active registry and resolves the exact
approved `policy_record_id` and `policy_hash`. It resolves the requested active
snapshot authority key. The private PKCS#8 key must derive the same key identifier.

Trusted server time supplies `issued_at`. The request supplies whole-second
`expires_at`. ADR-0010 validity limits apply to the policy, authority key, and signed
descriptor.

Each build derives the trusted snapshot epoch from the current registry. A caller's
`expected_previous_epoch` must match that high watermark. The publisher allocates the
next epoch. No long-lived in-memory watermark can outrun the accepted registry after
a failed write.

### Snapshot identity and ordering

Version 1 derives the public snapshot identifier from the blinded relationship root:

```text
snapshot-v1:<64 lowercase relationship-root hex>
```

The root already contains the fresh private salt. The identifier is deterministic
for the generated snapshot and reveals no new private field.

Publication uses this order:

1. Validate configuration, request, policy, key, time, and expected epochs.
2. Read and convert relationships at the exact ZedToken.
3. Generate one fresh 32-byte salt and build the blinded tree.
4. Sign the public snapshot descriptor.
5. Publish the immutable private snapshot file.
6. Ask `raziel-registry-writer` to append the accepted root atomically.
7. Reload the read-only registry view and confirm the exact accepted record.
8. Return the signed public descriptor.

A failure before step 5 leaves no snapshot artifact. A failure after step 5 can
leave an unreferenced private snapshot. This is fail-closed. No public accepted root
can exist without its private record. M4 records orphan cleanup as separate owner
maintenance and never deletes a snapshot referenced by an accepted record or active
job.

`GetSnapshot` reads only the trusted registry and returns the public descriptor and
derived status. `RevokeSnapshot` sends a bounded reason and trusted timestamp to the
registry writer. Revocation is one-way and becomes visible through the next registry
generation.

## Trust-Boundary Effect

`razield` gains network access to SpiceDB, access to configured SpiceDB secrets, and
access to the snapshot signing key. Those powers are already required by the
snapshot-authority design.

`razield` does not gain writable access to the trusted registry. The registry writer
has no snapshot private key, SpiceDB token, private snapshot mount, Redis access,
proof input, or agent-facing network port.

`verifierd`, the proof sandbox, and the proof worker keep read-only registry access.
The agent receives only the existing public descriptor fields.

## Public and Private Data Effect

Public output remains:

- signed snapshot descriptor
- accepted or revoked status
- registry generation through existing trusted records

Private state remains:

- SpiceDB bearer token
- symbolic ZedToken secret reference and its configured file path
- relationships and stored caveat context
- 32-byte salt
- private snapshot file and witness inputs

The exact opaque ZedToken becomes public as the descriptor's `spicedb_revision` under
ADR-0010. This ADR does not add a new public proof value or capability field.

## Migration Cost

Implementation changes:

- add two pinned Cargo dependencies and lockfile entries
- add the runtime SpiceDB adapter under `services/razield`
- add SnapshotService admission, orchestration, and handlers
- make `raziel-admin` registry publication reusable by its CLI and writer binary
- add the private registry-writer protocol and Compose service
- mount private snapshots into `razield` as writable and into the proof sandbox as
  read-only
- add fixed secret files, configuration, health checks, and VM instructions
- update architecture and technical-spec current state

No existing proof, capability, canonical hash, PolicyIR, or public Protobuf field
needs migration.

## Test Changes

B-0026 must add:

- adapter tests for exact schema reflection, per-definition reads, empty streams, 256
  and 257 aggregate records, malformed relationships, duplicate sorting, public
  wildcards, expiration timestamps, and every caveat value class
- live SpiceDB `v1.54.0` conformance for the generated `v1.53.0` API binding
- deterministic snapshot construction with an injected test salt source
- private-before-public ordering and each failure point
- registry-writer protocol, generation conflict, append, revoke, and redaction tests
- SnapshotService build, read, revoke, generic-error, and message-limit tests
- Compose checks for read-only and writable mounts, secret mounts, socket modes, and
  private-network placement
- privacy scans proving that Redis, logs, public responses, and registry files contain
  no private snapshot inputs

No SP1 proof run is part of this decision or B-0026.

## Rejected Options

### Give `razield` a writable registry mount

This weakens the filesystem boundary around policy and authority records. It conflicts
with the rule that `razield` cannot approve its own PolicyIR.

### Require manual registry publication after every API build

This leaves `BuildSnapshot` half-complete and breaks the API-driven M5 workflow. It
creates an error-prone gap between the private snapshot and accepted public root.

### Put credentials or ZedTokens in API requests

Requests, logs, traces, and retries would carry secrets. Symbolic references keep
secret selection under owner configuration.

### Read SpiceDB again during proof generation

The revision can expire or relationships can change. The stored private snapshot is
the reproducible witness source.

### Use a hand-written SpiceDB wire client

Generated API bindings provide the official message and service shape. Hand-written
gRPC paths and message encoders add avoidable compatibility risk.

### Use only the higher level `spicedb-rs-client` wrapper

Raziel needs one bounded call and strict control over endpoint, metadata, consistency,
streaming, limits, and error mapping. The generated protocol package is the smaller
dependency surface.

### Use Redis or BullMQ for registry mutation

Registry writes are short trusted-state transactions, not long proof jobs. A local
single-writer process gives immediate success or failure and keeps registry data out
of Redis.

## Approval

The maintainer approved ADR-0016 on 2026-08-24. The approval authorizes all of these
items together:

1. Add exact direct dependencies `spicedb-rs-proto = "=1.53.0"` and
   `getrandom = "=0.4.3"`.
2. Add the owner-only `raziel-registry-writer` process and narrow local protocol.
3. Implement the fixed secret-reference, exact-read, snapshot-ID, publication-order,
   and runtime SnapshotService rules in this record.
