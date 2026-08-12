# Raziel Repository Instructions

This file is the canonical operating contract for coding agents in this repository.
Codex is the primary assistant. Cursor and Claude use thin wrappers that point here.
Do not create tool-specific copies of these rules.

## Mission

Build Raziel as a proof-carrying authorization layer for AI-agent data access.
The first proof of concept covers payment reconciliation against real transaction data.

Raziel must prove this statement:

> An approved policy and committed relationship snapshot evaluate to `ALLOW` for this
> exact agent-bound capability scope.

The verifier checks cryptographic evidence. It does not trust a Boolean response from
the prover.

## First project goal

Run the payment-reconciliation workflow end to end against non-mocked transaction
data. The acceptance run must record source provenance, usage basis, checksum, schema
mapping, and row count. Small synthetic fixtures are allowed in automated tests and
the clearly labeled local M5 development walkthrough. Synthetic data never satisfies
the M6 acceptance gate.

Do not start another scenario until the reconciliation acceptance gate passes.

## Required reading order

Read these files before planning or editing:

1. `AGENTS.md`
2. `docs/agent/CURRENT-BATON.md`
3. `ARCHITECTURE.md`
4. `docs/spec/POC-TECHNICAL-SPEC.md`
5. The active task files and tests

For scenario work, read `docs/spec/SCENARIO-AUTHORING.md` and use the
`raziel-scenario` repo skill.

Some assistants do not auto-discover `.agents/skills/`. In that case, read
`.agents/skills/raziel-scenario/SKILL.md` directly and follow it as a checklist. The
skill contains plain Markdown and does not depend on Codex-only commands.

Use this source-of-truth order:

1. Architecture invariants and approved decision records
2. Protobuf wire contracts
3. Rust canonicalization and PolicyIR semantics
4. Executable tests and vectors
5. Scenario configuration and adapters
6. Product UI copy

Stop and record a conflict in the current baton. Do not silently change proof
semantics to make one test pass.

## Baton requirement

Every implementation session uses `docs/agent/CURRENT-BATON.md`.

At session start:

1. Read the current baton and `docs/agent/BATON-PROTOCOL.md`.
2. Check `git status --short` and preserve changes from other authors.
3. Claim one bounded objective in the baton.
4. Record the files in scope and planned validation.
5. Confirm that the objective respects architecture invariants.

During work:

- Keep only one primary objective in progress.
- Update the baton after a decision, blocker, scope change, or completed gate.
- Record commands with outcomes, not raw terminal transcripts.
- Do not erase another agent's unresolved notes.
- Create a decision record under `docs/decisions/` for a durable architecture change.

Before handoff:

1. Run the narrow tests for edited code.
2. Run all required cross-component tests affected by the change.
3. Record exact pass, fail, and skipped results.
4. Update the file map and remaining risks.
5. Set one exact next action.
6. Archive the completed baton entry under `docs/agent/history/`.

The full state machine and templates live in `docs/agent/BATON-PROTOCOL.md`.

## Architecture invariants

These rules cannot change inside a feature task:

1. `verifierd` never calls `razield`.
2. The agent never receives PolicyIR, relationship tuples, satisfying paths,
   Merkle openings, or stored caveat values.
3. The capability carries `policy_hash`, not PolicyIR.
4. The verifier resolves `policy_hash` through its trusted local policy registry.
5. `razield` cannot mark its own PolicyIR as approved.
6. The gateway never accepts an unproved `ALLOW` from `razield`.
7. The compiler rejects unsupported SpiceDB and CEL constructs.
8. Raw Protobuf bytes are never commitment inputs. Use the specified canonical form.
9. Every proof binds an approved policy record and signed snapshot descriptor.
10. Every capability binds an agent key, task, purpose, action, resource scope,
   issuance nonce, and short validity window.
11. Every protected request checks capability scope and agent request signature.
12. Acceptance demos use real transaction data with recorded provenance.
13. No hosted proving service receives private witness data.
14. Scenario adapters cannot change guest semantics.
15. The baseline guest proves `ALLOW` only.
16. Policy authorization and dataset relevance remain separate systems.

Any proposed change to this list needs a decision record and explicit maintainer
approval.

## Trust boundaries

Public proof-system material:

- PolicyIR interpreter semantics
- supported SpiceDB and caveat subset
- canonical encoding specification
- capability format
- proof public-value schema
- verification key identifier

Verifier-visible material:

- exact approved PolicyIR
- policy version and status
- `policy_hash`
- signed snapshot descriptor
- proof and capability public claims

Agent-visible material:

- capability identifier and public scope
- `policy_hash`
- relationship snapshot root
- expiry and anti-replay fields
- proof package or verifier receipt

Raziel-private witness material:

- relationship tuples
- satisfying path and branch choices
- Merkle membership openings
- stored caveat context
- sensitive caveat parameters
- witness-construction traces

Stateful service material:

- snapshot registry and revocation state
- policy registry and status
- capability replay and revocation state
- verification cache
- audit receipts

Never log private witness material.

## Repository ownership by language

TypeScript owns:

