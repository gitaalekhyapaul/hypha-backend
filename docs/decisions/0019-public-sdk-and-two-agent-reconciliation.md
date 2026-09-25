# ADR-0019: Public SDK And Two-Agent Reconciliation

- Status: accepted scope and architecture direction; implementation pending
- Date: 2026-09-25
- Milestones: revised M6 and M7
- Related baton: B-0062
- Supersedes: the real-data prerequisite for the current demo, the single-repository
  product boundary, and ADR-0018's single selected policy workflow for new work
- Preserves: all existing cryptographic commitments, guest semantics, and verifier
  independence

## Context

M0-M5 demonstrate policy authoring, snapshots, real local Plonk issuance, independent
verification, protected queries, and cached capability reuse. The operator portal
currently performs the agent role too. The existing SDK is a private workspace
package with signing helpers. It is not yet a published external integration client.

The maintainer wants a practical demonstration: two separate agents reconcile
processor and bank ledgers through Raziel. An operator reviews their access requests
in a third dashboard. All three applications must support a desktop demonstration.

## Decision

### Scope And Repositories

Use sibling repositories with independent histories and releases:

| Repository | Ownership |
| --- | --- |
| `hypha-backend` | Protocol, owner services, operator console, protected data API |
| `raziel-sdk-ts` | Public `@gitaalekhyapaul/raziel-sdk-ts` package |
| `raziel-demo-agents` | Agent A, Agent B, their dashboards and message channel |

The agents install a packaged SDK version. Backend source imports and Git submodules
are not part of the distribution contract. Development can use a versioned package
tarball. The release gate uses the public package. Backend Protobuf and canonical
vectors remain authoritative. The SDK package must be installable without private
workspace dependencies.

M6 now delivers the external agent API, request approval lifecycle, and public SDK.
M7 delivers the two-ledger scenario and three-dashboard demonstration. The former
real-data and second-scenario milestones become deferred work without new dates.
Synthetic seed data and portal-created rows satisfy the revised demo gate. They do
not establish real-data validation. This explicitly amends AGENTS invariant 12.

### Agent Identity And API Tokens

The SDK accepts an externally supplied identity and signer. It does not provision an
identity provider. Each demo agent persists its own Ed25519 key in private runtime
storage across process and container restarts.

The operator registers a name and public key through Create Agent Token. Raziel
issues a separate revocable API token for that registered agent. Request signatures
must validate against the same registered key. Tokens are shown once, kept out of
logs, and stored as one-way verifiers on the server. Private keys and tokens remain
in agent server processes, outside browser and LLM inputs.

Agent authentication authorizes access to the agent API and the caller's own records.
Protected data still requires a scoped proof capability and fresh request signature.
The new agent API signature contract needs an explicit versioned design. It must not
silently reuse the capability commitment as a general HTTP authentication envelope.

### Policies And Request-Triggered Snapshots

An operator can author and publish a policy without an agent request. Publishing
creates an immutable approved version and does not build a relationship snapshot.
New versions can be published during pending requests or active proofs.

For a pending access request, the operator selects an approved version or creates and
approves one. The approval binds that request to the selected policy and scope. The
owner provisions the registered agent's relationship and builds a snapshot only for
this issuance workflow. The agent then signs the exact existing capability request,
including its snapshot ID, scope, nonce, and requested expiry.

An in-flight proof reads its immutable private snapshot and pinned policy. Publication
of another version must not replace them or imply revocation. Explicit revocation
remains a separate action, subject to the existing verifier/cache freshness rules.
Selecting a different policy or scope requires renewed request approval and signature.

The current M5 schema write and snapshot workflow is single-policy. Implementation
must preserve compatible schema state or isolate provisioning contexts before
supporting overlapping policy versions. A short serialized provisioning step is
acceptable. Holding policy publication until a proof finishes is not acceptable.

### Sequential Proof Dispatch

