# Current Engineering Baton

- Baton ID: B-0007
- State: READY
- Milestone: M2, Canonical commitments and snapshots
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-20T19:44:03Z
- Branch: `main`
- Worktree: B-0006 is complete and included in the closing worktree for this baton
  reset.

## Objective

Define and implement canonical PolicyIR encoding and `policy_hash` in Rust with
versioned deterministic vectors and a TypeScript vector parser. Domain-specific
relationship and snapshot commitments remain separate batons.

## Acceptance Criteria

- [ ] One accepted decision record fixes the hash algorithm, signed integer encoding,
      UTF-8 normalization, optional presence bytes, length widths, and structural
      ordering.
- [ ] The Rust canonical crate encodes every approved PolicyIR field without hashing
      Protobuf bytes.
- [ ] `policy_hash` uses the exact ASCII domain `raziel/policy/v1` and rejects unknown
      or unsupported PolicyIR inputs.
- [ ] Checked-in vectors include human input, canonical bytes hex, domain separator,
      expected digest, Rust result, TypeScript parser result, and negative mutations.
- [ ] Focused and affected workspace gates pass.

## Architecture Constraints

- Rust remains the only commitment authority.
- TypeScript parses and displays vectors. It does not perform proof-critical hashing.
- The agent sees `policy_hash`, not PolicyIR. The verifier resolves the approved
  PolicyIR through its trusted local registry.
- Every commitment is versioned and domain-separated.
- Raw Protobuf bytes are never commitment inputs.
- Unknown fields, enum values, and unsupported PolicyIR nodes fail closed.
- Test vectors contain no private relationship tuples or caveat values.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `docs/decisions/` for the canonical encoding decision
- `crates/canonical/` for Rust canonical encoding and hashing
- `crates/policy-ir/` only for integration with validated PolicyIR access
- `tests/vectors/` or `tests/canonical/` for deterministic vectors
- `packages/proto-ts/` for vector parsing and tests only
- Cargo metadata only after dependency approval
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Relationship leaf encoding, trees, salts, openings, and witness construction
- Snapshot descriptors, signatures, registries, and revocation
- Capability commitments
- Services, gateway, SDK, console, SP1 execution, and proof generation
- Real transaction data
- Production proof ceilings

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0007.
- Pending: canonical hash algorithm, signed integer encoding, UTF-8 normalization,
  optional presence encoding, length widths, and structural ordering.

## Open Questions and Blockers

- The hash algorithm is not approved yet.
- Signed integer encoding is not approved yet.
- UTF-8 normalization behavior is not approved yet.
- The normal development `spicedb` service has a pre-existing command and key
  incompatibility. It belongs to a separate service-integration baton.

## Work Completed

- B-0006 completed the M1 exit gate and is archived as
  `docs/agent/history/B-0006-spicedb-policy-ir-equivalence.md`.
- `crates/canonical/` exists as the M2 placeholder from the initial repository
  scaffold.

## Validation Plan

- Deterministic positive and negative canonical vectors
- Mutated-field and malformed-input tests
- Rust canonical crate tests and workspace tests
- TypeScript parser tests over the same vectors
- Affected format, lint, typecheck, build, and generation gates
- `git diff --check`

## Exact Next Action

Claim B-0007, reconcile it with clean `HEAD`, and draft the canonical encoding
decision. Request maintainer approval for semantic choices and any dependency before
implementation.
