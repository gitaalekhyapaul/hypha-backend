# Raziel Payment-Reconciliation PoC Technical Specification

Status: draft for implementation handoff

Version: 0.1.0

Date: 2026-07-16

Owners: project maintainers

Primary architecture: [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)

## Document purpose

This document is the implementation contract for the first Raziel proof of concept.
It translates the architecture into bounded requirements, interfaces, milestones, and
acceptance tests.

An implementation agent must not expand scope without recording a decision in the
current baton and updating this specification.

## Product statement

Raziel turns a trusted online SpiceDB permission result into an independently
verifiable, proof-backed capability for an AI agent.

The first scenario is payment reconciliation. A task-scoped agent receives permission
to query a protected transaction dataset. The gateway verifies proof evidence instead
of trusting `razield` to state that access is allowed.

## Research statement

The PoC tests whether a bounded SpiceDB authorization program can be evaluated inside
SP1 with:

- an approved verifier-visible PolicyIR
- a private relationship witness
- a signed relationship snapshot commitment
- task and agent-bound capability claims
- independent verification
- reusable gateway enforcement

The PoC does not establish global research novelty. It establishes a concrete system
delta from vanilla SpiceDB and provides measurements for later comparison.

## Project goal

Deliver one end-to-end payment-reconciliation workflow against a real,
provenance-recorded transaction dataset. The workflow must create a valid proof,
verify it independently, execute an allowed query, reject tampering, and reuse the
capability for bounded rapid I/O.

No generated or hand-written transaction dataset can satisfy this goal. Deterministic
fixtures remain required for tests.

## Confirmed decisions

| ID | Decision |
| --- | --- |
| D-001 | Use one polyglot monorepo |
| D-002 | Use TypeScript as the primary product language |
| D-003 | Use Next.js with TypeScript for the console and gateway |
| D-004 | Use Rust for PolicyIR, canonicalization, SP1 host and guest, prover, and verifier |
| D-005 | Use a small Go CLI around the pinned SpiceDB compiler |
| D-006 | Use Protobuf and Buf for RPC and artifact schemas |
| D-007 | Use a separate `verifierd` process that never calls `razield` |
| D-008 | Run Linux containers locally on Apple Silicon |
| D-009 | Keep exact PolicyIR visible to the verifier through a local approved-policy registry |
| D-010 | Keep PolicyIR out of the capability and agent context |
| D-011 | Keep relationships, satisfying paths, openings, and stored caveat values private |
| D-012 | Prove only `ALLOW` in the first PoC |
| D-013 | Use a manually published relationship snapshot, not SpiceDB Watch |
| D-014 | Bind reusable capabilities to an agent key, task, purpose, action, resource, and expiry |
| D-015 | Use local proving and never send private witnesses to an external prover |
| D-016 | A data-owner policy authority approves PolicyIR; `razield` cannot approve its own policy |
| D-017 | Use the container-first pinned toolchain in ADR-0003 for M0 and CI |

## Open decisions

The following decisions must be closed before their milestone begins:

| ID | Decision | Deadline |
| --- | --- | --- |
| O-001 | Select the real transaction dataset and record its usage basis | Before scenario adapter implementation |
| O-003 | Select the SP1 proof mode used for the final demo | After the first proving benchmark |
| O-004 | Select the snapshot authority signing algorithm and key storage for the PoC | Before snapshot implementation |
| O-005 | Select the capability transport representation for HTTP | Before gateway integration |
| O-006 | Confirm whether policy approval signatures use the same authority as snapshots | Before policy registry implementation |
| O-007 | Approve the protected-request signature envelope in ADR-0002 | Before gateway integration |

Recommended PoC defaults for O-004 and O-006 are Ed25519 keys stored as mounted local
development secrets, with separate policy and snapshot key IDs.

## Actors and entities

