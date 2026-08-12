# Current Engineering Baton

- Baton ID: B-0016
- State: READY
- Milestone: M4, accepted capability-window documentation consistency
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-12T08:06:55Z
- Branch: `main`
- Worktree: B-0015 is archived at
  `docs/agent/history/B-0015-generated-tonic-contract-compilation.md`. Its generated
  contract, dependency boundary, workspace, generation, and formatting gates pass.

## Objective

Reconcile stale O-008 markers with ADR-0013's accepted approval record. Record the
approved 75-minute local Plonk capability window and 15-minute minimum usable window
consistently. Make no new protocol decision and change no implementation.

## Acceptance Criteria

- [ ] ADR-0013 lists only O-007 as an open related decision.
- [ ] ADR-0013 no longer describes the accepted 75-minute window as proposed or
      awaiting approval.
- [ ] The technical spec removes O-008 from the open-decision table.
- [ ] FR-063 matches the accepted 75-minute proof-bound window and 15-minute minimum
      usable window after a successful proof.
- [ ] No source, schema, generated, lockfile, or dependency file changes.
- [ ] Documentation checks, formatting, link checks, and `git diff --check` pass.

## Architecture Constraints

- The accepted ADR-0013 capability-window semantics do not change.
- The guest continues to bind `issued_at` and `expires_at`.
- Expiry remains capped by the request, policy, snapshot, delegation, and key bounds.
- A proof reaching the 60-minute deadline yields no capability.
- ADR-0002 and O-007 remain open and unchanged.

## File Scope

- `docs/decisions/0013-m4-service-and-verifier-execution.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Capability admission or signature implementation
- Public values, Protobuf contracts, canonical encodings, or proof semantics
- Service handlers, listeners, queue publication, private blobs, or workers
- Dependencies, toolchains, generated files, or proof runs
- ADR-0002 or O-007 approval
- M5, M6, M7, or another scenario

## Decisions

- Inherited: ADR-0013 was accepted on 2026-08-12.
- Inherited: Its approval record accepts the 75-minute Plonk capability window and
  states that this closes O-008.
- Inherited: No new decision or approval is permitted in this consistency baton.
- Made: None.

## Open Questions and Blockers

- A source-of-truth conflict blocks capability-window implementation until these
  stale markers are reconciled.
- ADR-0002 and O-007 remain open before the stolen-capability exit gate can close.
- A later positive isolated-verifier gate still needs explicit proof-run approval.

## Work Completed

- Identified the conflicting O-008 status markers during the B-0015 handoff review.
- Confirmed that ADR-0013's approval record explicitly accepts the 75-minute window.

## Validation Plan

- Search active architecture and specification documents for O-008 and the old
  15-minute default.
- Check local Markdown links and formatting.
- Confirm that the diff contains documentation and baton files only.
- Run `git diff --check`.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0015 archive | PASS | Generated Tonic contract compilation | 2026-08-12T08:06:55Z |

## Privacy Checks

- NOT APPLICABLE: This baton handles public protocol documentation only.
- NOT APPLICABLE: No proof, witness, relationship, caveat, or transaction data is read.

## Risks and Private-Data Notes

- Leaving O-008 marked open can cause later code to retain an unusable 15-minute
  proof-bound default.
- This correction must not broaden the 75-minute ceiling or weaken subordinate expiry
  bounds.

## Exact Next Action

After the B-0015 checkpoint commit, claim B-0016. Patch only the stale O-008 and
FR-063 text, then run documentation and diff checks.
