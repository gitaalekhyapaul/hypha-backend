# Current Engineering Baton

- Baton ID: B-0017
- State: READY
- Milestone: M4, capability issuance pre-queue admission
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-12T08:18:16Z
- Branch: `main`
- Worktree: B-0016 is archived at
  `docs/agent/history/B-0016-capability-window-doc-consistency.md`. It reconciles the
  accepted 75-minute Plonk window and closes the stale O-008 documentation conflict.

## Objective

Implement a pure, bounded pre-queue admission boundary for generated
`CreateCapabilityRequest` values. Convert transport fields into the existing
ADR-0011 canonical request model, derive the agent-key thumbprint, rebuild the exact
request commitment, verify the Ed25519 issuance signature over that 32-byte digest,
enforce the accepted local Plonk validity window, and derive the deterministic job
identifier. Stop before storing private data or contacting Redis.

## Acceptance Criteria

- [ ] Generated request values convert through strict typed validation into the
      proof-critical canonical request model.
- [ ] Admission derives the agent thumbprint from the supplied Ed25519 public key and
      never trusts `agent_id` for authorization.
- [ ] The request signature verifies over exactly the ADR-0011 request commitment.
- [ ] The deterministic job identifier derives from that commitment.
- [ ] Admission enforces the 75-minute local Plonk window against trusted API time and
      rejects malformed, expired, or insufficient upper bounds.
- [ ] Every request field, context entry, public key, and signature mutation fails or
      changes the derived commitment before any queue operation.
- [ ] Errors and debug output contain no private context values, key bytes, signature
      bytes, or commitment bytes.
- [ ] Focused tests, affected workspace tests, strict Clippy, formatting, and diff
      checks pass.

## Architecture Constraints

- `agent_id` remains presentation metadata and cannot select the PolicyIR subject.
- Raw Protobuf bytes are never commitment inputs.
- Canonical encodings and commitment semantics stay in proof-critical Rust crates.
- Private request context cannot enter public errors or standard logs.
- The API does not publish a BullMQ job until admission succeeds.
- The proof worker must recheck validity bounds against its trusted start time in a
  later baton.
- ADR-0002 governs protected gateway requests, not this approved issuance signature.

## File Scope

- `services/razield/Cargo.toml`
- `services/razield/src/lib.rs`
- `services/razield/src/admission.rs`
- `services/razield/tests/` only for admission boundary coverage
- Existing canonical or authorization-core tests only if a missing exported helper is
  required without changing semantics
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- BullMQ publication, queue state reads, cancellation, or workers
- Private envelope serialization, filesystem storage, or cleanup
- Witness construction, guest execution, proof generation, or proof verification
- Tonic handlers, listeners, interceptors, TLS, or transport policy
- Capability artifact issuance or protected-request verification
- ADR-0002 or O-007
- Dependencies not already approved and workspace-pinned
- Another proof run, M5, M6, M7, or another scenario

## Decisions

- Inherited: ADR-0011 defines the exact capability request commitment and issuance
  nullifier.
- Inherited: ADR-0013 accepts Ed25519 issuance authentication over that exact digest.
- Inherited: ADR-0013 accepts the 75-minute local Plonk capability window and a
  15-minute minimum usable period after a successful proof.
- Inherited: Existing workspace-pinned Rust crates can be selected by `razield`; no
  new package or version is approved.
- Made: None.

## Open Questions and Blockers

- None block the pure admission boundary.
- The proof worker must repeat time-window validation at its trusted start time.
- ADR-0002 and O-007 remain open before protected-request gateway integration.
- A later positive isolated-verifier gate still needs explicit proof-run approval.

## Work Completed

- B-0015 compiled generated Rust service contracts.
- B-0016 reconciled the accepted capability-window documentation.
- Existing ADR-0011 canonical request and thumbprint helpers remain to be inspected
  before selecting the smallest adapter surface.

## Validation Plan

- Run focused admission unit and mutation tests.
- Confirm no queue or filesystem operation occurs in the admission module.
- Run affected `razield`, authorization-core, and canonical tests.
- Run strict Clippy and repository formatting.
- Run `git diff --check` and inspect changed-file scope.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0016 archive | PASS | Capability-window documentation consistency | 2026-08-12T08:18:16Z |

## Privacy Checks

- NOT STARTED: Public errors and debug output must not contain private request values.
- NOT STARTED: Tests must use synthetic context values only.
- NOT APPLICABLE: No proof, witness, relationship snapshot, or transaction data is
  processed in this baton.

## Risks and Private-Data Notes

- A transport-to-canonical mapping error can sign one request and prove another.
- Error formatting can leak private caveat context or key material.
- API-time validation alone cannot guarantee a 15-minute usable period after queue
  delay. The proof worker must fail closed at its trusted start time.

## Exact Next Action

After the B-0016 documentation commit, claim B-0017. Inspect ADR-0011 request types,
canonical encoders, and existing Ed25519 verification helpers before editing service
code.