| Entity | Baseline identifier | Meaning |
| --- | --- | --- |
| Human principal | `user:maya` | Analyst who receives access through a team |
| AI agent | `agent:recon-17` | Reconciliation process acting on one task |
| Team | `team:fraud-ops` | Group that grants the human access |
| Task | `task:review-482` | Purpose and validity context for delegation |
| Dataset | `dataset:sg-payments` | Protected transaction resource |
| Action | `query` | Only protected action in the first PoC |
| Purpose | `reconciliation` | Public capability purpose |
| Policy | `policy:reconciliation-query/v1` | Approved PolicyIR record |
| Snapshot | generated UUID | One committed SpiceDB relationship revision |

## Baseline user journey

1. An administrator loads the reconciliation schema and relationships into SpiceDB.
2. SpiceDB confirms that Maya can query `dataset:sg-payments` through `team:fraud-ops`.
3. The delegation control writes a task-scoped caveated grant for `agent:recon-17`.
4. The Go compiler adapter converts the compiled SpiceDB schema into PolicyIR.
5. The data-owner policy authority reviews, signs, and registers the approved PolicyIR.
6. The administrator asks `razield` to publish a relationship snapshot at the returned
   ZedToken and approved `policy_hash`.
7. `razield` loads the approved policy record, builds the relationship commitment, and
   signs the snapshot descriptor.
8. The agent SDK requests a capability for the task, purpose, action, and dataset.
9. `razield` builds a private witness and generates an SP1 proof of `ALLOW`.
10. The agent sends a protected query and proof capability to the Next.js gateway.
11. The gateway asks local `verifierd` to verify the proof and signed request.
12. `verifierd` resolves the approved PolicyIR locally and returns verified claims.
13. The gateway enforces scope and sends the query to the transaction adapter.
14. Later requests reuse cached verified claims until expiry.
15. The gateway emits an audit receipt without private authorization data.

## Functional requirements

### Repository and developer experience

- **FR-001**: The repository shall expose one documented command for setup checks.
- **FR-002**: The repository shall expose one command to start the local stack.
- **FR-003**: The repository shall expose separate TypeScript, Rust, Go, and end-to-end test commands.
- **FR-004**: Toolchain and dependency versions shall be pinned.
- **FR-005**: Linux containers shall run on Apple Silicon through multi-platform images or local builds.
- **FR-006**: No package or system dependency shall be installed without maintainer approval.
- **FR-007**: Generated source files shall be reproducible from checked-in schemas and configuration.

### SpiceDB compiler adapter

- **FR-010**: The adapter shall accept SpiceDB schema text and an explicit compiler version.
- **FR-011**: The adapter shall call the pinned SpiceDB schema compiler.
- **FR-012**: The adapter shall translate compiled namespace and caveat definitions into Raziel PolicyIR.
- **FR-013**: The adapter shall reject every unsupported operation with a typed location-aware error.
- **FR-014**: The adapter shall emit PolicyIR Protobuf and a human-readable diagnostic form.
- **FR-015**: The adapter shall never silently rewrite unsupported semantics.
- **FR-016**: The adapter shall publish compiler-version metadata in its output.

### Policy registry

- **FR-020**: The registry shall store canonical PolicyIR, `policy_hash`, version, status, validity window, and authority signature.
- **FR-021**: The registry shall support `APPROVED`, `REVOKED`, and `EXPIRED` states.
- **FR-022**: The verifier shall accept only an `APPROVED` record within its validity window.
- **FR-023**: The verifier shall recalculate `policy_hash` from canonical PolicyIR.
- **FR-024**: The registry shall not be exposed to the AI-agent network identity.
- **FR-025**: Policy registration and revocation shall emit audit events.

### Relationship snapshot

- **FR-030**: `razield` shall publish a snapshot for an explicit SpiceDB revision.
- **FR-031**: Every relationship leaf shall include the full tuple and stored caveat context.
- **FR-032**: Leaves shall use domain separation and a private per-snapshot salt.
- **FR-033**: The snapshot shall contain a public relationship root and signed descriptor.
- **FR-034**: The descriptor shall include `policy_hash`, revision, epoch, issue time, expiry, and canonicalization version.
- **FR-035**: The verifier shall maintain an accepted-root registry.
- **FR-036**: A revoked or expired root shall invalidate new verification attempts.
- **FR-037**: Snapshot creation shall never log tuple contents at normal log levels.

### Witness construction