- Next.js App Router demo console
- Next.js gateway and protected payment routes
- agent SDK
- generated Connect clients
- scenario presentation and API orchestration

Rust owns:

- canonical commitment encoding
- PolicyIR types and interpreter
- relationship snapshots and Merkle verification
- witness validation
- SP1 guest and host
- prover service
- independent verifier service

Go owns:

- the build-time SpiceDB schema compiler adapter
- conversion from maintained SpiceDB compiler protobuf output into neutral input for
  the Rust PolicyIR compiler

Protobuf and Buf own cross-language wire contracts. Do not duplicate cryptographic
rules in TypeScript or Go.

## Planned repository shape

```text
apps/
  demo-console/
  demo-gateway/
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
proto/raziel/v1/
examples/reconciliation/
tests/
docs/
```

Do not create a new top-level package without updating `ARCHITECTURE.md` and the
current baton.

## Dependency policy

Do not install a package, CLI, runtime, or system dependency without user approval.

For each missing dependency, state:

- its name
- why this task needs it
- the exact install command
- what the install unlocks
- the fallback without it

Prefer workspace-pinned versions and lockfiles. Linux containers must support Apple
Silicon through native `linux/arm64` images or documented multi-architecture images.
Do not force `linux/amd64` without recording the performance and compatibility cost.

## Planned commands

These commands describe the target developer interface. Check that each script exists
before running it. Do not invent a missing script inside an unrelated task.

```bash
pnpm bootstrap:check
pnpm generate
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
cargo test --workspace
go test ./...
docker compose up
```

M0 defines and implements the final command surface.

## Coding rules

- Keep changes inside the active baton's file scope.
- Prefer existing local patterns over a new abstraction.
- Keep commitment inputs versioned and domain-separated.
- Use bounded collections and explicit maximum depths at proof boundaries.
- Reject unknown enum values, fields, PolicyIR nodes, and capability versions.
- Use typed errors across service boundaries.
- Return generic authorization failures to agents.
- Keep detailed failure codes in access-controlled audit records.
- Use UTC for protocol timestamps.
- Keep deterministic test vectors for every canonical hash.
- Keep generated code out of manual patches.
- Put domain mappings in scenario adapters, not the proof guest.
- Do not expose exact policy explanations through preflight or denial APIs.
- Do not add a client-side policy evaluator containing private policy or graph data.

## Security review triggers

Stop normal feature work and record a security review item for changes to:

- public proof values
- hash or canonical encoding
- snapshot leaf construction
- PolicyIR semantics
- caveat semantics
- key binding or request signatures
- capability reuse or revocation
- verifier registry trust
- logs containing authorization context
- new discovery or preflight oracles
- any client-visible policy artifact

## Test expectations

A narrow change needs focused unit tests. Shared semantics need broader tests.

Cryptographic changes require:

- deterministic positive and negative vectors
- mutated-field tests
- cross-language encoding checks
- proof public-value checks
- malformed and oversized input tests

Compiler changes require:

- accepted-subset fixtures
- rejected-construct fixtures
- SpiceDB-versus-PolicyIR equivalence tests
- stable PolicyIR output checks

Service changes require:

- typed failure tests
- timeout and retry behavior
- agent key and anti-replay tests
- verifier independence with `razield` stopped

Scenario changes require:

- source manifest validation
- deterministic import
- schema mapping tests
- policy and resource mapping tests
- one allowed and one denied journey
- audit receipt checks

Never claim the acceptance goal from fixture-only tests.

## Git and file safety

- Treat unknown changes as user or agent work. Do not revert them.
- Do not use destructive Git commands.
- Do not rewrite unrelated files.
- Do not commit generated credentials, private datasets, raw witness files, proofs with
  sensitive debug traces, or local environment files.
- Run `git diff --check` before handoff.
- Keep commits scoped to one baton objective.

## Definition of done

A task is done only after:

- its acceptance criteria pass
- relevant tests pass
- docs match behavior
- architecture invariants still hold
- no secret or private witness enters logs or client responses
- the current baton records evidence and the next action

The PoC is done only after every gate in `docs/spec/POC-TECHNICAL-SPEC.md` passes,
including the real-data reconciliation run.

## Adding another scenario

Start scenario work only after payment reconciliation passes its acceptance gate.

A normal scenario adds:

- a source and provenance manifest
- canonical domain records
- SpiceDB resources, relations, and caveats within the supported subset
- action and resource mappings
- a data adapter
- Next.js console and gateway routes
- allowed, denied, replay, expiry, and audit tests

A normal scenario does not add a new guest, PolicyIR node, hash format, or capability
format. A request for any of those changes is proof-system work, not scenario work.

Effort bands:

- Small: existing source format and existing PolicyIR nodes. About 2 to 4 engineering
  days after data access is ready.
- Medium: new adapter and supported caveat combinations. About 1 to 2 engineering
  weeks, including tests and review.
- Research: new proof semantics, non-membership, recursion, or a private-policy model.
  Write a decision record and separate technical plan before estimating delivery.

Follow `docs/spec/SCENARIO-AUTHORING.md` for the complete gate.
