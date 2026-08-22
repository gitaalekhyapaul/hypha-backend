# Raziel PoC Architecture

Status: active baseline; M0 through M3 validated; M4 under implementation

Last updated: 2026-08-22

## Human-readable overview

### What we are building

Raziel is a proof-carrying authorization layer around SpiceDB for AI-agent data access.

SpiceDB remains the source of relationship state and permission-schema semantics. A
data-owner policy authority approves the canonical PolicyIR used by the verifier.
Raziel converts an `ALLOW` decision into cryptographic evidence that an independent
enforcement point can verify. The enforcement point does not need to trust an `ALLOW`
response from `razield`.

The first working scenario is payment reconciliation:

- A human principal receives access through a SpiceDB relationship path.
- The principal delegates a narrow task to an AI agent.
- SpiceDB stores a caveated, task-scoped agent grant.
- Raziel commits the approved policy and relationship snapshot.
- Raziel proves that the committed policy and snapshot authorize a capability scope.
- A separate verifier checks the proof without calling the prover.
- A gateway reuses the verified capability for rapid, bounded transaction queries.

The end-to-end demo must process a real, provenance-recorded transaction dataset.
Hand-written or generated transaction rows do not satisfy the demo acceptance gate.
Small fixtures remain valid for unit, adversarial, and equivalence tests. M5 can use
portal-created synthetic transaction records for its clearly labeled local
development walkthrough. M6 remains the real-data acceptance run.

### Current implementation status

| Milestone | Current state | Durable evidence |
| --- | --- | --- |
| M0 | Complete | Repository, pinned toolchain, contracts, and process shells |
| M1 | Complete | Strict SpiceDB compiler adapter, PolicyIR evaluator, and live equivalence |
| M2 | Complete | Canonical commitments, blinded snapshots, authority signatures, and local registries |
| M3 | Complete | Real local Plonk proof, independent verification, mutation checks, and public benchmark |
| M4 | Active implementation | Accepted service and identity contracts, passing admission, private-envelope, atomic-publication, Create-ingress, BullMQ supervisor, queue-conformance, generated-contract, and isolated-verifier build gates |
| M5 | Shells only | Next.js console and gateway have no service wiring |
| M6 | Pending | No approved real transaction source or acceptance run |
| M7 | Pending | Starts only after the M6 reconciliation gate |

The validated M3 run used the synthetic direct fixture. It produced a 2,537-byte
Plonk artifact in 1,742,106 milliseconds, used 46,301,016,064 bytes at peak, and
verified in 479 milliseconds. This closes the cryptographic M3 gate. It does not
claim real-data acceptance.

M5 can produce the first connected visual checkpoint once M4 services exist. The
portal can create and review synthetic transaction records in the existing
reconciliation scenario. It labels the dataset `SYNTHETIC`, records the local source,
checksum, schema fingerprint, and row count, then sends protected queries through the
gateway. This is a development demonstration, not the M6 acceptance run. It does not
create a second scenario.

### The key distinction from a signed token

A signed JWT says:

> A trusted issuer claims that this agent has permission.

A Raziel capability says:

> This approved policy and committed relationship snapshot evaluate to `ALLOW` for
> this capability scope, and this proof verifies that computation.

The policy authority signs the approved policy record. The snapshot authority signs
the relationship descriptor. Those signatures establish provenance. The ZK proof
establishes correct computation. The PoC can use one administrative owner with separate
key identifiers, but the protocol keeps the roles distinct.

### Baseline privacy model

| Boundary | Visible information |
| --- | --- |
| Public | Interpreter semantics, supported SpiceDB subset, capability format, proof system |
| Independent verifier and auditor | Exact approved PolicyIR, policy version, `policy_hash` |
| AI agent | `policy_hash`, public scoped claims, relationship snapshot root, expiry, proof |
| Raziel only | Relationship tuples, selected path, stored caveat context, Merkle openings |

The capability never carries the PolicyIR. The verifier resolves `policy_hash` in a
trusted local policy registry. The agent has no policy-registry or relationship-read
access.

Fully private PolicyIR is a later experiment. It is not part of the baseline PoC.

### Goals