- **FR-040**: The witness builder shall search only within the selected snapshot.
- **FR-041**: The witness builder shall return one positive path for each required expression branch.
- **FR-042**: Every witness relationship shall include a Merkle membership opening.
- **FR-043**: The witness builder shall enforce depth, node, tuple, and context-size limits before proving.
- **FR-044**: The witness builder shall fail closed if no bounded positive path exists.
- **FR-045**: Private witness data shall remain in memory or protected local artifact storage.
- **FR-046**: Private witness data shall not enter capability, receipt, browser response, or standard logs.

### SP1 guest

- **FR-050**: The guest shall validate all version and limit fields.
- **FR-051**: The guest shall recalculate the PolicyIR commitment.
- **FR-052**: The guest shall verify every Merkle membership opening.
- **FR-053**: The guest shall evaluate the bounded PolicyIR and caveat IR.
- **FR-054**: The guest shall bind the result to the capability request commitment.
- **FR-055**: The guest shall commit only an `ALLOW` result.
- **FR-056**: The guest shall reject unused or excess witness entries.
- **FR-057**: The guest shall reject malformed, duplicate, ambiguous, and out-of-order canonical fields.
- **FR-058**: The guest shall publish only the fields listed in `AuthorizationPublicValues`.

### Capability issuance

- **FR-060**: The agent shall authenticate `CreateCapability` using an Ed25519 request signature in the baseline.
- **FR-061**: The service shall bind capability claims to the agent-key thumbprint.
- **FR-062**: The capability shall cover one task, purpose, action, resource type, and resource scope.
- **FR-063**: The baseline capability lifetime shall default to 15 minutes.
- **FR-064**: The service shall cap requested expiry at the snapshot, policy, and delegation expiry.
- **FR-065**: The service shall calculate a deterministic issuance nullifier for conflicting-issuance control.
- **FR-066**: Proof generation shall use an asynchronous job API.
- **FR-067**: Failed authorization shall return a generic client error and a typed internal reason.
- **FR-068**: The capability shall include `policy_hash`, never PolicyIR.

### Independent verification

- **FR-070**: `verifierd` shall run as a process separate from `razield`.
- **FR-071**: `verifierd` shall have no `razield` endpoint configuration.
- **FR-072**: `verifierd` shall resolve `policy_hash` from its local policy registry.
- **FR-073**: `verifierd` shall validate policy and snapshot authority signatures.
- **FR-074**: `verifierd` shall verify the SP1 proof and public-value equality.
- **FR-075**: `verifierd` shall validate agent-key binding, the protected-request signature envelope, expiry, and request anti-replay inputs.
- **FR-076**: `verifierd` shall return typed verified claims or typed failure codes.
- **FR-077**: `verifierd` shall complete valid verification with `razield` stopped.

### Gateway and rapid I/O

- **FR-080**: The Next.js gateway shall be the policy enforcement point.
- **FR-081**: The gateway shall never accept a prover-only Boolean decision.
- **FR-082**: The first request shall send the full capability to `verifierd`.
- **FR-083**: The gateway shall compare the API request with verified public scope.
- **FR-084**: The gateway shall cache verified claims by capability ID and proof digest until expiry.
- **FR-085**: Every request shall carry the agent public key, a signature over the versioned method-target-body envelope, a timestamp, and a unique nonce.
- **FR-086**: The gateway shall reject replayed nonces within the capability lifetime.
- **FR-087**: The gateway shall reject requests outside the action, resource, purpose, or task scope.
- **FR-088**: A cache hit shall verify the agent-key thumbprint, request signature, timestamp, nonce, and scope without calling `razield` or rerunning the SP1 verifier.
- **FR-089**: Gateway denials shall not reveal the failing relationship or policy branch.

### Real transaction data

- **FR-090**: The acceptance dataset shall be externally sourced or user-provided.
- **FR-091**: The source shall have a documented usage basis and provenance record.
- **FR-092**: The repository shall not commit restricted raw data.
- **FR-093**: A source manifest shall record URL or custody source, retrieval date, checksum, format, row count, and schema fingerprint.
- **FR-094**: The adapter shall map source columns into the canonical reconciliation model.
- **FR-095**: The demo shall execute queries against imported source rows, not hard-coded responses.
- **FR-096**: The adapter shall reject schema drift with a typed error.
- **FR-097**: Logs and screenshots shall redact sensitive or identifying source fields.

