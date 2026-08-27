# ADR-0018: Bounded Policy Draft And Owner Activation

- Status: accepted
- Date: 2026-08-27
- Milestone: M5
- Related baton: B-0036
- Security review triggers: client-visible policy artifact, policy registry trust,
  authority keys, relationship writes, and snapshot publication

## Context

M1 supplies a strict file-based SpiceDB-to-PolicyIR compiler. `raziel-admin` can sign
one policy and publish a complete registry generation. M4 supplies runtime snapshot
and capability services. These parts work, but M5 has no production path from a
visual policy draft to an activated policy and signed snapshot.

The missing path crosses several trust boundaries:

- The browser may draft supported rules but must not receive PolicyIR or compiler
  diagnostics.
- Next.js may coordinate operator actions but must not hold the policy signing key.
- `razield` may consume approved policy records but cannot approve them.
- The registry writer currently accepts snapshot append and revoke operations only.
- Production `razield` reads SpiceDB but does not write schemas or relationships.
- The snapshot builder needs an exact private ZedToken without carrying it through a
  browser request or public log.

M5 needs one selected policy active at a time. It does not need concurrent policy
administration or a general-purpose SpiceDB editor.

## Decision

M5 adds a bounded owner workflow with four separate roles:

1. The browser edits a typed `PolicyDraftV1` and submits write-only relationships.
2. The Next.js owner adapter authenticates the operator, applies request bounds, and
   coordinates reviewed owner commands through direct argument arrays.
3. A separate policy-authority process owns the signing key and signs one exact
   reviewed candidate after a single-use approval command.
4. The registry writer validates the already-signed approval and creates the mutable
   registry metadata. It never signs policy claims.

The owner adapter has no policy key. `razield` keeps a read-only registry mount. The
policy-authority and registry-writer sockets have no public network port.

### Typed policy draft

`PolicyDraftV1` contains only these fields:

```text
format_version = 1
candidate_id
policy_name
policy_version
resource_type
permission = query
rule
```

`rule` is one bounded tree containing:

- direct relation
- subject-set relation
- one-arrow delegation
- `ANY` union
- `ALL` intersection
- named caveat attached to one relation

The visual editor exposes the current string, integer, and Boolean caveat operations.
It selects parameters from the approved provenance catalog. The browser cannot set
compiler limits, provenance labels, arbitrary CEL, raw `.zed`, PolicyIR nodes, or
schema text.

The server rejects exclusion, recursion, wildcards, more than one arrow, unknown
claims, arbitrary functions, floats, maps, lists, IP helpers, regex, unknown fields,
and any tree outside the existing proof ceilings.

### Deterministic rendering

The owner adapter converts a validated draft into deterministic SpiceDB schema text
and the fixed M5 compiler configuration. It uses templates committed in the
repository. User strings can fill validated identifiers and bounded literal values
only. They never enter a command line, path, or unparsed schema fragment.

The adapter writes one candidate directory with mode `0700` and files with mode
`0600`. It invokes reviewed binaries with direct argument arrays. It never invokes a
shell. The compiler process has a 30-second deadline, bounded stdout and stderr,
read-only templates, no authority key, and no outbound network.

### Candidate inspection

`raziel-admin` gains a non-signing `inspect-policy` command. It strictly decodes the
compiler output, validates the current ceilings, calculates canonical `policy_hash`,
and writes one browser-safe projection.

The projection contains only:

```text
candidate_id
compiler_version
record_id
policy_name
policy_version
resource_type
permission
rule_summary
source_schema_digest
policy_hash
expected_approval_epoch
valid_from
valid_until
status
created_at
expires_at
```

It omits canonical PolicyIR, compiler diagnostic JSON, schema text, relationships,
caveat values, ZedTokens, authority signatures, private paths, and local file paths.

Draft blocks created in the current browser session remain visible to that operator.
The portal does not provide an API that fetches an approved policy body or reconstructs
PolicyIR from `policy_hash`.

### Candidate state machine

The owner workflow records the last successful durable state:

