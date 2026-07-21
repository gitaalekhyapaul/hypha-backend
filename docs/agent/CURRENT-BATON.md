# Current Engineering Baton

- Baton ID: B-0008
- State: READY
- Milestone: M2, Canonical commitments and snapshots
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-21T18:59:04Z
- Branch: `main`
- Worktree: B-0007 is complete and archived as
  `docs/agent/history/B-0007-canonical-policy-encoding-and-hash.md`. The worktree is
  expected to be clean after its scoped commit.

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

- `docs/decisions/` for the relationship commitment decision
- `crates/canonical/` for shared canonical relationship encoding only
- `crates/snapshot/` for blinded leaves, tree construction, roots, and openings
- `tests/vectors/` for deterministic public and private-test vectors
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
- Deferred: O-004 snapshot signing algorithm and key storage remains open. It does not
  block the tree-only objective.

## Open Questions and Blockers

- The relationship leaf and tree protocol is not yet approved. B-0008 must draft the
  decision and obtain explicit maintainer approval before proof-critical code changes.
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

## Validation Plan

- Deterministic relationship, leaf, tree, root, and opening vectors
- Positive, malformed, oversized, ordering, and one-field mutation tests
- Rust focused tests and affected workspace tests
- Format, lint, typecheck, build, generation, `git diff --check`, and private-data scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Not run | NOT AVAILABLE | B-0008 is ready but unclaimed | 2026-07-21T18:59:04Z |

## Risks and Private-Data Notes

- Salt reuse, ambiguous tuple encoding, unstable ordering, or unspecified odd-node
  handling can create privacy loss or incompatible roots.
- Test fixtures must remain synthetic. Private salts and openings may exist only in
  test-scoped vectors that are explicitly marked non-production.
- The public root must not expose tuple count through an undocumented tree-shape rule.

## Exact Next Action

Claim B-0008. Inspect the current relationship model, snapshot Protobuf, and M2 tree
requirements. Draft the relationship leaf and tree decision before implementation.