### Audit receipts

- **FR-100**: The gateway shall emit one receipt for capability verification and each protected request.
- **FR-101**: Receipts shall contain IDs, public claim digests, action, resource scope, timestamp, outcome, and reason code.
- **FR-102**: Receipts shall omit private policy and relationship data.
- **FR-103**: The baseline shall write append-only JSONL receipts.
- **FR-104**: Receipt fields shall have a versioned Protobuf schema even if stored as JSONL.

## Non-functional requirements

- **NFR-001 Security**: All authorization paths fail closed.
- **NFR-002 Determinism**: Canonical commitments match published test vectors.
- **NFR-003 Privacy**: Private witness material never enters public artifacts or logs.
- **NFR-004 Portability**: The stack runs in Linux containers on Apple Silicon.
- **NFR-005 Auditability**: Every accepted policy, snapshot, proof, and request has a stable ID.
- **NFR-006 Reproducibility**: A clean checkout can regenerate contracts and execute tests with pinned tooling.
- **NFR-007 Observability**: Logs use request, task, capability, proof-job, policy, and snapshot IDs.
- **NFR-008 Performance**: Benchmarks separate execution, proving, verification, cold gateway, and cache-hit latency.
- **NFR-009 Maintainability**: Scenario adapters cannot import SP1 guest internals.
- **NFR-010 Compatibility**: Every contract change includes Rust and TypeScript generation plus fixture updates.

## Planned repository structure

```text
apps/
  demo-console/
    app/
    components/
    lib/
  demo-gateway/
    app/api/
    src/auth/
    src/payments/
packages/
  sdk-ts/
  proto-ts/
services/
  razield/
  verifierd/
crates/
  canonical/
  policy-ir/
  snapshot/
  witness/
  prover/
  verifier/
programs/
  authorization-guest/
tools/
  spicedb-ir-compiler/
proto/
  raziel/v1/
    common.proto
    policy.proto
    snapshot.proto
    capability.proto
    prover_service.proto
    verifier_service.proto
    audit.proto
examples/
  reconciliation/
    README.md
    scenario.yaml
    source-manifest.example.yaml
    spicedb/
      schema.zed
      relationships.example.yaml
    mappings/
      actions.yaml
      resources.yaml
      fields.yaml
    fixtures/
      README.md
      allowed/
      denied/
    tests/
      acceptance.yaml
tests/
  fixtures/
  vectors/
  e2e/
```

## Protobuf service surface

The schema below is structural pseudocode. The implementation milestone shall convert
it into linted files under `proto/raziel/v1`.

```protobuf
service SnapshotService {
  rpc BuildSnapshot(BuildSnapshotRequest) returns (BuildSnapshotResponse);
  rpc GetSnapshot(GetSnapshotRequest) returns (RelationshipSnapshotDescriptor);
  rpc RevokeSnapshot(RevokeSnapshotRequest) returns (RevokeSnapshotResponse);
}

service CapabilityService {
  rpc CreateCapability(CreateCapabilityRequest) returns (ProofJob);
  rpc GetCapability(GetCapabilityRequest) returns (ProofJob);
  rpc CancelCapability(CancelCapabilityRequest) returns (ProofJob);
}

service VerificationService {
  rpc VerifyCapability(VerifyCapabilityRequest) returns (VerifyCapabilityResponse);
}
```

### `BuildSnapshotRequest`

Required fields:

- `spicedb_endpoint_ref`
- `zed_token_secret_ref`
- `policy_record_id`
- `policy_hash`
- `expires_at`
- `snapshot_epoch`
- `authority_key_id`

The request must reference the ZedToken by runtime secret name. It must never carry a
SpiceDB token value or persist one in a job record.

### `CreateCapabilityRequest`

Required fields:

- `format_version`
- `agent_id`
- `agent_public_key`
- `task_id`
- `purpose`
- `action`
- `resource_type`
- `resource_scope`
- `requested_expires_at`
- `issuance_nonce`
- `snapshot_id`
- `request_signature`