1. Prove an `ALLOW` authorization against committed policy and relationship state.
2. Let an independent verifier validate that proof without calling `razield`.
3. Keep the relationship graph, satisfying path, caveat values, and Merkle openings private.
4. Bind the result to an agent key, task, purpose, action, resource scope, and expiry.
5. Reuse one verified capability for bounded rapid I/O until expiry.
6. Match SpiceDB for every operation in the supported PolicyIR subset.
7. Run the complete stack in Linux containers on an Apple Silicon development machine.
8. Complete the payment-reconciliation flow against a real transaction dataset.
9. Make new scenarios repeatable without changing proof-system code.

### Non-goals for the first PoC

- Full SpiceDB language compatibility.
- Proofs of `DENY`.
- Exclusion or authenticated non-membership.
- Recursive permissions or unbounded graph traversal.
- Wildcards, maps, lists, IP helpers, regex, or arbitrary CEL functions.
- Live SpiceDB Watch synchronization.
- Private dataset discovery or relevance proofs.
- Cumulative query privacy budgets.
- Production key management, availability, or security claims.
- External prover networks that receive private witnesses.
- A proof of the historical correctness of delegation issuance.

## System context

```mermaid
flowchart LR
    subgraph OWNER["Data-owner trust boundary"]
        S["SpiceDB"]
        C["SpiceDB compiler adapter\nGo build-time tool"]
        PA["Policy authority\nreview and signature"]
        R["razield API\nRust job controller"]
        Q["BullMQ and Redis\nowner-only queue state"]
        B["Private envelope store\nbounded owner-only blobs"]
        W["Rust proof supervisor\nBullMQ worker"]
        E["One-shot prover\nnetworkless CpuProver"]
        PR["Policy registry\napproved PolicyIR records"]
        S --> C
        C --> PA
        PA --> PR
        S --> R
        PR --> R
        R -->|"publish and query"| Q
        R -->|"write private envelope"| B
        Q <-->|"claim, renew, finish"| W
        B -->|"bounded private input"| W
        W -->|"spawn in proof cgroup"| E
        E -->|"bounded public result"| W
    end

    A["AI agent\nTypeScript SDK"] -->|"Create, Get, Cancel"| R
    R -->|"job state or capability"| A
    A -->|"scoped API request"| G["Next.js gateway / PEP"]
    G -->|"local verification"| V["verifierd\nindependent Rust sidecar"]
    PR --> V
    V -->|"verified claims"| G
    G -->|"authorized query"| P["Payments API and data adapter"]
    P -->|"transaction result"| G
    G -->|"result"| A
```

The `verifierd` process must never call `razield`. That invariant keeps proof
verification separate from issuer trust.

The API, BullMQ queue, private envelope store, supervisor, and one-shot prover are the
accepted M4 target in
[ADR-0013](docs/decisions/0013-m4-service-and-verifier-execution.md). Implementation
started with a passing queue conformance gate. The gate covers request-ID
deduplication, one-job concurrency, lock renewal, zero proof retries, cancellation,
progress, bounded retention, and authenticated Redis restart recovery. The split keeps
a 43 GiB proof operation away from the responsive API and queue supervisor. All roles
remain inside the data-owner trust boundary. The one-shot prover has no hosted-prover
or SP1-network path. The verification-only `raziel-prover` feature uses direct
`sp1-verifier` Plonk verification against a trusted paired program-key record. Its
checked dependency graph excludes the SDK, prover, core executor, native Gnark, JIT,
and outbound SP1 clients. A retained real proof must still pass through the isolated
worker before M4 can claim verifier behavior.

Generated Snapshot, Capability, and Verification clients and servers compile through
the opt-in `raziel-proto` `services` feature. Both Rust service crates select that
feature. The default message-only crate and authorization guest dependency graphs
exclude Tonic, Tokio, HTTP, Hyper, Tower, and the Prost service codec. This compiles
wire contracts only. It does not add handlers, listeners, transport policy, or service
behavior.

The pure `razield` admission boundary implements
[ADR-0014](docs/decisions/0014-agent-key-thumbprint-and-issuance-identity.md). It maps
generated request values into the ADR-0011 canonical model, derives the agent identity
from validated raw Ed25519 key bytes, verifies the signature over the exact request
commitment, derives the 64-character BullMQ job ID, and checks the accepted 75-minute
window. Its API has no queue, filesystem, network, witness, or proof access. The
bounded RPC handler calls this boundary before private-envelope storage or
publication.

