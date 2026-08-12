# Engineering Baton History

- Baton ID: B-0016
- State: REVIEW
- Milestone: M4, accepted capability-window documentation consistency
- Owner: Codex
- Started UTC: 2026-08-12T08:16:41Z
- Last updated UTC: 2026-08-12T08:18:16Z
- Branch: `main`
- Worktree: B-0016 documentation changes are ready to commit on `main` after
  `5554908`. B-0015 is archived at
  `docs/agent/history/B-0015-generated-tonic-contract-compilation.md`.

## Objective

Reconcile stale O-008 markers with ADR-0013's accepted approval record. Record the
approved 75-minute local Plonk capability window and 15-minute minimum usable window
consistently. Make no new protocol decision and change no implementation.

## Acceptance Criteria

- [x] ADR-0013 lists only O-007 as an open related decision.
- [x] ADR-0013 no longer describes the accepted 75-minute window as proposed or
      awaiting approval.
- [x] The technical spec removes O-008 from the open-decision table.
- [x] FR-063 matches the accepted 75-minute proof-bound window and 15-minute minimum
      usable window after a successful proof.
- [x] No source, schema, generated, lockfile, or dependency file changes.
- [x] Documentation checks, formatting, link checks, and `git diff --check` pass.

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
- Made: None. This baton only reconciles already accepted text.

## Open Questions and Blockers

- A source-of-truth conflict blocks capability-window implementation until these
  stale markers are reconciled.
- ADR-0002 and O-007 remain open before the stolen-capability exit gate can close.
- A later positive isolated-verifier gate still needs explicit proof-run approval.

## Work Completed

- Identified the conflicting O-008 status markers during the B-0015 handoff review.
- Confirmed that ADR-0013's approval record explicitly accepts the 75-minute window.
- B-0016 claimed at commit `5554908` with a clean worktree.
- Removed O-008 from active open-decision markers and replaced FR-063's stale
  15-minute default with the accepted 75-minute ceiling and 15-minute usable minimum.
- Preserved the proof deadline, subordinate expiry bounds, revocation checks, and
  ADR-0002 status.

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
| `git status --short` | PASS, empty | B-0016 start state | 2026-08-12T08:16:41Z |
| Active-document stale-marker search | PASS, no stale matches | O-008 and old 15-minute default | 2026-08-12T08:17:33Z |
| Active O-008 mention review | PASS | Remaining mentions state that ADR-0013 closes it | 2026-08-12T08:17:33Z |
| `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-12T08:17:33Z |
| File-scope and `git diff --check` | PASS | Three approved documentation files only | 2026-08-12T08:17:33Z |

## Privacy Checks

- NOT APPLICABLE: This baton handles public protocol documentation only.
- NOT APPLICABLE: No proof, witness, relationship, caveat, or transaction data is read.

## Risks and Private-Data Notes

- Leaving O-008 marked open can cause later code to retain an unusable 15-minute
  proof-bound default.
- This correction must not broaden the 75-minute ceiling or weaken subordinate expiry
  bounds.

## Exact Next Action

Commit the B-0016 documentation correction. Then claim B-0017 and implement the pure
`CreateCapability` pre-queue admission boundary from approved ADR-0011 and ADR-0013.
