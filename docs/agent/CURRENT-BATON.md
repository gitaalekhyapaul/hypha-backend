# Current Engineering Baton

- Baton ID: B-0079
- State: ACTIVE
- Milestone: Review and release the selected console identity
- Owner: Codex release coordinator
- Started UTC: 2026-11-04T12:39:39Z
- Last updated UTC: 2026-11-04T12:39:39Z
- Branch: `main`
- Observed worktree: backend and demo-agents contain the explained B-0078 branding changes. SDK remains clean.
- Rollback: Existing remote `v1` is unchanged. No release commit or deployment occurred.

## Objective

Publish the reviewed console branding and Beamer presentation. Fast-forward the
VM checkouts and refresh the operator, Agent A, and Agent B consoles only.

## Acceptance Criteria

- [x] User authorized scoped commits, pushes, and VM refresh on 4 November.
- [x] Beamer presentation builds independently of the report and ends in a demo placeholder.
- [ ] Publish scoped backend, demo-agents, and report commits.
- [ ] Fast-forward the VM checkouts without overwriting local work.
- [ ] Rebuild or recreate only the three console services and verify served brand assets.

## Architecture Constraints

All proof, privacy, signature, registry and SDK invariants remain unchanged.
Branding does not establish a fresh live proof or close historical acceptance gates.
Do not change backend behavior to resolve a visual review item.

## File Scope

The release includes B-0078 operator branding, sibling demo-agent branding,
focused tests, and baton records. The revised editable Beamer source lives in
`raziel-final-report/presentation/`. The report text and SDK are unchanged.

## Out Of Scope

SDK publication, new proof submissions, data resets, and restarts of proof workers,
Redis, databases, verifier, channel, or tunnel services. B-0077 remains deferred.

## Decisions Inherited

- Chamber Lantern `Capstone/brand/final/design.md` is authoritative.
- Every specialist in B-0078 used GPT-6.1 Sol with xHigh reasoning.
- The final report and SDK repository were not edited.
- Obsidian requires a 20-minute demo-inclusive talk and five minutes of Q&A.
- The Beamer revision contains 16 slides. Allow 17 minutes for the talk and
  three for the updated recording. The final slide is a placeholder.
- The existing `v1` tags are not moved. Use fast-forward updates and retain
  VM secrets and state. If a checkout diverged, report the exact blocker.
- B-0077 remains deferred in `history/B-0077-deferred-for-brand-rollout.md`.
  It was not completed through this branding task.
- B-0078 evidence and the complete file map are archived in
  `history/B-0078-chamber-lantern-branding-and-presentation.md`.

## Open Questions And Blockers

The SSH ControlMaster is active. VM preflight, build, and smoke results are pending.
No VM mutation is authorized outside the scoped console release.

## Work Completed

B-0078 implemented operator and shared Agent A/B visual branding, named public
asset delivery, local typography and focused tests. The evaluator package includes
editable slides, a PDF review copy, source-linked notes, a rehearsal/demo run sheet
and renders. Parent review covered app diffs and all 24 slide renders.

## Validation Plan

For requested edits, repeat the narrow affected checks. Before any commit, review
the complete diff, run `git diff --check`, and check the staged files for secrets.
Deployment checks require a separate, explicit live scope.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Operator tests | PASS, 212 tests | Seven focused files | 2026-11-03 |
| Agent tests | PASS, 18 tests | Shared A/B application | 2026-11-03 |
| Type/lint/format | PASS | Applicable existing frontend checks | 2026-11-03 |
| Production build | PASS | Operator, pinned image, no network | 2026-11-03 |
| Desktop browser checks | PASS | 1600px/1280px, zero page errors or broken images | 2026-11-03 |
| Presentation validation | PASS | 24 slides, 24 notes, native tables/chart, re-import | 2026-11-03 |
| Presentation/PDF review | PASS | 1200-second main schedule, all slide renders, 24 PDF pages | 2026-11-03 |
| Diff checks | PASS | Backend and demo-agents, SDK clean | 2026-11-03 |

## Risks And Private-Data Notes

The console screenshots under `support/branding-validation/` are synthetic
interface fixtures, not fresh experiment evidence. The presentation uses dated,
unchanged report screenshots for the recorded synthetic demonstration. Local
fonts are self-hosted in applications. The PDF preserves presentation typography.
PowerPoint editing needs the selected fonts, as described in the package README.

The local operator visual preview is `http://127.0.0.1:3310`, container
`raziel-brand-console-b0078`. It is not a provisioned live stack. Temporary agent
fixture servers have been stopped. Existing services and the VM were untouched.

## Exact Next Action

Publish the scoped commits. Then fast-forward the VM and verify the three
branded consoles without changing proof infrastructure or state.
