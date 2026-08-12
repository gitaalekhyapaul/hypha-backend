# ADR-0011: SP1 Guest Input and Public Values

- Status: accepted
- Date: 2026-07-24
- Milestone: M3
- Approved by maintainer: 2026-07-24
- Execute-mode host amendment approved by maintainer: 2026-07-24

## Context

M2 fixed the approved PolicyIR commitment, blinded relationship tree, signed
snapshot descriptor, and verifier-local authority registries. M3 must now define the
statement proved by the SP1 program.

The Protobuf contract names the fields in `AuthorizationPublicValues`. It does not
define:

- the private guest input
- the exact bytes committed to the SP1 public journal
- the binding between an agent key and the PolicyIR subject
- the request and scope commitment formulas
- the source of request-time and capability caveat values
- the rule that rejects valid but unused Merkle openings
- guest-owned ceilings and failure behavior

Leaving any of these choices to the prover would let it change the authorization
question or provide values that satisfy a caveat without a matching signed request.

## Decision

### Shared proof statement

Add `crates/authorization-core` as the only shared authorization orchestration crate.
The SP1 guest and the local proving host both call this crate.

The crate owns:

- the versioned private input model
- guest-owned ceilings
- request validation and PolicyIR request derivation
- request, scope, and nullifier commitments
- conversion of verified openings into the PolicyIR resolver
- deterministic positive-path usage accounting
- exact public journal encoding and strict decoding
- the complete `ALLOW` statement

`authorization-core` depends only on guest-compatible policy, canonicalization, and
snapshot crates. It cannot depend on the SP1 SDK, authority keys, registries, service
code, files, clocks, or network clients.

`crates/witness` remains host-only. It will search the private relationship source and
assemble candidate openings. It cannot decide that a candidate input is valid.

`crates/prover` remains host-only. It validates an input with `authorization-core`,
serializes it into SP1 stdin, executes the guest locally, and compares the returned
journal bytes with the host result.

### Private guest input

The typed input is `AuthorizationGuestInputV1`:

```text
AuthorizationGuestInputV1 {
  format_version = 1
  policy
  expected_policy_hash
  relationship_snapshot_root
  relationship_witness_set
  request
  issued_at_unix_seconds
  expires_at_unix_seconds
}

CapabilityRequestBindingV1 {
  format_version = 1
  agent_key_thumbprint
  task_id
  purpose
  action
  resource_type
  resource_scope
  requested_expires_at_unix_seconds
  issuance_nonce
  snapshot_id
  request_context[]
}
```

`agent_key_thumbprint`, every digest, and `issuance_nonce` are exactly 32 bytes.
Timestamps are signed Unix seconds in UTC. Leap-second representation is outside the
protocol.

The input uses SP1's typed serde transport. Those transport bytes are private hints.
They are not commitment inputs and are not a stable Raziel wire format. The host
creates the typed value and applies all protocol limits before serialization. The
guest repeats semantic validation after deserialization.

The guest input derives `Debug` with all private fields redacted. Guest and host errors
must not format the input, PolicyIR, relationships, caveat values, salts, openings, or
resolver state.

### Guest-owned ceilings

The following ceilings are compiled into `authorization-core`. The prover cannot
raise them:

```text
max_expression_nodes = 64
max_caveat_nodes = 64
max_path_depth = 4
max_relationship_witnesses = 32
max_merkle_depth = 32
max_string_bytes = 256
max_context_entries = 16
max_total_private_input_bytes = 4096

max_definitions = 16
max_relations_per_definition = 16
max_allowed_subject_types_per_relation = 16
max_permissions = 32
max_caveats = 16
max_parameters_per_caveat = 16
max_syntax_depth = 16

max_snapshot_relationships = 256
max_relationship_bytes = 4096
max_snapshot_canonical_bytes = 1048576
max_canonical_policy_bytes = 1048576
```

The policy carries declared limits. Each declared value must be nonzero and at or
below the compiled ceiling. Snapshot verification calculates opening depth and
private byte use from the witness. The prover cannot supply those metrics.

### Agent subject binding

The authorization subject is derived only from the agent key thumbprint:

```text
subject.object_type = "agent"
subject.object_id = lowercase_hex(agent_key_thumbprint)
```

The friendly `agent_id` service field is metadata. It does not select the PolicyIR
subject and does not enter proof evaluation. A relationship that grants access to
`agent:recon-17` must migrate to the key-derived subject before it can support proof
issuance.

This rule removes the need for a separate trusted key-to-agent registry in the first
proof of concept. A later identity registry needs its own decision record.

### Authorization target binding

The PolicyIR request is derived without a scenario-specific guest mapping:

```text
resource.object_type = request.resource_type
resource.object_id = request.resource_scope
permission = request.action
subject = key-derived agent subject
```

