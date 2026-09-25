# ADR-0020: Agent API And Request Provisioning

- Status: proposed, awaiting maintainer approval
- Date: 2026-09-25
- Milestone: M6
- Related baton: B-0063
- Builds on: ADR-0019
- Security review: request signatures, caller isolation, registry identity, and schema provisioning
- Changes no guest semantics, existing commitment, proof value, or capability signature

## Need

ADR-0019 accepts external agents, individual tokens, an approval inbox, independent
policy publication, and serial proofs. The current M5 implementation cannot supply
that workflow by exposing its existing routes:

- Internal dataset routes rely on the authenticated operator facade. They are not an
  external agent API. A token must not grant unrestricted access to those routes.
- The SDK signs capabilities and protected queries, but not general agent API calls.
- Capability admission and the worker each require 75 minutes of remaining requested
  validity. Two signed requests cannot wait through arbitrary proof queues.
- `LocalRegistry` accepts only the highest approval epoch for one `policy_name`.
  Publishing another version under that name makes the older record unresolvable.
- M5 writes a candidate's entire SpiceDB schema and cleans up its temporary package.
  That conflicts with later requests using several durable policy versions.

The concrete proposed wire and lifecycle rules are in
[Agent API v1](../spec/AGENT-API-V1.md). They are not implemented by this ADR.

## Proposed Decisions

### Agent Authentication And Public Facade

Expose only explicit agent routes through `apps/demo-console`. Operator routes keep
their existing session and CSRF checks. An agent token never authenticates an operator
route. Keep `demo-gateway`, SpiceDB, Redis, and owner RPCs off the agent network.

Each token binds one registered Ed25519 key. A new signature domain binds the API
audience, token ID, key thumbprint, method, canonical target, body digest, nonce, and
timestamp. It reuses ADR-0002 framing and target rules, not its signature domain.
ADR-0014 key derivation and strict verification remain authoritative.

The server stores a one-way token verifier, not the bearer secret. Authentication
checks current token status on every request, including cached protected queries.
Revoking a token blocks later API authentication. It does not rewrite a capability
or retroactively undo an in-flight authorized response.

Use a bounded local Rust authentication helper for strict key and signature checks.
The TypeScript facade handles token lookup, constant-time secret comparison, and the
atomic nonce store. The helper has no private witness, authority key, proof runtime,
network access, or `razield` dependency. Add its canonical encoding in Rust and match
it through SDK vectors. Do not implement a second server-side crypto policy in JS.

The protected-query facade preserves the existing gateway path and exact signed
query envelope. Both API authentication and the existing capability signature must
name the registered key. A full query still passes the PEP scope, nonce, proof/cache,
and receipt checks. Authentication alone cannot release a row.

### Waiting Requests And Serial Dispatch

Store access intents separately from cryptographic proof jobs. The operator approves
one exact agent, task, purpose, finalized dataset revision, policy version, and grant
recipe. The recipe stays owner-private. Approval does not itself issue a capability.

Use the existing Rust BullMQ dependency for a single access dispatcher. It prepares
one approved request only after proof capacity is available. It provisions a grant,
builds an exact-revision snapshot, and asks the agent for its final issuance signature.
The agent signs its own key, nonce, context, snapshot, and exact capability scope.

An offline agent has a two-minute signature reservation, not an indefinite claim on
the prover. Reservation expiry returns the request to a resumable waiting state.
Other ready agents can proceed. Each signed attempt is persisted before submission.
An uncertain response triggers lookup by the existing commitment-derived job ID.
It never triggers a new issuance nonce or another proof automatically.

The worker still proves at most 75 minutes of capability validity under the current
one-hour execution deadline. A five-minute proposal-to-worker-start allowance sits
in the signed requested expiry. It does not extend the resulting capability window.
Waiting for review or capacity consumes neither allowance.

### Independent Policy Versions

Give the UI a stable policy family and separately identified immutable versions.
For new M6 packages, the signed name is `family:<familyId>:v<version>`, not the display
name alone. The numeric policy version remains visible. Approval epochs still advance
within that signed name. Reapproving v1 can invalidate its earlier approval, but
publishing v2 does not invalidate v1. Existing registry signature and resolution code
keeps its latest-epoch-only rule.

Generate collision-free definition and caveat names from the immutable version ID.
Keep `definition agent {}` shared and empty. The guest already derives subjects in
that namespace. Other definitions belong to one version. Compile each self-contained
source package with the maintained SpiceDB compiler and existing PolicyIR limits.
Do not combine all installed versions into one guest policy.

Policy publication approves and durably stores that package. It creates no snapshot.
At request provisioning, the owner writer installs a deterministic union of approved
version packages, writes the authorized relationships, and records the exact revision.
Only this short provisioning operation is serialized. Policy publication does not
wait for proof generation.