The service-private envelope boundary accepts only that admitted type. It writes a
versioned Rust-only Prost message with a 16 KiB ceiling into a dedicated `0700`
directory and publishes a `0600` file without replacing an existing job envelope.
Reads reject links, non-regular files, wrong modes, oversized or non-canonical bytes,
wrong versions, invalid request fields, and commitment or reference mismatches. The
store supports idempotent deletion and grace-based orphan cleanup. This boundary has
no queue, network, witness, guest, or proof API.

The B-0019 publisher joins the admitted request, private store, and BullMQ queue.
It completes private-envelope durability before queue publication. Redis receives
only a format version and opaque envelope reference under a 256-byte payload ceiling.
The exact request commitment remains the custom job ID. The publisher fetches and
validates the authoritative Redis record after add or duplicate handling. Confirmed
failure removes a newly created envelope. An unreadable queue preserves it for later
reconciliation. The queue policy fixes one attempt, global concurrency one, zero
stalled retries, and terminal retention at 24 hours or 100 records. Live tests prove
waiting-state publication, exact duplicate retention, payload privacy, confirmed
failure cleanup, stalled failure, and Redis restart recovery.

The B-0020 service ingress validates explicit owner configuration and reconciles the
private store against exact BullMQ records before binding. Its generated Tonic wrapper
caps decoded messages at 2 MiB. `CreateCapability` takes trusted whole-second server
time, calls admission, publishes the job, and returns the authoritative queued record.
Malformed and oversized requests cause no publication or private-file side effect.
Exact duplicates return their retained queued record. Authorization-bound mutations
fail before publication. The display-only `agent_id` does not change job identity.
`GetCapability` and `CancelCapability` remain closed as unimplemented. Terminal queue
records fail closed until the worker slice supplies a capability or typed terminal
result.

The B-0021 library supervisor uses the official BullMQ worker for claim, lock renewal,
one-job concurrency, stalled handling, and terminal retention. It validates the exact
job name, identity, payload, and persisted queue policy before private storage access.
The claimed processor loads one commitment-bound envelope, rechecks the 75-minute
window with trusted whole-second time, publishes a closed `EXECUTING` phase, and calls
a narrow one-shot executor contract. A fixed one-hour deadline and 30-second exit
grace bound that handoff. Cancellation reaches active execution before envelope
deletion. Supervisor shutdown cancels active work, closes the worker, and confirms no
active jobs remain. A crate-owned activation token keeps the library boundary out of
production startup until the isolated executor and strict terminal result validator
exist. A successful placeholder handoff still records `SERVICE_UNAVAILABLE`, returns
no capability, and deletes the envelope. Real witness construction, proof execution,
capability issuance, lifecycle RPCs, and independent retained-proof verification
remain pending.

## Main components

| Component | Language | Responsibility | Must not do |
| --- | --- | --- | --- |
| `apps/demo-console` | Next.js and TypeScript | Human workflow, task creation, snapshot status, proof inspection, audit view | Evaluate authorization or hold graph secrets |
| `apps/demo-gateway` | Next.js route handlers and TypeScript | PEP, agent request authentication, scope checks, capability cache, payments API facade | Trust an unsigned `ALLOW` response |
| `packages/sdk-ts` | TypeScript | Agent key handling, capability request, proof-capability transport, request signing | Receive PolicyIR or graph data |
| `services/razield` | Rust | Snapshot orchestration, asynchronous job control, private witness construction, local SP1 proving, capability issuance | Act as the independent verifier |
| `services/verifierd` | Rust | Policy-registry lookup, proof verification, snapshot checks, claim validation | Call `razield` or read private graph data |
| `programs/authorization-guest` | Rust and SP1 | Check commitments, Merkle membership, PolicyIR evaluation, caveats, capability claims | Perform network or filesystem I/O |
| `crates/proto` | Rust | Compile generated Protobuf messages by default and generated Tonic contracts behind an explicit service feature | Define canonical commitment bytes or add service behavior |
| `crates/authorization-core` | Rust | Shared guest statement, request binding, witness usage, and public journal bytes | Read files, clocks, registries, keys, or networks |
| `crates/policy-ir` | Rust | PolicyIR types, validation, bounded evaluator | Parse arbitrary SpiceDB text |
| `crates/canonical` | Rust | Domain-separated canonical encodings and commitments | Hash raw Protobuf serialization |
| `crates/snapshot` | Rust | Tuple leaves, blinded Merkle tree, descriptor claim encoding and intrinsic validation | Sign claims or decide authorization |
| `crates/authority` | Rust | Host-only Ed25519 key loading, role-safe signing, strict verification, key identifiers, and trusted snapshot epoch allocation | Enter the SP1 guest graph or generate private keys |
| `crates/witness` | Rust | Find positive paths and build Merkle openings | Produce `DENY` proofs |
| `tools/raziel-admin` | Rust | Offline policy approval, key inspection, atomic registry publication, and one-way revocation | Generate keys or expose private material |
| `tools/spicedb-ir-compiler` | Go | Run SpiceDB's compiler and translate its protos into PolicyIR | Make runtime decisions or perform cryptography |
| `proto/raziel/v1` | Protobuf | Stable RPC and artifact contracts | Define canonical cryptographic bytes |
| `examples/reconciliation` | Schema and adapters | Payment scenario policy, source manifest, field mapping, demo configuration | Contain generated acceptance data |