The baseline treats `resource_scope` as one exact PolicyIR object ID. Ranges, wildcard
scopes, query expressions, and adapter-defined scope expansion are unsupported.
Scenario adapters translate product names into these canonical fields before the
agent signs the issuance request.

### Caveat input binding

Request-context entries are private but enter the signed request commitment. They are
strictly sorted by name, unique, typed, and limited by the guest profile.

Relationship-context entries come only from committed relationship records in
verified Merkle openings.

Capability-claim entries are derived inside `authorization-core`. The prover cannot
supply a separate capability-claim collection. Version 1 supports these names:

| Name | Type | Source |
| --- | --- | --- |
| `agent_key_thumbprint` | string | lowercase hexadecimal thumbprint |
| `task_id` | string | request field |
| `purpose` | string | request field |
| `action` | string | request field |
| `resource_type` | string | request field |
| `resource_scope` | string | request field |
| `snapshot_id` | string | request field |
| `requested_expires_at` | integer | request field |
| `issued_at` | integer | issued capability field |
| `expires_at` | integer | issued capability field |

A PolicyIR caveat that declares another `CAPABILITY_CLAIM` name fails guest
validation. The compiler can preserve such a name for review, but M3 cannot prove it.

### Capability request commitment

The request commitment binds every value that can change authorization:

```text
domain = ASCII("raziel/capability-request/v1")

payload =
  u32be(format_version) ||
  bytes(agent_key_thumbprint) ||
  string(task_id) ||
  string(purpose) ||
  string(action) ||
  string(resource_type) ||
  string(resource_scope) ||
  i64be(requested_expires_at_unix_seconds) ||
  bytes(issuance_nonce) ||
  string(snapshot_id) ||
  context_collection(request_context)

capability_request_commitment = SHA-256(
  u32be(len(domain)) || domain ||
  u32be(len(payload)) || payload
)
```

`bytes` and `string` use a `u32be` byte-length prefix. A context collection starts
with its `u32be` count. Each entry encodes its name, then a one-byte value tag and its
value:

- Boolean tag `01`, then `00` or `01`
- integer tag `02`, then signed `i64be`
- string tag `03`, then the length-prefixed UTF-8 bytes

No Unicode normalization occurs.

The 32-byte issuance nonce is mandatory. It prevents practical dictionary testing of
a public request commitment that covers low-entropy private context.

The M4 issuance API must sign this exact commitment or an envelope that contains it.
Signature verification remains M4 service work.

### Capability scope hash

The public scope hash binds the final issued envelope:

```text
domain = ASCII("raziel/capability-scope/v1")

payload =
  u32be(format_version) ||
  bytes(agent_key_thumbprint) ||
  string(task_id) ||
  string(purpose) ||
  string(action) ||
  string(resource_type) ||
  string(resource_scope) ||
  string(snapshot_id) ||
  i64be(issued_at_unix_seconds) ||
  i64be(expires_at_unix_seconds)
```

The verifier recomputes this hash from the capability. Policy hash and relationship
root remain separate public fields and are compared separately.

### Issuance nullifier

The issuance nullifier is deterministic for one signed issuance request:

```text
domain = ASCII("raziel/issuance-nullifier/v1")
payload = bytes(capability_request_commitment)
issuance_nullifier = SHA-256(framed(domain, payload))
```

The service retains this digest as a proof-bound issuance conflict identifier and SP1
proof nonce input. ADR-0014 assigns BullMQ duplicate identity to the exact request
commitment, so this digest is not the queue job ID. The service does not consume it
after one protected request. Per-request replay control uses the protected-request
nonce from ADR-0002.

### Issued time checks

The guest requires:

```text
issued_at_unix_seconds >= 0
issued_at_unix_seconds < expires_at_unix_seconds
expires_at_unix_seconds <= requested_expires_at_unix_seconds
```

The guest has no trusted clock. `verifierd` later checks current time, policy
validity, snapshot validity, key validity, revocation, and deployment lifetime caps.

### Verified relationship resolver

The guest validates the PolicyIR with the compiled ceilings, recomputes
`policy_hash`, and compares it with `expected_policy_hash`.

It then verifies every relationship opening against
`relationship_snapshot_root`. The resolver is built only from those verified
records. Each resolved relationship receives its Merkle leaf index as its witness ID.
The resolver groups entries by exact resource and relation. It preserves canonical
relationship order.

The witness set and resolver both enforce ADR-0009 structural identity order. Caveat
name and context do not alter identity. Two openings for the same resource, relation,
subject, and optional subject relation fail even when one is direct and one is
caveated.

The generic PolicyIR resolver no longer accepts caller-reported Merkle depth or
private byte counts. Snapshot verification owns those checks.

