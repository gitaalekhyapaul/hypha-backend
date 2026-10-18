# Current Engineering Baton

- Baton ID: B-0070
- State: REVIEW
- Milestone: M7, role editor saved-dataset selection
- Owner: Codex
- Started UTC: 2026-10-18T09:02:00Z
- Last updated UTC: 2026-10-18T09:04:00Z
- Branch: `main`
- Worktree: clean baseline `a6910af`.
- Rollback: existing remote `v1` tag stays unchanged.

## Objective

Replace the role editor's hard-coded dataset suggestions with the existing saved
dataset catalogue. Include portal-created clones, revision/status labels, refresh,
and clear loading/error/empty states. The maintainer approved this correction
with "yeah, do that". B-0069's unstarted VM gate is deferred, not completed.

## Acceptance Criteria

- [x] Saved dataset IDs, including clones, appear without manual entry.
- [x] Revision and Draft/Finalized status are shown for each option.
- [x] Refresh reveals new clones and preserves an existing selected dataset.
- [x] Failed or empty catalogue reads do not leave stale role actions usable.
- [x] Browser regression, console tests, typecheck, formatting, and diff checks pass.
- [x] Runbook and baton record the correction and the pending VM gate.

## Architecture Constraints

Operator-only UI reuse of the existing authenticated dataset API. No new endpoint,
dependency, SDK, signature, guest, policy semantics, or role writer change. Dataset
selection does not authorize data access. Drafts still need finalization for queries.

## File Scope

- `apps/demo-console/components/agent-role-workspace.tsx`
- `apps/demo-console/test/agent-roles.ui.mjs`
- `docs/runbooks/M7-AGENT-DEMO.md`
- Current baton and dated history.

## Out Of Scope

Policy-family expansion, caveat editing, proof generation, VM source edits, data
reset, and changes to sibling repositories.

## Decisions

Use the existing `listDatasets` client. Show saved draft and finalized datasets,
not just the two seeded IDs. Keep the View roles confirmation on dataset changes.
This small, tightly coupled correction does not need parallel write scopes.

## Open Questions And Blockers

None. Docker 29.8.2 is available. No dependency installation is needed.

## Work Completed

- Confirmed the datalist contains only two literal suggestions and never reads saved datasets.
- Verified clone creates a distinct dataset ID, increments revision, and starts as Draft.
- Preserved [B-0069's pending VM gate](history/2026-10-18-b0069-vm-gate-deferred.md).
- Replaced static suggestions with a real saved-dataset select using the existing client.
- Added refresh, revision/status labels, and explicit inaccessible/error/empty states.
- Extended the existing browser regression for cloned draft selection and exact role scope.
- Browser screenshot inspected. Disposable local UI container removed, generated
  Next type-file drift restored. No VM source, runtime, proof, or sibling repo changed.

## Validation Plan

Extend the existing role-editor browser check with cloned datasets, refresh,
selection retention, failed/empty reads, and exact role request scope. Run console
Vitest, TypeScript, scoped Biome, and whitespace checks. No new Plonk run is needed.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Git status | PASS, clean baseline | Backend | 2026-10-18 |
| Docker availability | PASS, 29.8.2 | Existing local toolchain | 2026-10-18 |
| Full console Vitest | PASS, 673 passed, 18 skipped, zero failed | Existing optional compiler/live-service gates remain skipped | 2026-10-18T09:02:00Z |
| Console TypeScript | PASS | Pinned toolchain, no new dependencies | 2026-10-18T09:02:00Z |
| Scoped Biome and whitespace | PASS | Edited component and existing browser test | 2026-10-18 |
| Initial Chrome launch | FAIL, resolved by approved browser execution outside sandbox | No application change | 2026-10-18 |
| Initial select browser check | FAIL, resolved by explicit accessible label | New select now has exact Dataset ID label | 2026-10-18 |
| Browser role-editor regression | PASS | Clones, revision/status, refresh retention, failed/empty reads, exact scope, existing role actions | 2026-10-18T09:03:00Z |
| VM proof and two-agent recording | SKIP | Deferred B-0069 requirements remain open | 2026-10-18 |

## Risks And Private-Data Notes

No secrets or private graph data enter dataset options. Selection uses exact saved
dataset IDs, not the visible source label or policy version. Keep the existing
managed-mode and capability expiry/revocation semantics unchanged.

## Exact Next Action

Commit and push this UI-only correction. On the VM, pull backend main and recreate
only demo-console. No owner-tool, guest, or agent-image rebuild is needed for this
correction. Resume B-0069's still-open VM role and two-agent gate through B-0071.