## Why this stack

### One polyglot monorepo

The PolicyIR schema, SP1 guest, public values, verifier, Protobuf contracts,
TypeScript SDK, and test vectors form one compatibility unit. One repository lets a
single change update and test that unit atomically.

Repository separation comes later, after the proof format and API reach a stable,
versioned boundary.

### TypeScript and Next.js

TypeScript is the primary product and integration language. Next.js with the App
Router supplies the human demo console and protected gateway routes. The browser
does not call the Rust prover directly. Server-side TypeScript uses generated RPC
clients.

### Rust and SP1

Proof-critical semantics live in Rust. The host evaluator and SP1 guest share the
same PolicyIR types and pure evaluation code where the target permits it. This
reduces host-versus-guest semantic drift.

Plonk is the only approved proof mode for an agent-facing capability. The validated
profile provides computational zero knowledge, a small public artifact, and fast
independent verification. Its measured cost is about 29 minutes and 43.12 GiB for the
direct fixture on the four-CPU VM profile.

Core remains useful for execute-mode checks and non-sensitive internal measurements.
The pinned SP1 6.3.1 Core artifact carries witness-derived trace openings and has no
approved witness-hiding claim. It cannot enter `ProofCapability.zk_proof`, reach an
agent, or replace Plonk in a product demonstration.

This cost profile makes capability creation an asynchronous control-plane operation.
It does not run inside an HTTP request or a protected data query. One completed
capability supports many fast requests through independent verification and the M5
gateway cache.

### A small Go compiler adapter

SpiceDB's maintained schema compiler is written in Go and returns compiled
namespace and caveat Protobuf definitions. A small pinned adapter converts those
definitions into Raziel's bounded PolicyIR. Reimplementing the SpiceDB parser in
Rust would place avoidable risk at the policy boundary.

The adapter is a build-time or snapshot-publication tool. It is not a long-running
runtime service.

### Protobuf and Buf

Protobuf defines RPC and artifact shapes. Buf generates Rust and TypeScript code,
lints the schema, and detects breaking API changes.

Protobuf bytes are not canonical cryptographic bytes. `crates/canonical` defines
every commitment encoding explicitly and publishes cross-language test vectors.

### Independent verifier sidecar

The first verifier is a Rust sidecar. This avoids making Node native-module or WASM
packaging a blocker. A later packaging spike can expose the Rust verifier through
N-API or WASM after proof format and platform compatibility are measured.

## Administrative snapshot sequence

```mermaid
sequenceDiagram
    autonumber
    actor O as Data owner
    participant S as SpiceDB
    participant C as Compiler adapter (Go)
    participant P as Policy registry
    participant R as razield (Rust)

    O->>S: Write schema and relationships
    S-->>O: ZedToken
    O->>C: Compile schema text
    C->>C: Run pinned SpiceDB compiler
    C->>C: Reject unsupported operations
    C-->>O: PolicyIR and policy_hash candidate
    O->>O: Review and sign policy record
    O->>P: Register approved PolicyIR and signature
    O->>R: BuildSnapshot at ZedToken and policy_hash
    R->>P: Load approved policy record
    P-->>R: Canonical PolicyIR and approval signature
    R->>S: Read relationships at ZedToken
    S-->>R: Tuples, caveat context, revision
    R->>R: Canonicalize tuples
    R->>R: Build blinded relationship tree
    R->>R: Sign snapshot descriptor
    R-->>O: Snapshot ID, roots, revision, expiry
```