### `ProofJob`

```text
ProofJob {
  job_id
  state: QUEUED | EXECUTING | PROVING | SUCCEEDED | FAILED | CANCELED
  submitted_at
  updated_at
  capability?
  public_error_code?
  internal_error_ref?
  metrics?
}
```

Internal errors must never embed private witness data.

### `VerifyCapabilityRequest`

```text
VerifyCapabilityRequest {
  proof_capability
  agent_public_key
  request_method
  canonical_request_target
  request_body_hash
  request_nonce
  request_timestamp
  agent_request_signature
}
```

### `VerifyCapabilityResponse`

```text
VerifyCapabilityResponse {
  outcome: VERIFIED | REJECTED
  verified_claims?
  failure_code?
  policy_record_id?
  snapshot_id?
  proof_digest?
}
```

### `ProofCapability`

```text
ProofCapability {
  format_version
  capability_id
  agent_key_thumbprint
  task_id
  purpose
  action
  resource_type
  resource_scope
  policy_hash
  relationship_snapshot_root
  snapshot_id
  capability_request_commitment
  issuance_nullifier
  issued_at
  expires_at
  proof_system
  verification_key_id
  zk_proof
  snapshot_attestation
}
```

The capability never includes PolicyIR, relationship tuples, branch choices, Merkle
openings, or caveat witness values.

### `AuthorizationPublicValues`

```text
AuthorizationPublicValues {
  format_version
  policy_hash
  relationship_snapshot_root
  capability_request_commitment
  decision = ALLOW
  capability_scope_hash
  issued_at
  expires_at
  issuance_nullifier
}
```

The verifier must compare every public value with the capability claims or a canonical
commitment recomputed from those claims. Extra public values fail verification.

## PolicyIR schema

### Program

```text
PolicyProgram {
  ir_version
  compiler_name
  compiler_version
  source_schema_digest
  definitions[]
  permissions[]
  caveats[]
  limits
}
```

### Permission expression nodes

```text
PermissionExpr =
  RelationRef(name)
  | SubjectRelationRef(relation, subject_relation)
  | AnyOf(children[])
  | AllOf(children[])
  | Arrow(source_relation, target_permission)
  | CaveatedRelation(relation, caveat_name)
```

Constraints:

- `AnyOf` has at least two children.
- `AllOf` has at least two children.
- Child ordering follows canonical source-independent ordering.
- The baseline allows at most one arrow in a satisfying path.
- Every reference resolves at compilation time.
- No node can refer to itself directly or indirectly.

### Caveat expression nodes

```text
CaveatExpr =
  ConstBool(value)
  | ConstInt(value)
  | ConstString(value)
  | InputRef(source, name, type)
  | Eq(left, right)
  | NotEq(left, right)
  | LessThan(left, right)
  | LessThanOrEqual(left, right)
  | GreaterThan(left, right)
  | GreaterThanOrEqual(left, right)
  | And(children[])
  | Or(children[])
  | Not(child)
```

`InputRef.source` is one of:

- `REQUEST_CONTEXT`
- `RELATIONSHIP_CONTEXT`
- `CAPABILITY_CLAIM`

No implicit type coercion is permitted.

Boolean caveat composites evaluate every child and combine missing-input results with
CEL-compatible, order-independent absorption:

| Expression | Result |
| --- | --- |
| `false && missing` | `false` |
| `true && missing` | missing-input error |
| `true || missing` | `true` |
| `false || missing` | missing-input error |

Type, witness, resolver, and budget errors always propagate. ADR-0007 records the
security rationale and supersedes ADR-0004's conflicting clause.

### Limits

Initial safe ceilings shall be low and benchmarked:

```text
PolicyLimits {
  max_expression_nodes
  max_caveat_nodes
  max_path_depth
  max_relationship_witnesses
  max_merkle_depth
  max_string_bytes
  max_context_entries
  max_total_private_input_bytes
}
```

The technical implementation plan must select numeric values after cycle-count
measurement. Tests must cover every ceiling and one-over-ceiling rejection.

