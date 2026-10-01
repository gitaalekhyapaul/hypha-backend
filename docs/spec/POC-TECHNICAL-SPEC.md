# Raziel Payment-Reconciliation PoC Technical Specification

Status: active implementation contract

Version: 0.2.0

Date: 2026-09-25

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

Deliver an end-to-end two-agent payment-reconciliation workflow through a public SDK
and three dashboards. The agents use labeled synthetic processor and bank ledgers,
request operator approval, receive separate real local Plonk capabilities, query
protected data, exchange bounded evidence, and reuse capabilities for rapid I/O.

[ADR-0019](../decisions/0019-public-sdk-and-two-agent-reconciliation.md) records the
maintainer's revised scope. The [M6-M7 plan](M6-M7-AGENT-DEMO-PLAN.md) defines the
feature order and user flows. Real-data acceptance is deferred and unclaimed.
Fixture-only tests cannot replace the live two-agent proof demonstration.

## Confirmed decisions

| ID | Decision |
| --- | --- |
| D-001 | Keep the polyglot backend repo and use separate public SDK and demo-agent repos under ADR-0019 |
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
| D-013 | Use explicit owner-controlled snapshots, triggered by approved access requests in M6-M7, not SpiceDB Watch |
| D-014 | Bind reusable capabilities to an agent key, task, purpose, action, resource, and expiry |
| D-015 | Use local proving and never send private witnesses to an external prover |
| D-016 | A data-owner policy authority approves PolicyIR; `razield` cannot approve its own policy |
| D-017 | Use the container-first pinned toolchain in ADR-0003 for M0 and CI |
| D-018 | Use strict Ed25519 with separate policy and snapshot keys, canonical signed claims, deployment trust-domain binding, and verifier-local registries as defined by ADR-0010 |
| D-019 | Use local SP1 Plonk for final demo proof artifacts under the validated ADR-0012 profile |
| D-020 | Use bounded typed policy drafts, a separate policy-authority process, validated registry publication, write-only relationship administration, and private pre-publication revision handoff as defined by ADR-0018 |
| D-021 | Deliver a public signer-based SDK, per-agent tokens, request approval, independent policy publication, serial proof dispatch, and a synthetic three-dashboard demo under ADR-0019 |

## Open decisions

The following decisions must be closed before their milestone begins:

| ID | Decision | Deadline |
| --- | --- | --- |
| O-001 | Deferred: select a real transaction dataset and record its usage basis | Before any later real-data acquisition, outside revised M6-M7 |
| O-005 | M5 uses `capability_proto_hex`; review the new public API ProtoJSON transport in ADR-0020 | Before external SDK integration |

O-007 closed on 2026-08-24 when the maintainer accepted ADR-0002.

[ADR-0020](../decisions/0020-agent-api-and-request-provisioning.md) approves the new
agent-authentication envelope and independent policy-version provisioning contract.
Its [Agent API v1 specification](AGENT-API-V1.md) lists endpoints, fields, state
transitions, resource bounds, retry behavior, and the SDK boundary. These decisions
were approved on 2026-09-25. B-0064 authentication is implemented and locally tested.
B-0065 request admission, own-request inspection, operator inbox, rejection, and
cancellation pass locally. Composed policy publication passes a two-version local
SpiceDB gate and the connected visual builder in an isolated opt-in M6 runtime.
The owner catalogue and connected installed-approval view pass locally. Immutable
dataset-binding storage and cold/cache gateway enforcement are implemented. Operator
approval writes that mapping and pins exact private input. The browser and packaged
SDK observed APPROVED_WAITING without a capability. Private snapshot preparation
passed against real SpiceDB with an exact pinned revision and signed descriptor.
Retry reused that descriptor and private tree. B-0066 now implements the serial
preparation/signature-reservation worker, durable final signature acceptance,
authenticated resume, proof-service handoff, terminal result retention, and the
protected-query facade. The SDK signs and submits retained attempts, polls with
bounded waits, and reuses capabilities with fresh protected-query signatures.
The packaged consumer passes cold/cache and revoked-token checks with a controlled
proof verifier. The separate live M6 Compose stack and VM runbook are implemented.
ADR-0021 adds dedicated persistent SpiceDB storage. Connected VM proof validation
and public npm publication remain pending. Existing proof contracts are unchanged.

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

