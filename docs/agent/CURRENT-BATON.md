# Current Engineering Baton

- Baton ID: B-0023
- State: READY
- Milestone: M4, trusted authorization input assembly
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-22T09:09:59Z
- Branch: `main`
- Worktree: B-0022 is archived at
  `docs/agent/history/B-0022-canonical-positive-path-witness-construction.md`. Its
  scoped implementation and the B-0023 reset await one commit after `5b2f84c`.

## Objective

Implement the bounded host boundary that combines one admitted capability request,
one trusted approved policy, one trusted signed snapshot selection, and B-0022 witness
construction into a validated `AuthorizationGuestInputV1`. Do not execute SP1 or
start the production proof supervisor in this baton.

## Acceptance Criteria

- [ ] Input assembly accepts trust-bearing policy and snapshot selections. It cannot
      approve its own PolicyIR or snapshot descriptor.
- [ ] Snapshot ID, policy hash, relationship root, request scope, agent key, trusted
      issue time, and requested expiry bind to the exact assembled guest input.
- [ ] The request-to-evaluation mapping uses the existing canonical request and
      authorization-core rules without duplicating commitment encoding.
- [ ] B-0022 builds the minimum positive witness from the exact selected snapshot.
- [ ] The assembled guest input passes host authorization before it can reach a
      one-shot prover.
- [ ] Denial, registry mismatch, snapshot mismatch, invalid time, and witness failure
      produce closed typed errors with no private values.
- [ ] Focused assembly tests, affected cross-component tests, strict Clippy,
      formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- `razield` cannot mark its own PolicyIR as approved.
- Every proof input binds an approved policy record and signed snapshot descriptor.
- Policy and snapshot selection happen before witness construction. Request data
  cannot select a different policy, root, salt, relationship set, ELF, or proof mode.
- Relationship tuples, salts, caveat values, paths, openings, and assembled guest
  input remain inside the data-owner proof boundary.
- Canonical encodings, hashes, PolicyIR semantics, snapshot leaves, public values,
  capability fields, and guest behavior do not change in this baton.
- No hosted prover or SP1 network path receives private input.

## File Scope

- `services/razield/src/input_assembly/`
- `services/razield/src/lib.rs`
- `services/razield/Cargo.toml`
- Existing Rust crates only when a narrow trust-bearing type export is required
- `Cargo.lock` only for existing internal workspace selections
- Focused `services/razield/tests/` assembly checks
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- New policy approval or snapshot signing behavior
- A new registry storage format or authority key format
- One-shot child-process isolation, cgroups, or SP1 execution
- Production supervisor activation or terminal queue publication
- `GetCapability`, `CancelCapability`, `verifierd`, ADR-0002, or O-007
- M5, real transaction data, M6 acceptance, M7, or another scenario
- New external dependencies, package versions, toolchains, or proof runs

## Decisions

- Inherited: B-0017 owns admission and the exact capability request binding.
- Inherited: B-0022 owns positive-path selection and validated Merkle witness assembly.
- Inherited: `authorization-core` owns guest input validation and the shared `ALLOW`
  statement.
- Inherited: Local authority and registry types remain the only source of approved
  policy and signed snapshot trust.
- Made: None.

## Open Questions and Blockers

- The first design step must identify the current trust-bearing policy and snapshot
  selection types. A new storage format or approval path needs a separate decision.
- No new external dependency is planned.

## Work Completed

- B-0022 implemented and validated canonical positive-path witness construction.
- No B-0023 implementation has started.

## Validation Plan

- Read the authority, verifier registry, admission, private-envelope,
  authorization-core, and witness contracts before fixing the file map.
- Run focused assembly cases for a valid direct path, denial, policy mismatch,
  snapshot mismatch, time mismatch, and private output.
- Run affected service, authority, verifier, authorization-core, and witness tests.
- Run the non-guest workspace gate after focused feature checks pass.
- Run strict Clippy, repository formatting, privacy scans, and `git diff --check`.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0022 archive | PASS | Canonical positive-path witness construction | 2026-08-22T09:09:59Z |
| B-0023 implementation | NOT STARTED | No feature files edited | 2026-08-22T09:09:59Z |

## Privacy Checks

- NOT STARTED: Assembly errors and debug output must omit requests, relationships,
  caveats, salts, paths, openings, and guest input.
- NOT APPLICABLE: No real transaction data or proof run is part of this baton.

## Risks and Private-Data Notes

- A structurally valid PolicyIR value is not proof of approval. Assembly must require
  a trust-bearing approved selection.
- A signed snapshot descriptor does not carry private relationships or salt. The
  selected private snapshot data must match its committed root and snapshot ID.
- Host authorization is a pre-prover validation gate. It does not replace the SP1
  proof or independent verification.

## Exact Next Action

Inspect the current approved-policy record, signed-snapshot selection, admission,
private-envelope, and guest-input types. Then claim B-0023 with the smallest viable
production file map.