## Canonical encoding rules

The canonical encoding is a security protocol, not an implementation detail.

Each commitment input shall use:

1. A fixed ASCII domain separator.
2. A fixed format version.
3. Big-endian unsigned lengths.
4. Two's-complement or explicit signed-magnitude integers, selected once and documented.
5. UTF-8 strings after a documented normalization policy.
6. Explicit optional-field presence bytes.
7. Sorted maps converted to repeated ordered entries.
8. Deterministically sorted set-like collections.
9. Original order for sequence-like collections.
10. Rejection of unknown fields at the commitment boundary.

The implementation shall publish test vectors containing:

- human-readable input
- canonical bytes in hex
- domain separator
- expected digest
- Rust result
- TypeScript parser result
- negative mutation cases

TypeScript does not implement proof-critical hashing in the baseline. It can decode and
display vectors. Rust remains the commitment authority.

## Relationship tree requirements

### Leaf ordering

Leaves shall be sorted by the canonical unblinded tuple key before blinded hashing.
The sort key excludes the private snapshot salt.

### Leaf privacy

A private per-snapshot salt shall enter each leaf hash. The salt remains in the
snapshot secret material and private witness.

### Membership witness

Each witness entry contains:

```text
RelationshipWitness {
  canonical_relationship
  snapshot_secret_salt
  leaf_index
  sibling_hashes[]
  direction_bits[]
}
```

The guest shall reject duplicate leaf indices and unused entries.

### Update model

The baseline rebuilds the whole tree for every manual snapshot. Incremental trees and
Watch synchronization are deferred.

## Policy registry requirements

```text
PolicyRecord {
  record_id
  policy_name
  policy_version
  canonical_policy_ir
  policy_hash
  status
  valid_from
  valid_until
  approved_by
  signing_key_id
  authority_signature
  registered_at
}
```

The registry can use a local mounted directory in the PoC. Each record should have a
binary Protobuf form and an inspectable JSON projection. Signature verification uses
the binary canonical record payload, not JSON bytes.

## Capability semantics

### Scope

The baseline scope is:

```text
agent = agent:recon-17
task = task:review-482
purpose = reconciliation
action = query
resource_type = dataset
resource_scope = sg-payments
expires_at <= minimum(policy, snapshot, delegation, requested expiry)
```

### Key binding

The capability carries `agent_key_thumbprint`. Each protected request carries an
Ed25519 signature from that key. A copied capability without the agent private key is
not usable.

### Reuse

The proof covers capability scope. It does not cover every transaction query body.
The gateway validates each request against the verified scope and scenario query
allowlist.

### Revocation

The baseline revokes through:

- short expiry
- policy-record status
- accepted-root status
- capability denylist for emergency local testing

Online per-request revocation checks against `razield` are prohibited.

## Next.js application requirements

### Demo console

The console shall expose operational screens, not a marketing landing page:

- scenario status and source provenance
- SpiceDB schema and relationship-load status
- approved policy record
- snapshot publication and root status
- agent and task creation
- capability proof-job progress
- public capability inspection
- protected reconciliation query
- independent verification result
- audit receipts and benchmark measurements

No screen may render private relationship paths, caveat witness values, salts, or
Merkle openings.

### Gateway

The gateway shall use Next.js route handlers for the PoC. Keep authorization middleware
separate from scenario routes:

```text
apps/demo-gateway/src/auth/
  capability-parser.ts
  verifier-client.ts
  scope-enforcer.ts
  nonce-store.ts
  verified-cache.ts
  receipt-writer.ts
```

Scenario handlers depend on verified claims. They do not parse or verify ZK proofs.

## Reconciliation domain model

The real source adapter shall map source data into a minimal canonical form:

```text
TransactionRecord {
  transaction_id
  occurred_at
  amount_minor
  currency
  merchant_reference?
  account_reference?
  transaction_type?
  source_record_digest
}
```

The acceptance query set should remain narrow:

- list transactions in a bounded time range
- filter by exact transaction or merchant reference where available
- identify candidate duplicate records using a documented deterministic rule
- return aggregate count and bounded result rows

