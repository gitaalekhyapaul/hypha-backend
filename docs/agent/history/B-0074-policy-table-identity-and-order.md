# Current Engineering Baton

- Baton ID: B-0074
- State: READY
- Milestone: M7, identifiable recent-first policy table
- Owner: Codex
- Started UTC: 2026-10-18T12:28:34Z
- Last updated UTC: 2026-10-18T12:39:00Z
- Branch: `main`
- Worktree: Clean at start, backend `c2cc024`.
- Rollback: Existing `v1` remains unchanged.

## Objective

Sort the installed policy table newest first and show enough approved metadata to
match its rows with the policy Version ID in Agents / Roles and teams.

## Acceptance Criteria

- [x] Latest valid-from policies appear first with deterministic ties.
- [x] Full Version ID and Family ID are separately labeled in the table.
- [x] Version, approval epoch, validity, record ID and policy hash are available.
- [x] Existing family selection and refresh still work.
- [x] Focused tests, TypeScript, formatting, and a desktop browser check pass.

## Architecture Constraints

Operator presentation only. Use existing approved catalogue fields. No API, private
policy exposure, guest, proof, role, SDK, signature, or queue changes.

## File Scope

- `apps/demo-console/components/policy-catalogue.tsx`
- `apps/demo-console/lib/policy-version/catalogue-types.ts`
- `apps/demo-console/test/policy-catalogue.test.ts`
- Existing policy browser fixture if needed.
- Current baton, history, and M7 runbook.

## Out Of Scope

Proof-failure debugging, new API timestamps, public discovery, policy publishing
semantics, agent changes, and VM restarts.

## Decisions

- Sort by existing valid-from time. Do not relabel it as creation or publication time.
- Keep the full Version ID visible to match the existing Agents dropdown.
- Keep longer approval details expandable rather than expanding all rows.

## Open Questions And Blockers

None. Docker is available. No dependency installation is required.
The unstarted B-0073 diagnostic task is preserved in the deferred history record.

## Work Completed

Implemented a non-mutating recent-first display sort over existing catalogue
metadata. Added separately labeled full Version ID and Family ID, version number,
approval epoch, validity dates, and expandable record ID/hash/resource type.
Kept family selection and refresh behavior. No API shape changed.

The existing policy browser fixture now serves a controlled approved catalogue,
checks recency/identity/details, and uses the current Policy navigation label.

## Validation Plan

Focused sorting regressions, console suite/typecheck, scoped formatter, desktop
fixture browser, and `git diff --check`.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Worktree and Docker | PASS | Clean backend, Docker 29.8.2 | 2026-10-18T12:28:34Z |
| Console suite | PASS, 675 tests, 18 existing skips | Includes two new sort regressions | 2026-10-18 |
| Console TypeScript | PASS | Pinned Docker toolchain | 2026-10-18 |
| Scoped Biome | PASS, four files | Existing pinned formatter | 2026-10-18 |
| Browser fixture syntax | PASS | Native Node syntax check | 2026-10-18 |
| Desktop browser | PASS | Real console, controlled HTTP: IDs, order, details, family selection, refresh; no page errors | 2026-10-18 |
| Screenshot inspection | PASS | 1440 px desktop, collapsed and expanded table | 2026-10-18 |
| Whitespace and scoped diff | PASS | No generated route file retained | 2026-10-18 |
| VM deployment | SKIP | Maintainer pull/recreate still required | 2026-10-18 |

## Risks And Private-Data Notes

No new metadata leaves the operator boundary. The table cannot claim a creation
or publication timestamp that the existing catalogue does not provide.

## Exact Next Action

Pull the containing backend commit on the VM and recreate only demo-console using
the documented M7 policy-table update. No owner tools, agent image, guest, or proof
restart. Return to the still-unstarted diagnostic task after this checkpoint.

## Checkpoint And Privacy

Backend-only release, committed and pushed before handoff. SDK and agent repositories
remain untouched. The existing rollback tag remains unchanged. No new dependency.
No PolicyIR, tuple, witness, caveat value, or new metadata crossed a boundary.

The disposable local console and browser were stopped after validation. The dev
server's generated next-env.d.ts route-path drift was restored to the clean baseline.
Screenshots stay outside Git: /tmp/raziel-b0074-policy-table.png and
/tmp/raziel-b0074-policy-details.png.

## Validation Notes

The browser harness first encountered unavailable Node imports in the browser tool.
The direct controlled-page check ran without those imports. Its first navigation
locator used the old Policy builder label and timed out. The fixture now uses the
current Policy navigation button. The final desktop check passed.