### Deterministic positive-path usage

The PolicyIR evaluator gains a traced result:

```text
EvaluationOutcome {
  allowed
  used_witness_ids[]
}
```

The existing Boolean API calls the traced evaluator and discards the trace. Boolean
and CEL error semantics do not change.

The traced evaluator uses these deterministic rules:

- a direct relation uses the first canonical matching relationship
- a caveated relation evaluates every canonical candidate and uses the first true
  candidate
- a subject set uses the first canonical successful source and target path
- an arrow uses the first canonical successful source and target path
- a union evaluates every child and uses the first allowed child
- an intersection evaluates every child and unions all successful child usage
- one opening can satisfy more than one intersection obligation
- usage IDs are returned strictly sorted and unique

Relationship-witness count and stored relationship-context count are charged once per
present witness ID. A later resolver visit still validates and evaluates the
relationship, but it cannot charge the same private opening twice. Host-only
relationships without a witness ID retain per-visit accounting.

Every branch still runs under the accepted M1 error and budget semantics. Selecting a
positive trace does not absorb a non-missing error that the Boolean evaluator would
return.

After an `ALLOW`, the guest requires the used witness IDs to equal the set of supplied
opening leaf indices. A missing, duplicate, unknown, or unused opening fails. Host-side
filtering cannot replace this guest check.

### ALLOW-only statement

The guest performs these steps in order:

1. Validate input version, request fields, context order, lengths, nonce, and time
   relation.
2. Validate PolicyIR against compiled ceilings.
3. Recompute and compare `policy_hash`.
4. Check the canonical PolicyIR byte ceiling.
5. Verify every relationship opening and the snapshot root.
6. Build the resolver from verified records.
7. Derive the PolicyIR request and fixed capability claims.
8. Evaluate PolicyIR and caveats with usage tracking.
9. Require `allowed = true`.
10. Require exact witness consumption.
11. Compute request commitment, scope hash, nullifier, and public values.
12. Commit the public journal exactly once.

False evaluation, conditional evaluation, missing input, malformed input, excess
input, commitment mismatch, and unused witness data all abort before a journal
commit. `ALLOW` is a constant output. It is never accepted as guest input.

### Public journal bytes

The guest commits one byte slice with `sp1_zkvm::io::commit_slice`. It does not call a
typed SP1 commit function for public values.

Version 1 bytes are:

```text
bytes(ASCII("raziel/authorization-public-values/v1")) ||
u32be(format_version = 1) ||
bytes(policy_hash) ||
bytes(relationship_snapshot_root) ||
bytes(capability_request_commitment) ||
u8(decision = 1) ||
bytes(capability_scope_hash) ||
i64be(issued_at_unix_seconds) ||
i64be(expires_at_unix_seconds) ||
bytes(issuance_nullifier)
```

Each digest is exactly 32 bytes and retains its `u32be` length prefix. The decoder
rejects another domain, another version, a zero or unknown decision, a wrong digest
length, a negative timestamp, an invalid time interval, truncated input, and trailing
bytes.

The Protobuf message remains a presentation and service wire model. Raw Protobuf
bytes never enter the journal or a commitment.

### SP1 host and guest boundary

Pin the host SDK to the existing SP1 release:

```toml
sp1-sdk = { version = "=6.3.1", default-features = false, features = ["blocking"] }
sp1-zkvm = { version = "=6.3.1", default-features = false, features = ["lib"] }
```

The `blocking` host feature avoids a direct async runtime in `crates/prover`. SP1
still owns its internal runtime. Network proving features remain disabled. Execute
mode constructs the concrete local `LightProver` rather than an environment-selected
client. `LightProver` executes and verifies but cannot generate proofs. The next M3
proof-generation slice will construct the concrete local `CpuProver`. Private witness
data cannot leave through an SP1 network proving client.

SP1 `6.3.1` execute requests keep gas reporting enabled. That path returns instruction
and syscall metrics and enforces the configured cycle limit. The host independently
rejects a non-success exit code from the returned execution report.

SP1 retains HTTP and RPC libraries in its transitive host dependency graph. This
decision establishes a feature and code-path boundary, not an operating-system egress
sandbox. M4 must define process-level network isolation before a service handles
private witness data.

The host accepts caller-provided guest ELF bytes or a reviewed local path. B-0010 does
not add a nested `sp1-build` build script. The existing pinned `cargo prove build`
command builds the guest first.

Execute mode must:

- serialize exactly one `AuthorizationGuestInputV1` into `SP1Stdin`
- execute the supplied authorization guest ELF locally
- obtain the committed `SP1PublicValues` bytes
- compare them byte-for-byte with the host `authorization-core` result
- return typed host errors and cycle metrics without logging private input
- reject zero or caller-selected cycle limits above the compiled maximum
- reject every non-success execution-report exit code