```text
DRAFT
  -> COMPILED
  -> APPROVAL_PENDING
  -> APPROVED
  -> REGISTERED
  -> SPICEDB_WRITTEN
  -> SNAPSHOTTED

Any pre-APPROVED state -> REJECTED or DISCARDED
```

Each transition binds `candidate_id`, `source_schema_digest`, and `policy_hash`. A
failed command records a bounded public error code but leaves the candidate at its
last successful state. A retry can repeat only the same transition for the same bound
artifacts. It cannot activate another candidate. `REJECTED`, `DISCARDED`, and
`SNAPSHOTTED` are terminal. Retrying a completed idempotent transition returns the
same public result.

### Explicit approval

The operator reviews the safe projection and sends an approval command containing:

- candidate ID
- policy hash
- expected approval epoch
- validity window
- operator identity
- a random single-use approval nonce

The Next.js owner adapter accepts the command only from an authenticated same-origin
session. It checks the request Origin, a session-bound CSRF token, whole-second time,
body size, and exact candidate state. The approval nonce is consumed atomically and
never released after a downstream failure.

The adapter forwards the exact command through the policy-authority Unix socket. The
authority reloads the compiled candidate from owner storage, recomputes its hash,
loads the current registry read-only, derives the next approval epoch for the policy
name, checks the expected epoch and command, and signs the existing ADR-0010 policy
approval claims. It returns one staged approved record. The staged record has no
caller-selected registration time or registry generation. The authority never
accepts caller-supplied PolicyIR or output paths.

The policy-authority key is mounted read-only into that process only. It is absent
from the browser, Next.js, `razield`, the registry writer, and standard logs.

### Registry publication

ADR-0016 is amended with one new registry-writer operation:

```text
AppendApprovedPolicy {
  expected_registry_generation
  staged_policy_record
}
```

The staged record contains canonical PolicyIR, signed approval claims, and the
authority signature. Its mutable status, registration time, revocation fields, and
registry generation are unset. The writer does not approve or sign policy. It
strictly decodes the record, verifies the authority signature and trust domain,
recalculates `policy_hash`, checks validity, and requires an unused record ID and the
next approval epoch. It sets status to `APPROVED`, supplies registration time from its
trusted clock, assigns the new registry generation, rewrites the complete generation,
and atomically publishes `current.pb`.

The writer keeps one serial operation loop for policy and snapshot records. Existing
snapshot append and revoke behavior remains unchanged. This removes concurrent CLI
and runtime writers during M5. Offline administration commands remain recovery tools
and require the writer to be stopped.

### SpiceDB write and revision handoff

The owner adapter invokes a new bounded `raziel-admin write-spicedb-candidate`
command through a direct argument array. The command uses the existing pinned
`spicedb-rs-proto` generated Authzed client over `owner-internal`. It reads only fixed
owner configuration and the candidate's owner-controlled files. It writes the exact
schema bytes whose `source_schema_digest` is bound into PolicyIR and `policy_hash`.
It then writes the bounded submitted relationships and captures the returned
ZedToken. No new Node SpiceDB client is needed.

Relationship submission is write-only. The response contains accepted count,
resource type, policy record ID, and status. It never returns relationship tuples or
stored caveat values.

The command writes the ZedToken to a candidate-specific `0600` owner secret file. The
adapter passes only a symbolic secret identifier to `BuildSnapshot`. The token never
enters a browser request, Redis, command argument, or standard log before snapshot
publication.

After `BuildSnapshot` succeeds, the portal receives snapshot ID, root, policy hash,
expiry, state, and the exact revision already made public by ADR-0016 through the
signed snapshot descriptor. Existing private snapshot storage and registry-root
publication rules remain unchanged.

### Authentication and deployment

M5 has one operator account. The VM receives its password hash or external session
secret through a Docker secret. The browser receives an `HttpOnly`, `Secure` in live
mode, `SameSite=Strict` session cookie. State-changing routes require a session-bound
CSRF token and exact same-origin checks.

Owner administration routes bind only to the console server. The policy-authority and
registry-writer processes expose private Unix sockets. SpiceDB administration uses an
owner-only credential. No owner administration port binds to the VM host.