The gateway must enforce maximum date range, page size, and result fields. These
controls are application policy in the baseline, not part of the ZK proof.

## Real-data source manifest

`examples/reconciliation/source-manifest.example.yaml` shall contain placeholders for
this canonical schema. A completed local manifest shall use the same schema at
`data/manifests/reconciliation/source-manifest.yaml`, which Git ignores.

```yaml
format_version: 1
source_id: ""
source_name: ""
source_url: null
custody_record: null
retrieved_at: ""
usage_basis: ""
license_reference: null
sha256: ""
raw_filename: ""
format: ""
compressed_size_bytes: 0
uncompressed_size_bytes: 0
row_count: 0
schema_fingerprint: ""
contains_personal_data: false
redactions_applied: []
raw_data_git_policy: "not-committed"
adapter_version: ""
notes: null
```

Exactly one of `source_url` and `custody_record` must be non-null. The selected source
must pass maintainer review before download or ingestion code is added.

## Logging and audit model

### Allowed identifiers

- request ID
- task ID
- capability ID
- proof-job ID
- proof digest
- policy record ID and hash
- snapshot ID and root
- public action and resource scope
- timing and outcome codes

### Forbidden log content

- raw relationship tuples
- satisfying path
- caveat witness values
- snapshot salt
- Merkle siblings
- agent private key
- raw transaction rows at normal levels
- source credentials
- full capability proof bytes

## Error taxonomy

External errors remain coarse:

```text
INVALID_REQUEST
NOT_AUTHORIZED
CAPABILITY_EXPIRED
CAPABILITY_INVALID
REQUEST_REPLAYED
SERVICE_UNAVAILABLE
```

Internal typed errors can include:

```text
UNSUPPORTED_POLICY_NODE
POLICY_NOT_APPROVED
POLICY_HASH_MISMATCH
SNAPSHOT_NOT_ACCEPTED
SNAPSHOT_EXPIRED
MERKLE_OPENING_INVALID
CAVEAT_FALSE
NO_SATISFYING_PATH
PROOF_GENERATION_FAILED
PROOF_INVALID
PUBLIC_VALUES_MISMATCH
AGENT_KEY_MISMATCH
SCOPE_MISMATCH
NONCE_REUSED
SOURCE_SCHEMA_DRIFT
```

Internal errors must not include secret values.

## Test strategy

### Unit tests

- PolicyIR validation and evaluation
- caveat type checking and evaluation
- canonical encoding vectors
- relationship leaf encoding
- Merkle membership verification
- snapshot descriptor signing and verification
- capability request commitments
- capability scope comparison
- nonce replay store

### Compiler tests

- one fixture for every supported SpiceDB construct
- one rejection fixture for every unsupported construct
- source-location diagnostics
- pinned SpiceDB compiler compatibility
- deterministic PolicyIR output

### Equivalence tests

For the supported subset, execute the same authorization cases through:

1. SpiceDB `CheckPermission`.
2. The Rust host PolicyIR evaluator.
3. SP1 guest execution without proof generation.
4. SP1 proof generation and `verifierd` verification for selected cases.

All four must agree for positive cases. Negative cases must fail closed.

### Adversarial tests

The suite shall mutate one field at a time:

- policy hash
- PolicyIR record
- relationship root
- snapshot signature
- snapshot expiry
- request commitment
- agent key
- action
- resource scope
- task
- purpose
- capability expiry
- proof bytes
- public values
- request nonce
- request body after signing

Every mutation must fail.

### Independence test

1. Build a snapshot and capability.
2. Stop `razield`.
3. Start or retain `verifierd` and the gateway.
4. Verify and use the capability.
5. Confirm no network request targets `razield`.

### Real-data acceptance test

1. Verify source manifest and checksum.
2. Import the documented dataset.
3. Load SpiceDB schema and relationships.
4. Publish policy and relationship snapshot.
5. Issue a capability for the reconciliation agent.
6. Verify it through `verifierd`.
7. Run a bounded reconciliation query against source rows.
8. Repeat through the capability cache.
9. Tamper with one request and confirm rejection.
10. Export public audit receipts and benchmark measurements.

## Performance measurements

