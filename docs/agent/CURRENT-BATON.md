# Current Engineering Baton

- Baton ID: B-0010
- State: READY
- Milestone: M3, SP1 execution and proof
- Owner: unassigned
- Started UTC: None
- Last updated UTC: 2026-07-23T11:45:01Z
- Branch: `main`
- Worktree: B-0009 is archived with its authority-signature and local-registry
  implementation. Its closeout preserves maintainer-authored TEE follow-up text.

## Objective

Define and implement the first M3 slice: the bounded SP1 authorization guest input,
exact `AuthorizationPublicValues`, host input builder, and execute-mode equivalence.
Stop before local proof generation.

## Acceptance Criteria

- [ ] One accepted decision record fixes the versioned guest input, private and public
      split, public-value encoding, request binding, witness-consumption rules, and
      failure rules.
- [ ] The host input builder rejects malformed, oversized, duplicate, ambiguous, and
      out-of-order inputs before guest execution.
- [ ] The guest recalculates the policy commitment, verifies every relationship
      opening, evaluates PolicyIR and caveats, rejects unused witness entries, and
      commits only `ALLOW`.
- [ ] Execute-mode cases match the Rust host evaluator for the approved supported
      subset.
- [ ] Public-value and witness field mutations fail closed.
- [ ] Focused and affected workspace gates pass.

## Architecture Constraints

- The baseline guest proves `ALLOW` only.
- The agent never receives PolicyIR, relationship tuples, paths, openings, salts, or
  stored caveat values.
- Raw Protobuf bytes are never commitment inputs.
- Every proof binds an approved policy record and signed snapshot descriptor.
- The capability carries `policy_hash`, not PolicyIR.
- Guest public values contain only the fields named in
  `AuthorizationPublicValues`.
- `verifierd` never calls `razield`.
- Host-only signing and registry code cannot enter the SP1 guest dependency graph.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `ARCHITECTURE.md` and `docs/spec/POC-TECHNICAL-SPEC.md`
- One proposed M3 guest input and public-value decision record
- `proto/raziel/v1/` for an approved public-value wire contract
- `programs/authorization-guest/`
- `crates/prover/` and `crates/witness/`
- Guest-compatible shared types in `crates/canonical/`, `crates/policy-ir/`, and
  `crates/snapshot/` only where the accepted contract needs them
- Execute-mode equivalence, mutation vectors, and tests
- Cargo metadata after dependency review and approval
- Baton records

## Out of Scope

- Local proof generation and proof benchmarking
- Proof verification inside `verifierd`
- Capability job APIs, request signatures, and replay control
- Next.js gateway, SDK, and console work
- Real transaction data
- Service process wiring, which remains M4 work
- A denial proof or private-policy model
- Hypha dataset relevance attestation

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0010.
- M2 supplies canonical PolicyIR commitments, blinded relationship openings, signed
  snapshot descriptors, and verifier-local registry semantics.
- Exact guest input and public-value bytes remain subject to an M3 decision record and
  explicit maintainer approval.

## Open Questions and Blockers

- Open: exact versioned guest input shape and maximum encoded size.
- Open: exact canonical byte encoding for `AuthorizationPublicValues`.
- Open: the guest-compatible shared crate boundary for PolicyIR, snapshot openings,
  caveat inputs, and request commitments.
- Open: exact unused-witness accounting for union, intersection, and arrow paths.
- Blockers: None for protocol inspection and decision drafting.
- The normal development `spicedb` service has a pre-existing command and key
  mismatch. Record its effect on live equivalence work during baton claim.
- Deferred follow-up: carry these items through each baton reset. After the M7 exit
  gate passes and the PoC meets its definition of done, remind the maintainer about
  the separate Hypha relevance-attestation scope, the Obsidian
  `Current Implementation Discussion Post`, and the TEE-backed local-authorization
  experiment in `docs/spec/POC-TECHNICAL-SPEC.md`.

## Work Completed

- M0 established the repository, toolchain, contracts, and service shells.
- M1 delivered the strict SpiceDB compiler adapter, PolicyIR semantics, and live
  equivalence harness.
- M2 delivered canonical commitments, blinded relationship snapshots, authority
  signatures, strict local registries, and deterministic cross-language vectors.
- B-0009 is archived as
  `docs/agent/history/B-0009-authority-signatures-and-local-registries.md`.

## Validation Plan

- Deterministic guest input and public-value vectors
- Host and guest equivalence for every approved positive case
- Malformed, oversized, unused, duplicate, ambiguous, and out-of-order witness tests
- One-field public-value and witness mutations
- SP1 execute-mode tests without proof generation
- Guest dependency-tree check
- Rust focused tests and affected workspace gates
- Format, lint, typecheck, build, generation, `git diff --check`, and private-data scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0009 archive | PASS | M2 authority signatures and local registries completed with all gates | 2026-07-23T11:45:01Z |
| Not run | B-0010 remains unclaimed | M3 guest execution | 2026-07-23T11:45:01Z |

## Risks and Private-Data Notes

- Guest inputs contain private witness material and must never enter normal logs,
  capability artifacts, browser responses, or checked-in runtime files.
- Public-value bytes become proof-system protocol. Any field, order, width, or
  canonicalization change needs mutation vectors and a decision record.
- The host builder cannot silently remove unused witness data to make a guest case
  pass.
- A host evaluator result cannot replace SP1 execute-mode agreement.

## Exact Next Action

Claim B-0010, inspect the guest, prover, witness, PolicyIR, snapshot, and canonical
crate graphs, then draft the M3 guest input and public-value decision record before
changing public proof values.
