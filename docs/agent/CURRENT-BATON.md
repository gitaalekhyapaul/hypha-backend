# Current Engineering Baton

- Baton ID: B-0009
- State: READY
- Milestone: M2, Canonical commitments and snapshots
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-23T05:00:24Z
- Branch: `main`
- Worktree: `main` at `c1359e1` with completed B-0008 changes awaiting final stage
  and commit.

## Objective

Define signed snapshot descriptors and local approved-policy and accepted-root
registries. Resolve O-004 signing and key-storage rules before implementation.

## Acceptance Criteria

- [ ] One accepted decision record fixes the snapshot signing algorithm, trusted key
      identifiers and storage, canonical signed bytes, rotation, and failure rules.
- [ ] Rust signs and verifies descriptors that bind the exact policy hash, relationship
      root, SpiceDB revision, epoch, issue time, expiry, and canonicalization version.
- [ ] Local registries fail closed for unapproved policies, unknown roots, revoked
      roots, expired descriptors, unknown keys, and invalid signatures.
- [ ] Deterministic vectors and one-field mutation tests cover descriptor signatures
      and registry status transitions.
- [ ] Focused and affected workspace gates pass.

## Architecture Constraints

- `razield` cannot mark its own PolicyIR as approved.
- `verifierd` resolves `policy_hash` through its trusted local policy registry.
- Every proof binds an approved policy record and signed snapshot descriptor.
- The verifier never calls `razield`.
- Descriptor bytes and signatures contain no relationship tuple, salt, opening,
  satisfying path, or stored caveat value.
- Raw Protobuf bytes are never signature or commitment inputs.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `ARCHITECTURE.md` and `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/decisions/` for O-004 and any registry trust decision
- Existing snapshot and verifier-side Rust crates selected after inspection
- Existing Protobuf contracts only if the accepted decision requires a wire correction
- Deterministic descriptor and registry vectors and tests
- Cargo metadata only after dependency approval
- Baton records

## Out of Scope

- SpiceDB relationship loading and ZedToken retrieval
- Witness path search and PolicyIR evaluation
- SP1 guest execution and proof generation
- Capability issuance, gateway enforcement, SDK, and console
- Real transaction data
- Production hardware key management

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0009.
- ADR-0008 fixes canonical SHA-256 framing and makes Rust the commitment authority.
- ADR-0009 fixes the exact relationship root and private witness protocol.
- Open O-004: snapshot signing algorithm and authority key storage.

## Open Questions and Blockers

- O-004 must be resolved before descriptor signature implementation.
- The normal development `spicedb` service has a pre-existing command and key
  incompatibility. It does not block descriptor and registry design.
- Deferred follow-up: carry this item through each baton reset. After the M7 exit gate
  passes, remind the maintainer to revisit the separate Hypha relevance-attestation
  scope and the Obsidian `Current Implementation Discussion Post`.

## Work Completed

- B-0008 implemented and validated the accepted blinded relationship-tree protocol.
- B-0008 is archived as
  `docs/agent/history/B-0008-blinded-relationship-tree.md`.
- The remaining M2 deliverables are the signed snapshot descriptor and local policy
  and accepted-root registries.

## Validation Plan

- Deterministic canonical descriptor and signature vectors
- Signature, key, field, expiry, revocation, and registry mutation tests
- Rust focused tests and affected workspace tests
- Format, lint, typecheck, build, generation, `git diff --check`, and privacy scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0008 archive | PASS | Blinded relationship-tree objective completed with all gates | 2026-07-23T05:00:24Z |

## Risks and Private-Data Notes

- A descriptor signature is a public trust artifact. Ambiguous bytes, weak key
  identity, or stale registry status can turn a valid signature into an invalid trust
  decision.
- Test keys must be synthetic and marked non-production.
- Private relationship and witness material must never enter descriptor vectors,
  registry records, errors, or logs.

## Exact Next Action

Inspect the existing descriptor Protobuf, registry placeholders, architecture, and
prior decisions. Draft the O-004 decision and request maintainer approval before
changing signature or registry behavior.