Record at least:

- PolicyIR compile time
- snapshot build time
- witness search time
- SP1 execute time and cycle count
- proof generation time
- proof size
- verification time
- first gateway-request latency
- cached gateway-request latency
- container memory at idle and proof peak

Performance claims must state hardware, container runtime, tool versions, policy size,
relationship count, path depth, and proof mode.

## Milestones

### M0: Repository and contract scaffold

Deliver:

- pnpm and Cargo workspaces
- Next.js app shells
- Rust service and crate shells
- Go adapter shell
- Protobuf and Buf setup
- Docker Compose skeleton
- pinned version files
- CI format, lint, build, and test jobs

Exit gate:

- clean checkout runs setup checks
- all empty-shell builds pass
- generated code is reproducible

### M1: PolicyIR and SpiceDB compiler adapter

Deliver:

- baseline PolicyIR Protobuf
- Go translation adapter
- Rust PolicyIR validator and evaluator
- supported and rejected schema fixtures
- SpiceDB equivalence harness

Exit gate:

- every supported fixture matches SpiceDB
- every unsupported fixture fails closed

### M2: Canonical commitments and snapshots

Deliver:

- canonical encoding crate
- test vectors
- blinded relationship tree
- signed snapshot descriptor
- local policy and root registries

Exit gate:

- mutation tests fail
- snapshot can be reproduced from fixed input and salt

### M3: SP1 execution and proof

Deliver:

- authorization guest
- host input builder
- public values
- execute-mode equivalence
- local proof generation
- proof benchmark

Exit gate:

- valid proof verifies
- all public-value mutations fail

### M4: Capability service and independent verifier

Deliver:

- asynchronous capability job API
- proof capability artifact
- local policy registry lookup
- `verifierd`
- agent key binding and request signatures

Exit gate:

- verifier works with prover stopped
- agent cannot use an altered or stolen unbound capability

### M5: Next.js gateway and console

Deliver:

- operational demo screens
- gateway enforcement middleware
- verifier client
- capability cache
- nonce replay control
- JSONL receipts

Exit gate:

- first and cached requests execute
- out-of-scope and replayed requests fail

### M6: Real-data reconciliation

Deliver:

- approved source manifest
- reproducible ingestion adapter
- canonical transaction mapping
- bounded reconciliation queries
- complete demo journey

Exit gate:

- no mocked acceptance data
- end-to-end proof-backed query succeeds
- evidence bundle records provenance and measurements

### M7: Scenario-extension proof

Deliver:

- second scenario design using the scenario-authoring contract
- effort and changed-file report
- confirmation that no SP1 guest fork is needed

Exit gate:

- second scenario can be planned through configuration, policy, adapter, and UI changes

## Definition of done

The PoC is done only after all of these statements are true:

- A real transaction source is documented and ingested.
- SpiceDB and PolicyIR agree for the supported policy subset.
- A relationship snapshot and approved policy record are signed.
- `razield` generates a real SP1 proof of `ALLOW`.
- `verifierd` validates the proof without contacting `razield`.
- The agent uses a key-bound capability at the Next.js gateway.
- The gateway enforces scope and blocks replay.
- A cached request executes without a second proof verification.
- Tampering tests fail as specified.
- Private graph and witness data do not appear in public artifacts or logs.
- The baton and architecture documents match the running system.
- Benchmarks and an acceptance evidence bundle are checked in without private data.

## Deferred experiments

### Fully private PolicyIR

Later work can replace verifier-visible PolicyIR with:

```text
policy_commitment = H(
  "raziel/private-policy/v1",
  private_policy_salt,
  canonical_policy_ir
)
```

The experiment shall compare proof cost, auditability, inference exposure, registry
trust, and implementation complexity against the baseline.

### Live snapshot synchronization

Later work can consume SpiceDB Watch updates, build versioned incremental trees, and
define a root-acceptance window.

### Stateful leakage controls

Later work can add a query ledger, cumulative privacy budget, coarse relevance proof,
and discovery-rate controls.

### Verifier packaging

Later work can package the Rust verifier as N-API, WASM, or an embedded library after
compatibility and proof-size measurements.