The package's `source_schema_digest` still commits its exact source bytes. It does not
commit the whole multi-package SpiceDB schema. The owner writer checks the complete
composition against its durable manifest and checks every compiled package's names
and references. Unexpected live schema drift fails closed. This explicitly replaces
ADR-0018's single-candidate full-schema assumption for the M6 provisioning path.
This is the networked SpiceDB provisioner, not the networkless registry writer.
Retain installed definitions after revocation or expiry so existing tuples remain
well-formed. Snapshot publication still requires current approval.

The gateway resolves an exact trusted mapping from policy hash, resource type, and
resource scope to a finalized dataset revision. A type prefix or an agent-provided
alias is not authority. Agents see that opaque resource type as part of their scope,
not the source schema, PolicyIR, relationships, or private caveat values.

## Storage And Migration

- Persist agent registrations and token verifiers in the existing owner Redis.
  Agent containers have no Redis access. Gateway query authentication remains usable
  with issuer and dispatcher processes stopped.
- Persist public request state and dispatcher references in Redis. Private request
  context, grant inputs, and signed issuance envelopes use owner-only file storage.
  Queue payloads contain identifiers, never witness or context material.
  A minimal private intent comparison record lasts seven days for exact-retry
  checks. Proof and issuance envelopes are removed after terminal cleanup.
- Persist approved source packages and composition manifests in owner storage.
  Expired temporary candidates can be removed. Referenced approved packages cannot.
- Retain existing M5 artifacts, names, proofs, and verification keys unchanged.
  Preserve a validated legacy schema package during composition. Missing historical
  source bytes cannot be reconstructed from PolicyIR with the old source digest.
  Block legacy reprovisioning without those bytes, but retain old proof verification.
  Do not rewrite old records into the new version naming scheme.
- Give M6 one schema/relationship writer. Disable the legacy M5 publisher in that
  deployment mode. A second full-schema writer would defeat the composition checks.
- Regenerate public agent API DTOs from new Protobuf files. Keep existing strict
  verifier descriptors and guest dependencies unchanged.

## Review Gates

| Boundary | Required evidence before its feature ships |
| --- | --- |
| API signatures | Matching Rust/TS bytes, field mutations, malformed and weak keys, time boundaries, nonce concurrency |
| Caller isolation | Token A plus key B fails; token rotation preserves ownership; revoked token fails; other callers' records stay hidden |
| Query facade | Raw internal data routes unreachable; both signing keys match; wrong path/body rejected; issuer-offline query works |
| Dispatch | Two approvals run serially; offline signer releases capacity; uncertain submission reconciles; no automatic second proof |
| Policy versions | v1 and v2 coexist; reapproval affects only its version; source/schema mismatch and cross-package references fail |
| Private data | No source package, graph, caveat context, token, or private envelope in agent projections or logs |

The design review compares the changes with ADR-0002, ADR-0010, ADR-0014, and ADR-0018.
These planned tests do not count as passing runtime evidence.

The B-0063 read-only policy review identified six integration requirements: explicit
reapproval semantics, a cumulative trusted composer, request-attempt handoffs,
immutable dataset bindings, durable package retention, and aggregate resource bounds.
The API specification records each requirement. Runtime validation remains pending.
The API review added owner-qualified request IDs, durable cancellation of uncertain
submissions, private comparison-record retention, and fresh inner and outer signatures
for query retries. These are specification corrections, not claims of runtime fixes.

## Alternatives Not Chosen

- Bearer tokens alone do not prove possession of the registered agent key.
- Reusing an issuance signature for polling does not bind the HTTP route or body.
- Exposing the internal gateway grants access beyond the protected-query route.
- Signing both agents' final requests immediately consumes validity during queueing.
- Treating every publication as reapproval invalidates unrelated in-flight versions.
- One mutable schema per request risks replacing definitions needed by another policy.
- One SpiceDB container per version adds deployment lifecycle work to a small demo.
- Editing the SP1 guest or weakening registry epoch checks changes the proven protocol.

## Approval Requested

Approve these two security decisions as one M6 protocol amendment:

1. The separately domain-bound agent API authentication contract, including its
   token/key binding, freshness, replay, and explicit public-route boundary.
2. Coexisting immutable policy versions through version-specific signed names and
   composed namespaced schemas, with per-package source digests and trusted dataset
   bindings. Reapproval and explicit revocation retain their current semantics.

The two-minute reservation and bounded queue limits are documented operational
defaults. They do not alter proof semantics. No new third-party dependency is selected.
Public npm publication and VM deployment remain separate execution steps.