The sequence below describes the implemented M0-M5 service path. M6-M7 adds an
agent access intent and operator review before snapshot preparation. Policy
publication runs independently. The agent signs the final snapshot-bound request
near proof dispatch. See the [new user flows](M6-M7-AGENT-DEMO-PLAN.md#user-flows).

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
9. `razield` validates the request, queues a private proof job, and returns its ID.
10. A local proof worker builds the private witness and generates an SP1 proof of
    `ALLOW`.
11. The SDK polls the job API and receives the completed proof capability.
12. The agent sends a protected query and proof capability to the Next.js gateway.
13. The gateway asks local `verifierd` to verify the proof and signed request.
14. `verifierd` resolves the approved PolicyIR locally and returns verified claims.
15. The gateway enforces scope and sends the query to the transaction adapter.
16. Later requests reuse cached verified claims until expiry.
17. The gateway emits an audit receipt without private authorization data.

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
- **FR-034**: The descriptor shall include the trust domain, exact policy record and hash, revision, epoch, issue time, expiry, canonicalization version, signing key identifier, and authority signature.
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
- **FR-063**: The local Plonk profile shall use a maximum 75-minute proof-bound
  capability window and preserve at least 15 minutes of usable time after a successful
  proof.
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

### Transaction data

- **FR-090**: M6-M7 shall use labeled synthetic seed or operator-created records. Externally sourced real-data acceptance is deferred.
- **FR-091**: Synthetic sources shall record generator or operator origin. A later real source shall have a documented usage basis and provenance record.
- **FR-092**: The repository shall not commit restricted raw data.
- **FR-093**: Demo revisions shall record source, checksum, format, row count, and schema fingerprint. A later real source manifest shall add URL or custody source and retrieval date.
- **FR-094**: The adapter shall map source columns into the canonical reconciliation model.
- **FR-095**: The demo shall execute queries against stored seed, portal-created, or imported rows, not hard-coded responses.
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

This tree describes the backend repository. ADR-0019 moves SDK source ownership into
the sibling `raziel-sdk-ts` repository and adds `raziel-demo-agents` for the two agent
applications. `packages/sdk-ts` now re-exports a pinned local release candidate from
the sibling SDK. Public npm publication remains pending. There is one SDK implementation.

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
  authorization-core/
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
- `expected_previous_epoch`
- `authority_key_id`

The request must reference the ZedToken by runtime secret name. It must never carry a
SpiceDB token value or persist one in a job record.

The snapshot authority compares `expected_previous_epoch` with trusted publication
state. It assigns the next signed epoch. A caller cannot select the signed epoch.

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
- `request_context`
- `request_signature`

`agent_id` is service metadata. Authorization derives the PolicyIR subject from
`agent_key_thumbprint` as specified by ADR-0011. Request context is private, typed,
strictly ordered, and covered by `capability_request_commitment`.

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
The SP1 journal is the exact ADR-0011 canonical byte sequence. It is not a serialized
`AuthorizationPublicValues` Protobuf message.

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

Relationship input shall already be in strict structural identity order by resource
type, resource ID, relation, subject type, subject ID, and optional subject relation.
The builder shall reject unordered or duplicate identities rather than silently sort
them. Caveat name and stored context remain committed data but do not change identity.
The sort key excludes the private snapshot salt.

### Canonical relationship

Version 1 shall encode the complete relationship and stored caveat context through
explicit field order, `u32` lengths and counts, option tags, and scalar tags. Stored
context entries shall use strict parameter-name order. Every record shall validate
against the exact approved PolicyIR before hashing. Raw Protobuf and JSON bytes shall
not enter the commitment.

### Leaf privacy

A private per-snapshot salt shall enter each leaf hash. The salt remains in the
snapshot secret material and private witness. One snapshot shall use one shared
32-byte salt. Each leaf shall bind that salt, its zero-based index, and the canonical
relationship bytes.

### Tree shape and root

The tree shall use the left-balanced unpadded shape from ADR-0009. Ordered child
digests shall enter the node hash. Empty snapshots shall use the versioned empty-tree
digest. The final root shall bind the tree digest, shared private salt, and exact
private leaf count. The descriptor shall publish only the 32-byte root.

### Membership witness

A private witness set contains:

```text
RelationshipWitnessSetV1 {
  format_version = 1
  snapshot_secret_salt
  leaf_count
  entries[]
}

RelationshipOpeningV1 {
  canonical_relationship
  leaf_index
  sibling_hashes[]
}
```

Entries shall use strict leaf-index order. The guest shall derive each child side and
the exact sibling count from `leaf_index` and `leaf_count`. Direction bits shall not
enter the witness. The guest shall reject duplicate indices, out-of-range or phantom
indices, malformed sibling counts, mixed salts, and unused entries.

Private-input accounting shall use checked arithmetic over:

```text
4  witness version
32 shared snapshot salt
4  leaf count
4  entry count

for each entry:
  4 + canonical relationship bytes
  4 leaf index
  4 sibling count
  32 * sibling count
```

Selected context entries shall remain within the approved PolicyIR context limit.
The derived total shall remain within `max_total_private_input_bytes`.

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
  trust_domain_id
  approval_epoch
  revoked_at
  revocation_reason
  registry_generation
}
```

The registry can use a local mounted directory in the PoC. Each record should have a
binary Protobuf form and an inspectable JSON projection. Signature verification uses
the canonical claim payload from ADR-0010, not Protobuf or JSON bytes.

## Capability semantics

### Scope

The baseline scope is:

```text
agent = agent:<lowercase hexadecimal key thumbprint>
task = task:review-482
purpose = reconciliation
action = query
resource_type = dataset
resource_scope = sg-payments
expires_at <= minimum(policy, snapshot, delegation, requested expiry)
```

`resource_scope` is one exact PolicyIR object ID in the baseline. Scenario adapters
must resolve product aliases before the agent signs the capability request.

### Key binding

The capability carries `agent_key_thumbprint`. Each protected request carries an
Ed25519 signature from that key. A copied capability without the agent private key is
not usable. The same thumbprint derives the PolicyIR subject. A friendly `agent_id`
cannot redirect evaluation to another subject.

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

The first connected M5 checkpoint exposes service state, job progress, public proof
fields, verification, cache behavior, denials, and receipts. The portal shall let the
operator create, edit, review, and finalize a bounded synthetic transaction dataset
for the existing reconciliation scenario. Finalization records a local source label,
checksum, schema fingerprint, and row count. Every related screen and receipt shall
display `SYNTHETIC`. These records may support the revised M6-M7 demonstration but
cannot establish real-data acceptance.

Transaction rows belong to the reconciliation data adapter. They are not SpiceDB
relationships and do not enter the authorization Merkle tree. Dataset finalization is
an operator workflow and audit event. It is not a policy-authority or
snapshot-authority signature. Policy approval, relationship changes, signed snapshot
publication, and capability issuance remain separate explicit actions.

M6-M7 coordinates relationship and snapshot steps from an approved agent request.
The planned operator flow can publish policy versions independently. Registration,
token management, signed identity lookup, and the request inbox pass locally.
Approval and request-triggered private preparation pass locally. Explicit M5
policy-version migration and the three-dashboard flow remain pending in the
[implementation plan](M6-M7-AGENT-DEMO-PLAN.md).

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

Deferred outside M6-M7 by ADR-0019. Preserve this test for later real-data validation.

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

Status: complete.

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

Status: complete.

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

Status: complete.

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

Status: complete on 2026-08-11. The public proof benchmark is recorded at
[`cluster-20260810T174732Z-76324.md`](../benchmarks/sp1-plonk/cluster-20260810T174732Z-76324.md).

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

Status: complete on 2026-08-26. The maintainer accepted
[ADR-0013](../decisions/0013-m4-service-and-verifier-execution.md) and
[ADR-0014](../decisions/0014-agent-key-thumbprint-and-issuance-identity.md) on
2026-08-12, then accepted
[ADR-0015](../decisions/0015-private-snapshot-and-one-shot-prover-boundary.md) on
2026-08-23, then accepted
[ADR-0016](../decisions/0016-runtime-snapshot-publication-and-spicedb-adapter.md)
on 2026-08-24, then accepted
[ADR-0017](../decisions/0017-durable-capability-cancellation-control.md) on
2026-08-24. The BullMQ `1.2.3` and Redis `8.2.8` conformance gate passes. Generated
Tonic `0.14.6` service contracts compile behind an explicit feature. The pure
`razield` admission boundary now performs typed request conversion, raw-key Ed25519
thumbprint derivation, exact commitment signature verification, deterministic job-ID
derivation, and 75-minute window admission. It has no Redis, filesystem, network, or
proof access. The separate private-envelope boundary stores only admitted
proof-worker fields in a strict, versioned Rust-only Protobuf message capped at 16
KiB. Its owner-only store uses atomic no-replacement publication, strict mode and link
checks, commitment-bound reads, idempotent deletion, and grace-based orphan cleanup.
The capability publisher now completes private-envelope durability before publishing
one deterministic BullMQ job. Its strict 256-byte public payload contains only a
format version and opaque private-envelope reference. Exact duplicates return the
authoritative retained record. Confirmed failures remove a newly created envelope,
and ambiguous queue outcomes preserve it for reconciliation. One-attempt, zero-stall,
global-concurrency-one, and bounded terminal-retention checks pass against live Redis.
The PoC admits only one distinct nonterminal proof job through one fixed atomic BullMQ
deduplication slot. A different request receives a typed capacity result and retains
no private envelope or queue job. M5 retries retain one operator-scoped submission
ID, reuse the exact signed request, and reconcile its commitment-derived job ID
through `GetCapability` before resending create.
The runnable `razield` ingress now validates explicit owner configuration and
reconciles private envelopes against exact BullMQ records before binding. Its Tonic
wrapper caps decoded messages at 2 MiB. `CreateCapability` uses trusted whole-second
server time, then calls admission and publication. It returns the authoritative
queued record without waiting for proof work. Exact duplicates return the retained
queued record. Bound mutations fail before publication, and display-only `agent_id`
changes do not change job identity. Terminal records fail closed until the worker
stores a bound public result. The proof supervisor now claims one job through
the official BullMQ worker and validates its exact public metadata before private
storage access. It loads one commitment-bound envelope, rechecks the full 75-minute
window with trusted whole-second time, records a closed `EXECUTING` phase, and passes
the private request to a narrow one-shot executor contract. A fixed one-hour deadline,
30-second termination grace, active cancellation, ordered envelope deletion, and
shutdown confirmation bound this handoff. Production startup constructs only the
fixed Unix-socket executor and strict terminal capability validator.
The B-0022 host builder now checks one selected snapshot's count, ordered records,
secret salt, and committed root before policy evaluation. It maps each validated leaf
index to the existing PolicyIR witness ID, evaluates the approved policy, opens only
relationships used by the first positive path, and validates the resulting
`RelationshipWitnessSetV1` against the selected root and declared limits. Denial,
invalid evaluation, malformed records, count excess, root mismatch, and invalid
constructed witnesses return typed errors with no private values. The builder does
not run SP1 and has no queue or service binding. The B-0023 service assembler resolves
the capability request's exact signed snapshot through `LocalRegistry`, checks the
request, approved policy, and snapshot against the fixed 75-minute proof window,
constructs the minimum witness, builds `AuthorizationGuestInputV1`, and passes the
shared host authorization statement. It retains the exact expected public journal for
the later proof result check. The B-0024 owner-only store now persists one immutable
private snapshot under its approved relationship-root digest. Every load strictly
decodes typed relationships and caveat values, binds the exact registry snapshot ID
and root, and rebuilds the blinded tree. The feature-gated `razield-prover` child uses
a strict 32 KiB request and 2 MiB result protocol over an owner-only Unix socket.
Fixed owner configuration supplies trusted paths, Plonk setup, trust domain, key ID,
and resource profile. The child loads `current.pb`, calls B-0023, executes the guest,
runs local CPU Plonk proving, and compares the complete public journal and key ID.
Its feature build and focused protocol, configuration, cgroup, and privacy tests pass
without running SP1. The default API graph excludes the SP1 SDK. The B-0025 fixed
launcher accepts one bounded request and starts one fixed child at a time. It confirms
child exit after cancellation, crash, malformed output, and closed typed failures.
The proof-runtime build prefetches and verifies SP1's pinned Gnark Go modules in a
build-only container with no repository or private-state mounts. The runtime build
stays networkless, mounts that module cache and its finalized manifest read-only, and
disables Go proxy access. The fetch step never writes to the immutable Cargo source.
The Compose proof profile applies the accepted 60,129,542,144-byte cap, zero swap,
eight CPUs, no network, read-only root, fixed trusted mounts, owner-only socket, and
bounded tmpfs. The supervisor deletes the private envelope before direct Plonk and
exact public-claim validation. Only then does BullMQ retain the public capability and
aggregate metrics. The B-0026 runtime now resolves one exact approved policy and
active snapshot authority key, reflects and reads every PolicyIR resource type at one
exact SpiceDB revision, creates a freshly salted blinded tree, and publishes the
immutable private snapshot before the signed public root. A separate networkless
registry writer owns the writable mount, accepts only append-root and revoke-root
operations over a `0600` Unix socket, and atomically publishes a validated complete
generation. `razield` reloads its read-only registry view before returning.
`BuildSnapshot`, `GetSnapshot`, and `RevokeSnapshot` pass a live Compose lifecycle
gate. The generated Authzed `v1.53.0` API passes against the pinned SpiceDB `v1.54.0`
server. The B-0027 runtime strictly maps retained BullMQ proof state into
`GetCapability`. `CancelCapability` publishes one durable public command before the
supervisor removes a waiting job or signals active work. Duplicate commands retain
one result. Terminal proof jobs remain immutable. Startup recovery completes cleanup
for commands retained before or after target removal. ADR-0002 is accepted. The
B-0029 `verifierd` admission boundary strictly decodes the public request against a
frozen descriptor, validates the agent key and exact protected-request signature,
applies trusted time and registry checks, and rebuilds the complete ADR-0011 expected
public journal. It sends only bounded public proof input to the next worker boundary.
The verification-only Rust feature and dependency-tree gate use direct
`sp1-verifier` `6.3.1` Plonk verification and exclude SDK proving, core execution,
native Gnark, JIT, and outbound SP1 client packages. B-0030 binds the strict decoder
to the live Tonic edge and sends one bounded request through an owner-only launcher
to a one-shot direct verifier. Its Compose sandbox applies the provisional 2 GiB,
zero-swap, no-network, read-only-root, bounded-tmpfs, 60-second profile. Controlled
tests contain child rejection, crash, malformed output, and timeout. Gateway nonce
consumption and the matching TypeScript canonical vector remain M5 work. These build
and controlled-process results do not replace the retained real-proof worker gate.
B-0031 retained an approved public-only Plonk proof and passed it through the isolated
verifier with every `razield` role stopped. The proof took 932,309 milliseconds and
peaked at 46,271,315,968 bytes. The verifier took 447 milliseconds, peaked at
8,146,944 bytes, used no swap, and rejected proof, journal, key, policy, root, scope,
expiry, and request-signature mutations. The public
[proof](../benchmarks/sp1-plonk/cluster-20260825T182353Z-18591.md) and
[verifier](../benchmarks/sp1-verifier/verifier-20260825T192214Z-34749.md) reports
record the retained-proof gate. B-0032 completed the signed-request, queue, prover,
terminal-capability, and verifier service run on the NUS VM. The production client
compiled and approved the synthetic policy, built an exact-revision snapshot,
submitted and deduplicated the signed request, generated one real local Plonk proof,
and published one terminal capability. The issuer and prover stopped before the
independent verifier accepted the capability. The verifier rejected an altered
capability and a stolen key binding. Proving took 955,516 milliseconds and peaked at
47,003,119,616 bytes under the 56 GiB cap with no swap or OOM event. The private job
envelope was deleted, and the owner-only private snapshot remained. The
[complete service-flow report](../benchmarks/m4-service-flow/m4-service-20260826T090300Z-199729.md)
records the final M4 gate.

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

Status: complete on 2026-08-30. The live NUS VM walkthrough created and finalized a
three-row synthetic dataset, compiled and activated a visual policy, published a
key-bound relationship snapshot, generated one local Plonk capability, ran one
verifier-backed query and one cache-backed query, rejected all five Denial Lab
requests without rows, filtered the public receipt ledger, and downloaded its JSONL
projection. The [M5 portal runbook](../runbooks/M5-LOCAL-PORTAL.md) records the public
evidence and screenshots. This historical synthetic gate does not establish the new
external-agent SDK flow or real-data acceptance.

Deliver:

- operational demo screens
- bounded local synthetic transaction editor and store
- synthetic dataset review, finalization, checksum, schema fingerprint, and row count
- gateway enforcement middleware
- verifier client
- capability cache
- nonce replay control
- JSONL receipts

Exit gate:

- first and cached requests execute
- out-of-scope and replayed requests fail

### M6: External agent access and public SDK

Status: in progress under ADR-0019. The former real-data objective is deferred.
The detailed B-0063 contract is accepted in [Agent API v1](AGENT-API-V1.md).
ADR-0020 is approved. B-0064 agent authentication and SDK foundation pass local
Rust, TypeScript, and live HTTP/Redis gates. B-0065 request storage and inbox pass
local checks. Owner-allocated version identities and namespaced candidate compilation
pass local checks. Composed owner publication passed against pinned SpiceDB with two
coexisting versions and an exact retry. The visual builder and exclusive-writer M6
preview passed locally. Connected operator approval and agent reads pass locally.
Request-triggered grant provisioning and signed snapshot preparation pass against
real SpiceDB. Exact retries retain the same snapshot. B-0065 is locally complete.
B-0066 is active. Serial preparation and proposal recovery pass local Redis/BullMQ
checks. Final signature acceptance persists the exact attempt before `SUBMITTING`,
with deadline recovery and changed-request rejection. Resume and protected-query
facade routes are implemented. Proof-service RPC handoff and terminal delivery pass
controlled local checks, including uncertain create, cancellation, exact resend,
restart, and retention cleanup. The external SDK signs exact proposals and submits
retained attempts. No connected real proof, complete SDK flow, or new VM deployment
is claimed.

Deliver:

- agent registration and separate tokens bound to registered public keys
- signed agent API and durable access-request lifecycle
- operator review, approve/reject, and independent policy publication
- request-triggered snapshots and sequential real proof dispatch
- external signer, typed client, polling, cancellation, retry, and capability reuse
- public `@gitaalekhyapaul/raziel-sdk-ts` package with standalone consumption example

Exit gate:

- externally keyed agents authenticate and access only their own requests
- approved requests bind policy, snapshot, final agent signature, and proof job
- two approved requests can wait and dispatch serially without duplicate proofs
- an externally installed SDK completes a proof-backed query and cached reuse

### M7: Three-dashboard reconciliation demonstration

Status: in progress under B-0067. Depends on the M6 live proof gate.

Both agent desktop dashboards, private persistent identities/tasks, packaged SDK
request/sign/query flows, the authenticated channel, bounded LLM tools, and editable
synthetic ledger seed are implemented. Local agent checks, seed/store checks, and
Docker/browser startup pass. The SoC API passed a bounded completion-connectivity
check, not a proof-backed business tool journey. Capability reuse with an editable
UTC query window retains the task, dataset, and agent binding. The
[three-dashboard runbook](../runbooks/M7-AGENT-DEMO.md) records the deployment path.
Two real connected capabilities, independent/cached queries, and the complete
three-dashboard recording remain pending. No production mock-proof flag was added.

Deliver:

- seeded and operator-editable processor and bank ledgers with synthetic labels
- separate Agent A and Agent B dashboards plus the operator dashboard
- configurable OpenAI-compatible LLM tools and a bounded task message channel
- reconciliation output with settlement references, amount differences, and receipts
- Docker deployment, VM walkthrough, screenshots, and repeatable demo runbook

Exit gate:

- both agents consume the public SDK and receive their own real Plonk capabilities from Raziel
- the operator reviews requests and publishes policies independently of proof execution
- both agents query protected data and reuse their scoped capabilities
- Agent A requests settlement evidence and Agent B returns a bounded result
- incorrect key/scope and replay requests return no protected rows
- all three dashboards support the recorded desktop demo with source and timing evidence

## Definition of done

The PoC is done only after all of these statements are true:

- Synthetic processor and bank sources are labeled, stored, and editable through the portal.
- SpiceDB and PolicyIR agree for the supported policy subset.
- A relationship snapshot and approved policy record are signed.
- `razield` generates a real SP1 proof of `ALLOW`.
- `verifierd` validates the proof without contacting `razield`.
- The agent uses a key-bound capability at the Next.js gateway.
- The gateway enforces scope and blocks replay.
- A cached request executes without a second proof verification.
- Two separate agent applications use the public SDK with persistent private keys.
- The operator registers public keys, issues tokens, and reviews access requests.
- Two real proofs run sequentially and support the shared reconciliation task.
- Three dashboards show the task, request, capability, query, and message flows.
- Tampering tests fail as specified.
- Private graph and witness data do not appear in public artifacts or logs.
- The baton and architecture documents match the running system.
- Benchmarks and an acceptance evidence bundle are checked in without private data.

## Deferred experiments

### Real-data acceptance and scenario extension

The former M6 real-data source selection and acceptance gate remain deferred. O-001
must close before real-data acquisition. The source manifest and real-data acceptance
test above remain the contract for that later work. Synthetic demo success does not
satisfy them. A second domain scenario is deferred until the revised reconciliation
demo passes M7 and the maintainer returns scenario extension to scope.

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

### TEE-backed local authorization

Revisit this experiment only after the M7 exit gate passes and the payment-
reconciliation PoC satisfies its full definition of done.

Future work can evaluate a local confidential PDP that lets an agent check its
task-bound access before work starts. The discussion shall cover:

- local capability inventory and pre-task access-needs checks
- enclave and confidential-VM trust boundaries
- remote attestation and attestation-bound secret provisioning
- self-hosted key-broker deployment on SEV-SNP or TDX
- protected state, revocation freshness, rollback, and query-oracle controls
- cold starts, pre-warming, warm-path latency, and service availability
- proof-carrying delegation options that do not require a TEE

Keep task-needs metadata, normative authorization, and stateful leakage controls as
separate artifacts. This experiment shall not change the baseline PoC.

### Verifier packaging

Later work can package the Rust verifier as N-API, WASM, or an embedded library after
compatibility and proof-size measurements.
