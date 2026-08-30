# Current Engineering Baton

- Baton ID: B-0056
- State: REVIEW
- Milestone: M5, remaining fail-closed desktop controls
- Owner: Codex
- Started UTC: 2026-08-30T08:13:33Z
- Last updated UTC: 2026-08-30T08:45:14Z
- Branch: `main`
- Worktree: B-0055 was archived and committed locally before this objective.

## Objective

Complete the approved Denial Lab screen inventory with controlled expired, altered,
and unsigned protected-request actions. Each action must reach the real gateway,
fail closed, return no rows, and preserve generic public errors.

## Acceptance Criteria

- [x] Denial Lab offers expired, altered, and unsigned request actions beside scope
      denial and exact replay.
- [x] Expired uses a correctly signed request timestamp outside the approved five
      minute freshness window.
- [x] Altered changes a signed body commitment after signing.
- [x] Unsigned removes the signature field from the submitted envelope.
- [x] Each gateway call returns a generic rejection with no protected rows or policy
      details.
- [x] Existing scope-denial and exact-replay actions remain unchanged.
- [x] Focused console checks and the production build pass.

## Architecture Constraints

- These are fixed authenticated demo controls, not a free-form request editor.
- The gateway remains the real enforcement point. The console cannot synthesize a
  denied result without receiving the gateway rejection.
- No request can reach the transaction store after freshness, body, or signature
  validation fails.
- No proof, capability, receipt, or policy wire format changes.

## File Scope

- `apps/demo-console/app/api/reconciliation/query/`
- `apps/demo-console/app/globals.css`
- `apps/demo-console/components/denial-lab-workspace.tsx`
- `apps/demo-console/lib/portal-client/reconciliation.ts`
- `apps/demo-console/test/reconciliation-route.test.ts`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0056-fail-closed-desktop-controls.md`

## Out Of Scope

- Free-form envelope editing
- Detailed gateway rejection reasons
- Rejected receipts from untrusted malformed envelopes
- Real transaction data
- Mobile layout polish

## Decisions

- Inherited: ADR-0002 defines the freshness and signed-envelope checks.
- Inherited: FR-089 keeps gateway denials generic.
- Local implementation choice: the authenticated console route maps a successful
  generic gateway rejection back to the selected controlled test label. The gateway
  response itself remains unchanged.

## Open Questions And Blockers

- None. Docker and the SSH ControlMaster were available. Their loss remains an
  external prerequisite failure that must be reported before related work continues.

## Work Completed

- Added fixed `EXPIRED`, `ALTERED`, and `UNSIGNED` console request modes.
- Added a stale correctly signed request at 301 seconds before submission.
- Added post-signing body-hash alteration and signature removal controls.
- Required one real generic gateway rejection before returning the selected
  operator-side result label.
- Added the three actions to the desktop Denial Lab without exposing free-form
  envelope editing or internal gateway reasons.
- Updated the live VM walkthrough with the exact controls and expected visible
  results.

## Validation Plan

- Add route tests that inspect each outbound mutation and require one real generic
  gateway rejection.
- Confirm no protected rows enter any browser response.
- Run focused console test, lint, type, format, and production build commands.
- Run `git diff --check` and inspect the final Denial Lab field set.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Demo-console focused reconciliation route test | PASS, 11 tests | Scope, replay, expired, altered, unsigned, and protected success | 2026-08-30T08:43:09Z |
| Demo-console lint and typecheck | PASS | Denial Lab and query contract | 2026-08-30T08:43:46Z |
| Demo-console full test | PASS, 64 tests | Existing M5 console flows and new controls | 2026-08-30T08:44:32Z |
| Demo-console production build | PASS | Next.js desktop application | 2026-08-30T08:44:56Z |
| `git diff --check` | PASS | B-0056 worktree | 2026-08-30T08:45:14Z |

## Risks And Private-Data Notes

- The operator-facing label identifies the selected controlled test. It does not
  claim a gateway-internal failure branch.
- Malformed and unsigned requests do not create claim-derived receipts because the
  gateway has not established trusted public claims.

## Exact Next Action

Commit B-0056 locally. Obtain maintainer approval to push B-0051 through B-0056, then
run the complete live M5 walkthrough on the NUS VM.