`razield` cannot mark a policy as approved. It can submit or consume a candidate, but
`verifierd` accepts only a policy record signed by the configured policy authority.

## Capability and request sequence

```mermaid
sequenceDiagram
    autonumber
    actor A as AI agent and SDK
    participant R as razield API
    participant B as private envelope store
    participant Q as BullMQ on Redis
    participant W as proof supervisor
    participant E as one-shot prover
    participant G as Next.js gateway
    participant V as verifierd
    participant P as Payments adapter

    A->>A: Build and sign capability request
    A->>R: CreateCapability
    R->>R: Validate envelope and admission limits
    R->>B: Persist bounded private envelope
    R->>Q: Publish job with deterministic ID
    R-->>A: Job ID and QUEUED state
    W->>Q: Claim one job and renew its lock
    W->>Q: Record EXECUTING
    W->>E: Launch with bounded private envelope
    E->>E: Resolve snapshot and build private witness
    W->>Q: Record PROVING
    E->>E: Execute and prove SP1 guest
    E-->>W: Return bounded public result
    W->>B: Delete private envelope
    W->>Q: Record SUCCEEDED and public capability

    loop Poll until terminal state
        A->>R: GetCapability
        R->>Q: Read public job state
        R-->>A: State, metrics, or capability
    end

    A->>G: Query, capability, nonce, signature
    G->>V: VerifyCapability
    V->>V: Resolve policy_hash locally
    V->>V: Verify policy registration and proof
    V->>V: Verify snapshot, key binding, expiry, nonce

    alt Valid capability
        V-->>G: Verified claims
        G->>G: Check request is within claims
        G->>G: Cache claims until expiry
        G->>P: Run authorized query
        P-->>G: Result
        G-->>A: Result
    else Invalid capability
        V-->>G: Typed verification failure
        G-->>A: Generic access denial
    end

    loop Later scoped requests
        A->>G: Query, capability ID, nonce, signature
        G->>G: Match agent-key thumbprint
        G->>G: Verify signature, timestamp, nonce, and scope
        G->>P: Run authorized query
        P-->>A: Result through gateway
    end
```

`CreateCapability` returns after admission and durable queueing. It does not wait for
proof generation. BullMQ enforces one active proof job. A separate networkless child
runs the validated M3 profile. The exact queue format, cancellation rules, process
caps, and capability time window are accepted in
[ADR-0013](docs/decisions/0013-m4-service-and-verifier-execution.md).

## Core data commitments

### Policy commitment

The approved baseline keeps PolicyIR visible to the verifier and auditor. The agent
sees only its hash.

```text
policy_hash = H(
  "raziel/policy/v1",
  canonical_policy_ir
)
```

The policy registry stores canonical PolicyIR, version, signed approval claims,
mutable status, and registry generation. `verifierd` recalculates `policy_hash`,
strictly verifies the authority signature, and checks the current local trust state
before accepting the record.

### Relationship snapshot commitment

```text
canonical_relationship_v1 =
  u32be(1) ||
  resource_type,
  resource_id,
  relation,
  subject_type,
  subject_id,
  subject_relation,
  caveat_name,
  canonical_stored_caveat_context

relationship_leaf = H(
  "raziel/relationship-leaf/v1",
  u32be(1) ||
  u32be(leaf_index) ||
  bytes(snapshot_secret_salt) ||
  bytes(canonical_relationship_v1)
)

relationship_node = H(
  "raziel/relationship-node/v1",
  u32be(1) ||
  bytes(left_digest) ||
  bytes(right_digest)
)

relationship_snapshot_root = H(
  "raziel/relationship-root/v1",
  u32be(1) ||
  u32be(private_leaf_count) ||
  bytes(snapshot_secret_salt) ||
  bytes(left_balanced_tree_digest)
)
```

The private snapshot salt reduces dictionary attacks against small, guessable
relationship sets. Records use strict structural identity order before hashing.
Duplicate identities fail. The tree is left-balanced with no padding or duplicated
leaf. The private count fixes the unique opening path and prevents phantom odd-leaf
indices. Only the root and signed descriptor become public. ADR-0009 defines the
complete encoding, empty-tree digest, limits, and opening rules.

### Relationship snapshot descriptor

```text
RelationshipSnapshotDescriptor {
  format_version
  trust_domain_id
  snapshot_id
  relationship_snapshot_root
  policy_record_id
  policy_hash
  spicedb_revision
  epoch
  issued_at
  expires_at
  canonicalization_version
  signing_key_id
  authority_signature
}
```