Pre-approval candidate files expire after 24 hours. Rejected and discarded candidates
are deleted. Approved candidates remain available for exact-transition retries. After
`SNAPSHOTTED`, the rendered schema, PolicyIR candidate copy, submitted relationships,
and ZedToken handoff file are deleted. The trusted registry and private snapshot keep
their existing durable records. Public receipts retain identifiers, hashes, state,
operator identity, and whole-second timestamps only.

## Trust-Boundary Effect

The browser gains a bounded policy-draft surface and safe hash projection. It does not
gain an approved-policy read API, PolicyIR, schema text, compiler diagnostics, graph
reads, stored caveat values, or authority material.

The registry writer gains authority to append a validated already-signed policy
approval and set mutable registry metadata from trusted local state. It still cannot
create or change signed approval claims. This is a deliberate amendment to ADR-0016
and requires focused security tests.

The separate policy-authority process gains the policy signing key. It signs only an
exact stored candidate after a single-use approval command.

## Public And Private Data Effect

Public operator data:

- candidate ID and draft digest
- rule summary and compiler version
- policy record ID, source schema digest, and `policy_hash`
- lifecycle status and timestamps
- accepted relationship count
- snapshot ID, root, public SpiceDB revision, and expiry

Owner-private data:

- rendered schema and compiler configuration
- compiler PolicyIR and diagnostic output
- policy signing key
- submitted relationships and stored caveat values
- pre-publication ZedToken handoff file and symbolic secret reference
- snapshot salts and private snapshot records

ADR-0016 already publishes the exact ZedToken value as
`RelationshipSnapshotDescriptor.spicedb_revision`. ADR-0018 adds no proof public
value, capability field, hash format, or guest input.

## Failure Rules

- Compiler timeout or invalid output leaves the draft unapproved and records a bounded
  compile failure.
- Approval hash or expected-epoch mismatch consumes no signature and leaves the
  candidate at `APPROVAL_PENDING`.
- Registry generation conflict leaves the signed record staged and returns a generic
  retryable state.
- SpiceDB schema or relationship failure leaves the candidate at `REGISTERED` and
  never calls `BuildSnapshot`.
- ZedToken publication failure never sends a snapshot request.
- Snapshot failure leaves the candidate at `SPICEDB_WRITTEN` and prevents capability
  issuance until the same snapshot transition succeeds.
- No failure rolls back or reuses an approval nonce.

## Required Tests

- accepted direct, team, delegated, union, intersection, and caveated drafts
- rejected exclusion, recursion, wildcard, two-arrow, arbitrary CEL, unknown field,
  limit, identifier, and literal cases
- stable schema, configuration, source digest, policy hash, and safe projection vectors
- projection scans proving PolicyIR, schema, diagnostics, relationships, caveat values,
  tokens, keys, and paths are absent
- command invocation tests proving no shell and no caller-selected path
- operator authentication, Origin, CSRF, body-limit, expiry, and approval-nonce tests
- policy-authority hash mismatch, duplicate approval, malformed frame, timeout, and
  redaction tests
- registry-writer append, stale generation, duplicate record, invalid signature,
  wrong trust domain, invalid policy hash, and concurrent snapshot tests
- `write-spicedb-candidate` argument, fixed-path, schema-digest, bounded relationship,
  partial failure, and private ZedToken handoff tests
- full compile, approve, register, write, snapshot, and capability-request flow

## Rejected Options

### Put the policy key in Next.js

This merges portal compromise with policy-authority compromise and lets the UI server
approve any candidate it can construct.

### Let `razield` sign or register policies

This breaks the core rule that the prover cannot approve its own PolicyIR.

### Return PolicyIR or compiler diagnostics for browser review

This creates a client-visible policy artifact and can expose internal caveat and
evaluation structure. The safe projection is enough for M5 operator review.

### Keep policy publication as an untracked manual gap

This prevents a connected portal workflow and makes the active policy ambiguous.

### Let the registry writer sign policy records

The writer would become both policy authority and registry owner. The selected design
keeps signature creation in a separate process.

### Add a free-form SpiceDB editor

It would exceed the supported compiler subset and turn input validation into a second
language parser.

## Approval Record

Approved by the maintainer on 2026-08-27 local time.
