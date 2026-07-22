# Current Engineering Baton

- Baton ID: B-0008
- State: ACTIVE
- Milestone: M2, Canonical commitments and snapshots
- Owner: Codex
- Started UTC: 2026-07-21T19:34:48Z
- Last updated UTC: 2026-07-21T19:55:23Z
- Branch: `main`
- Worktree: the B-0008 protocol proposal and active baton are the only changes from
  `c846ab1`. ADR-0009 remains proposed until explicit maintainer approval.

## Objective

Define and implement deterministic blinded relationship-tree construction from fixed
relationship input and a caller-supplied private snapshot salt.

## Acceptance Criteria

- [ ] One accepted decision record fixes relationship record encoding, leaf and node
      domains, salt handling, leaf ordering, odd-node behavior, and empty-tree policy.
- [ ] Rust builds a deterministic relationship root without logging or returning the
      private salt through a public interface.
- [ ] Rust produces bounded membership openings that verify against the root and
      rejects malformed, duplicate, unsorted, and oversized inputs.
- [ ] Positive and one-field mutation vectors cover relationship records, leaf hashes,
      internal nodes, roots, and openings.
- [ ] Focused and affected workspace gates pass.

## Architecture Constraints

- Raw Protobuf bytes are never commitment inputs.
- Relationship tuples, stored caveat values, salts, openings, and satisfying paths are
  Raziel-private witness material.
- The public snapshot root binds one exact relationship set at one SpiceDB revision.
- Leaf ordering uses the canonical unblinded tuple key. The private salt does not alter
  ordering.
- Commitment inputs remain versioned, length-framed, and domain-separated.
- The agent never receives relationship tuples, branch choices, salts, or openings.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `ARCHITECTURE.md` for the accepted root and witness model
- `docs/spec/POC-TECHNICAL-SPEC.md` for the accepted tree and opening rules
- `docs/decisions/` for the relationship commitment decision
- `crates/canonical/` for shared canonical relationship encoding only
- `crates/snapshot/` for blinded leaves, tree construction, roots, and openings
- `tests/vectors/` for deterministic public and private-test vectors
- `packages/proto-ts/test/` for strict parsing of shared vectors without hashing
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Snapshot descriptor signatures and authority key storage
- Policy and accepted-root registries
- SpiceDB relationship loading and ZedToken retrieval
- Witness path search and PolicyIR evaluation
- Services, gateway, SDK, console, SP1 execution, and proof generation
- Real transaction data

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0008.
- Inherited: ADR-0008 makes Rust the only commitment authority and fixes shared hash
  primitives. B-0008 must use separate relationship and tree domains.
- Proposed ADR-0009: policy-bound canonical relationship records, structural identity
  order, index-bound salted leaves, a left-balanced unpadded tree, private salt and
  count bound into the public root, and direction-free openings.
- Deferred: O-004 snapshot signing algorithm and key storage remains open. It does not
  block the tree-only objective.

## Open Questions and Blockers

- The relationship leaf and tree protocol is not yet approved. B-0008 must draft the
  decision and obtain explicit maintainer approval before proof-critical code changes.
- The current witness shape carries `leaf_index` but no count committed by the root.
  Duplicate-last trees then permit a duplicated odd leaf to validate under a phantom
  adjacent index. The draft decision must bind a private `leaf_count` into the public
  root and require it during opening verification, or select another rule with the
  same anti-alias property.
- The normal development `spicedb` service has a pre-existing command and key
  incompatibility. It does not block deterministic tree work.
- Deferred follow-up: carry this item through each baton reset. After the M7 exit gate
  passes, remind the maintainer to revisit the separate Hypha relevance-attestation
  scope and the Obsidian `Current Implementation Discussion Post`.

## Work Completed

- B-0007 implemented canonical PolicyIR encoding, `policy_hash`, strict Protobuf
  ingress, shared vectors, and the test-only TypeScript parser.
- B-0007 passed focused, workspace, live SpiceDB equivalence, build, generation,
  privacy, and final diff checks.
- Reconciled the ready baton against clean `main` at `c846ab1` and claimed B-0008 with
  protocol drafting as the only active objective.
- Inspected the architecture, technical specification, snapshot Protobuf, PolicyIR
  relationship model, equivalence fixture, canonical crate, and empty snapshot crate.
- Identified a leaf-index alias risk in the incomplete tree specification and expanded
  the documentation scope needed to resolve it durably.
- Drafted `docs/decisions/0009-blinded-relationship-tree.md` without changing accepted
  behavior or implementation.
- Completed two independent read-only audits. The revised draft replaces
  duplicate-last padding with a left-balanced tree, binds index and the common salt,
  removes redundant direction bits, uses structural identity order, defines exact
  digest widths, and adds explicit snapshot limits plus derived guest accounting.
- Prepared a documentation-only checkpoint. It accepts no protocol and changes no
  implementation, dependency, public root, wire contract, or approved specification.

## Validation Plan

- Deterministic relationship, leaf, tree, root, and opening vectors
- Positive, malformed, oversized, ordering, and one-field mutation tests
- Rust focused tests and affected workspace tests
- Format, lint, typecheck, build, generation, `git diff --check`, and private-data scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Baton start reconciliation | PASS | Clean `main` at `c846ab1`; B-0008 scope matches M2 | 2026-07-21T19:34:48Z |
| Architecture and model inspection | PASS | Snapshot, canonical, PolicyIR, fixture, Protobuf, and M2 requirements reconciled | 2026-07-21T19:39:59Z |
| Two independent protocol audits | PASS after findings incorporated | Tree aliasing, salt binding, ordering, limits, witness shape, and digest width | 2026-07-21T19:53:42Z |
| `git diff --check` | PASS | Proposed ADR-0009 and active baton | 2026-07-21T19:54:07Z |

## Risks and Private-Data Notes

- Salt reuse, ambiguous tuple encoding, unstable ordering, or unspecified odd-node
  handling can create privacy loss or incompatible roots.
- Test fixtures must remain synthetic. Private salts and openings may exist only in
  test-scoped vectors that are explicitly marked non-production.
- The public root must not expose tuple count through an undocumented tree-shape rule.

## Exact Next Action

Obtain explicit maintainer approval for proposed ADR-0009. After approval, mark it
accepted, align architecture and specification, then implement the Rust and vector
work without adding an external dependency.