The root commits one relationship snapshot. It does not track requests, prove
freshness by itself, or establish that SpiceDB contained the tuples. The snapshot
authority signature attests that provenance. Expiry and the verifier's accepted-root
registry control freshness and revocation. The descriptor binds one exact approved
policy record inside one deployment trust domain.

### Capability request commitment

```text
capability_request_commitment = H(
  "raziel/capability-request/v1",
  agent_key_thumbprint,
  task_id,
  purpose,
  action,
  resource_type,
  resource_scope,
  requested_expiry,
  issuance_nonce,
  snapshot_id,
  request_context
)
```

The proof binds an authorization envelope, not each transaction query. The gateway
checks that every API request remains inside the proven envelope. Request context is
private, typed, strictly ordered, and covered by the commitment. ADR-0011 defines the
exact framing, scope hash, and issuance nullifier.

The proof subject is `agent:<lowercase hexadecimal key thumbprint>`. A friendly
`agent_id` is service metadata and cannot select a PolicyIR subject. The action is the
PolicyIR permission name. `resource_type` and `resource_scope` form one exact PolicyIR
object reference.

## PolicyIR baseline

The first PolicyIR supports:

- direct relationship references
- subject-set relationships
- union
- intersection
- one bounded arrow
- named caveats
- string, integer, and Boolean values
- equality and ordered integer comparisons
- Boolean `and`, `or`, and `not`
- explicit evaluator limits

The first PolicyIR rejects:

- exclusion
- recursion
- wildcards
- unbounded traversal
- floating-point values
- maps and lists
- IP and regex helpers
- arbitrary CEL functions
- unknown or extension fields

The compiler must fail closed. It must never approximate an unsupported SpiceDB
operation.

## Capability token model

The capability is a Protobuf artifact on internal RPC boundaries. O-005 still governs
its HTTP representation, so the baseline does not yet select base64url or another
browser-facing encoding. It is not a JWT.

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

The agent signs a versioned, length-prefixed protected-request envelope:

```text
ProtectedRequestSignatureInput {
  format_version = 1
  capability_id
  request_method
  canonical_request_target
  request_body_hash
  request_nonce
  request_timestamp
}
```

The canonical target binds the normalized path and query parameters. Every protected
request carries the agent public key. The gateway recomputes its thumbprint and matches
the capability claim before signature verification. Cold requests pass the same
envelope to `verifierd`. Cache hits verify the envelope in the gateway and skip only
proof verification. [ADR-0002](docs/decisions/0002-protected-request-signature-envelope.md)
defines the proposed canonical rules.

The exact request commitment identifies duplicate BullMQ jobs. The issuance nullifier
remains a proof-bound conflicting-issuance identifier and is not consumed on first
use. The request nonce prevents replay of individual calls.

## Repository layout target

```text
apps/
  demo-console/               Next.js human console
  demo-gateway/               Next.js PEP and payments routes
packages/
  sdk-ts/                     agent SDK
  proto-ts/                   generated TypeScript contracts
services/
  razield/                    Rust prover service
  verifierd/                  Rust verifier sidecar
crates/
  authorization-core/
  canonical/
  policy-ir/
  snapshot/
  witness/
  prover/
  verifier/
programs/
  authorization-guest/       SP1 program
tools/
  spicedb-ir-compiler/        Go CLI
proto/
  raziel/v1/
examples/
  reconciliation/
tests/
  vectors/
  e2e/
docs/
  agent/
  decisions/
  spec/
```

## Architecture invariants