Keep one real local Plonk proof executor. Multiple access requests may wait for
operator review or proof capacity. Their waiting state is separate from an admitted
cryptographic proof job. Use the existing BullMQ/Redis stack for durable dispatch.

Prepare a snapshot and obtain the final agent signature near dispatch. Recheck policy,
snapshot, request, and key validity at that point. Waiting must not consume the full
proof validity allowance. Persist each signed issuance attempt and reconcile uncertain
submissions by its deterministic job ID. Never create another proof merely after a
client timeout. An offline agent must not hold the executor indefinitely.

The operator dashboard distinguishes pending review, approved and waiting, signing,
proving, ready, rejected, canceled, expired, and failed. Exact wire states and limits
belong in the next contract task. No mocked proof may authorize data access.

### LLM And Message Channel

Agent execution uses an OpenAI-compatible API with configurable base URL, model, and
server-side API secret. The maintainer selected compatibility as an assumption. Live
endpoint compatibility remains a deployment check. Model choice does not block work.

Use a small bounded tool loop controlled from each dashboard. The model can request
SDK tools and interpret authorized results. Application code validates arguments and
handles credentials, signing, retries, and exact amount arithmetic. An LLM response
cannot grant access or mark a proof verified.

Agent A sends Agent B a task-scoped settlement lookup message. Agent B returns a
bounded result with status, transaction reference, settlement reference, currency,
settlement amount, amount difference, and a gateway receipt reference. Missing or
ambiguous matches remain explicit. The channel displays these messages in both dashboards.

These fields are an allowed synthetic demo output. Full bank rows, authorization
witnesses, keys, tokens, and unrestricted file attachments are excluded. Evidence
references aid inspection. They do not prove reconciliation correctness or confer
the sender's capability on another agent. Each agent queries its own dataset through
its own SDK instance and capability.

## Trust And Data Effects

- New public API records contain agent identity, requested scope, lifecycle status,
  public policy/snapshot bindings, capability claims, and safe receipt references.
- The owner retains PolicyIR, relationships, caveat values, salts, and openings.
- Each agent keeps its own signing key. Raziel keeps only the registered public key.
- Message recipients see only the selected business evidence for their task.
- The SDK is the supported agent integration interface. Server authentication and
  authorization enforce access even for clients that do not use the SDK.
- The new demo extends payment reconciliation. It is not a second domain scenario.

## Migration And Validation

Retain M0-M5 evidence unchanged. Mark it with its original milestone meanings.
Archive unstarted B-0061 as deferred. Update current roadmap and scenario guidance.
Move SDK ownership once, replacing backend consumers with a pinned package rather
than retaining two independently maintained SDK implementations.

Validate each feature with focused integration checks. The final demonstration must
show both real serial proofs, independent queries, cached reuse, key/scope rejection,
operator review, messages, and reconciliation output across the three dashboards.
Fresh deployment, restart persistence, package consumption, and uncertain submission
reconciliation are part of the user workflow. Do not add unrelated test infrastructure.

## Rejected Options

- Keep the operator's process as both agents: this would not show external SDK use.
- A shared installation token: the maintainer requested individual registered keys
  and tokens with separate ownership.
- Proof-long policy locks: these prevent independent policy administration.
- Approve an unbound capability and attach the agent later: this breaks the existing
  signed request and proof statement.
- Parallel proofs on the current VM: retain the measured serial resource profile.
- Make real-data acquisition the next gate: the maintainer deferred that work.

## Decision Record

The maintainer accepted the scenario, dashboards, synthetic data, simple messaging,
independent policy publication, public package name, signer interface, per-agent
tokens, and sequential real proofs in this conversation. On 2026-09-25 the maintainer
directed use of an OpenAI-compatible API assumption and delegated the evidence format
choice to the implementation. This ADR records that direction without claiming that
new API contracts, code, package publication, or deployment already exist.

The concrete feature order and remaining engineering decisions are in the
[M6-M7 plan](../spec/M6-M7-AGENT-DEMO-PLAN.md).