Proof creation and verification-key binding remain the next M3 slice.

The numeric guest ceilings remain semantic acceptance bounds. The 100,000,000-cycle
value is a provisional measured operating maximum for the pinned B-0010 guest and
SP1 `6.3.1` executor:

- the six supported positive paths use 422,138 to 518,217 instructions
- the combined accepted ceiling fixture uses 94,608,186 instructions
- that fixture carries a 1,046,611-byte canonical policy, 16 witnesses, 3,790 of
  4,096 private witness bytes, and 16 request-context values at the 256-byte string
  limit
- the selected permission contains 33 nodes and follows an arrow into one
  subject-set node, the policy contains 64 permission nodes, and the active caveat
  evaluates 34 of the policy's 64 caveat nodes
- the selected caveat reads all 16 request-context values, and the selected path
  reaches depth 3, the deepest path in the approved one-arrow subset
- the same policy with a 256-relationship snapshot and three depth-8 openings uses
  93,284,394 instructions

The largest measured case leaves 5,391,814 instructions below the compiled maximum.
These fixtures combine the high-cost dimensions that can coexist, but they do not
prove that every semantically accepted combination completes below the limit. An
input that exhausts the execution budget fails closed without an `ALLOW` journal.
Any guest, SP1 dependency, transport, canonical encoding, or ceiling change must
rerun these boundary measurements or amend this decision.

## Trust and Privacy Effects

- The prover still sees private PolicyIR and witness data.
- The SP1 proof reveals only the public journal.
- The agent receives no PolicyIR, relationships, openings, salts, caveat values, or
  witness trace.
- The key thumbprint selects the authorization subject without a separate identity
  registry.
- Request context remains private but is bound by a nonce-hardened public commitment.
- Public cycle counts and job timing can reveal coarse path complexity. Service and
  UI work must not expose per-branch timing or private trace diagnostics.
- Guest panic text is constant and contains no private values.

## Test Requirements

M3 tests must cover:

- stable request commitment, scope hash, nullifier, and public journal vectors
- strict public journal decoding and trailing-byte rejection
- key-derived subject mapping
- every fixed capability-claim name and rejection of another name
- malformed, unsorted, duplicate, oversized, and empty request context
- invalid nonce, timestamps, policy hash, and snapshot root
- positive direct, caveated, union, intersection, subject-set, and arrow paths
- deterministic first-path choice
- shared opening use across intersection branches
- missing, duplicate, and unused witness rejection
- one-field mutations of every public value and every witness field
- host and guest execute-mode equality
- exact guest status and an empty journal for typed semantic rejection and malformed
  serialized input
- host rejection of mutated, truncated, and extended journal bytes after a successful
  guest exit
- guest dependency-tree exclusion of host authority, verifier, service, and network
  crates

## Consequences

- Existing friendly agent relationship IDs must migrate to key-derived IDs.
- `CreateCapabilityRequest` service wiring will need private typed request context and
  a signature over the canonical request commitment in M4.
- Scenario adapters cannot place a scope parser or agent identity mapping inside the
  guest.
- A policy with an unsupported capability-claim name can compile for review but
  cannot produce an M3 capability.
- Public journal bytes are now a versioned proof-system protocol. A field, order,
  type, or encoding change needs a new decision record and mutation vectors.

## Rejected Options

### Trust a host-computed ALLOW

This would prove only that the guest received a Boolean. It would not prove policy,
snapshot, caveat, or request evaluation.

### Let the prover supply an agent ID

A self-asserted ID is not bound to the signing key. It can select another subject's
relationships.

### Add a key-to-agent registry in M3

This adds a new signed authority surface and a second identity lookup. The first proof
of concept can use the key thumbprint directly.

### Let the prover supply capability claims

The prover could choose values that satisfy a caveat. Version 1 derives claims from
the public issuance envelope.

### Use a private witness-plan expression

A second expression tree would duplicate PolicyIR semantics and create another
proof-critical interpreter. Evaluator-produced usage tracking keeps one semantic
implementation.

### Stop after the first successful union branch

This would change the accepted error and budget semantics from M1. Version 1
evaluates every branch and selects the first successful trace only after evaluation.

### Accept unused but valid openings

Extra openings can smuggle unrelated private relationships into the proof input and
make the proved path ambiguous. The guest rejects them.

### Commit typed SP1 or Protobuf public values

Those encodings are dependency-controlled transport formats. Raziel commits its own
fixed canonical bytes.

### Add an SP1 build script

`sp1_build` would compile the guest during host builds and add nested toolchain work.
The existing container command already builds the guest in a separate pinned step.