The canonical list is [the 16-item repository invariant
set](AGENTS.md#architecture-invariants). It is part of this architecture by reference.
No shortened list replaces it. A proposed change needs a decision record and explicit
maintainer approval.

## Security and trust limits

The baseline proves correct evaluation against committed inputs. It does not prove:

- that the source application wrote correct relationships to SpiceDB
- that the snapshot authority read SpiceDB correctly
- that human delegation policy was lawful or well-governed
- that an allowed query is free from inference risk across many calls
- that a compromised gateway will enforce capability scope
- that a stolen snapshot signing key has not issued a malicious root

These limits belong in demo and research reporting. They must not be hidden behind a
general claim of trustlessness.

## Agent-oriented implementation dump

### Source-of-truth order

1. `AGENTS.md` for repository operating rules.
2. `docs/agent/CURRENT-BATON.md` for active work state.
3. `ARCHITECTURE.md` for boundaries and invariants.
4. `docs/spec/POC-TECHNICAL-SPEC.md` for implementation requirements.
5. Protobuf files for wire contracts.
6. Rust canonical and PolicyIR crates for cryptographic semantics.
7. Scenario documents for domain mapping.

If prose conflicts with executable tests, stop and record the mismatch. Do not silently
change cryptographic semantics to satisfy one test.

### Trust-zone ownership

| Zone | Reads secrets | Produces trusted artifact |
| --- | --- | --- |
| SpiceDB and snapshot builder | Schema, relationships, stored caveat context | Signed snapshot descriptor |
| `razield` | Snapshot data, private path, openings | ZK proof and capability |
| Policy authority and registry | Candidate schema, approved canonical PolicyIR | Signed policy record |
| `verifierd` | Approved PolicyIR, public proof values | Verified claims or typed failure |
| Gateway | Public claims, request body, cache | Enforced API decision and audit receipt |
| Agent | Public capability and its own private key | Signed capability and API requests |

### Required public values from the SP1 guest

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

No private tuple, path identifier, caveat value, or Merkle sibling may enter public
values or logs. The guest commits the exact ADR-0011 byte encoding once with
`commit_slice`. Protobuf remains a service wire model and is not the SP1 journal
encoding.

### Host and guest split

Host responsibilities:

- resolve the requested policy and snapshot
- search for one satisfying positive path
- collect each relationship used by that path and its Merkle opening
- collect stored and request-time caveat inputs
- enforce size and depth limits before proving
- validate the candidate input through `authorization-core`
- supply private and public inputs to SP1
- package proof output without reinterpreting `ALLOW`

Guest responsibilities:

- validate version and limit fields
- recompute `policy_hash`
- verify every Merkle membership path
- evaluate the bounded PolicyIR
- evaluate the bounded caveat IR
- derive the subject from the agent key thumbprint
- derive capability claims from the issuance envelope
- bind task, purpose, action, resource scope, nonce, snapshot, context, and expiry
- commit `ALLOW` and the exact public values
- fail on malformed, unused, ambiguous, or excess witness data

Verifier responsibilities:

- resolve `policy_hash` in the local registry
- validate policy status, signature, and validity window
- validate snapshot authority, root status, and expiry
- verify the SP1 proof against the selected verification key
- compare public values with capability claims
- validate agent key binding and request anti-replay data
- return claims, never an unexplained Boolean

### Positive-proof constraint

The first guest proves only `ALLOW`. It receives the positive relationships needed by
the supported expression. Intersection requires positive evidence for both branches.
Union uses the first canonical successful branch after every branch has run under M1
error semantics. The evaluator returns the exact leaf IDs used by that path. The guest
rejects any supplied opening outside that set. The compiler does not emit exclusion
nodes.

`DENY` responses come from failed proof creation or verification and carry no ZK proof.
User-facing denials remain generic. Internal logs use typed reason codes without graph
paths or policy explanations.

### Real-data acceptance boundary

The reconciliation acceptance run must have:

- a named external or user-provided source
- source URL or custody record
- license or usage basis
- retrieval date
- file checksum
- immutable raw-data location outside generated build output
- documented column mapping
- redaction and secret-scanning check
- deterministic import command
- recorded row count and schema fingerprint

Tests can use minimal fixtures under `tests/fixtures`. Product screenshots, performance
claims, and acceptance reports must state which data source they used.

### Scenario extension rule

A new scenario adds domain configuration and adapters. It must not fork the proof guest.
See `docs/spec/SCENARIO-AUTHORING.md` and the `raziel-scenario` repo skill.

### Completion gates

Architecture work is complete only after:

- all Mermaid blocks parse
- every referenced file exists
- public and private fields agree across architecture, spec, and Protobuf
- the baton records exact next work

Implementation work is complete only after:

- unit and integration tests pass
- SpiceDB-versus-PolicyIR equivalence tests pass
- a valid proof verifies in `verifierd`
- altered policy hash, root, scope, proof, expiry, and signature each fail
- `verifierd` completes with `razield` stopped
- cached capability requests remain inside scope
- the real-data reconciliation journey completes end to end
- benchmark output records execution, proving, verification, proof size, and cache latency
