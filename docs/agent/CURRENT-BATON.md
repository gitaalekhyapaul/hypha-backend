# Current Engineering Baton

- Baton ID: B-0056
- State: ACTIVE
- Milestone: M5, remaining fail-closed desktop controls
- Owner: Codex
- Started UTC: 2026-08-30T08:13:33Z
- Last updated UTC: 2026-08-30T08:13:33Z
- Branch: `main`
- Worktree: B-0055 is archived. Its public receipt ledger awaits a scoped local
  commit.

## Objective

Complete the approved Denial Lab screen inventory with controlled expired, altered,
and unsigned protected-request actions. Each action must reach the real gateway,
fail closed, return no rows, and preserve generic public errors.

## Acceptance Criteria

- [ ] Denial Lab offers expired, altered, and unsigned request actions beside scope
      denial and exact replay.
- [ ] Expired uses a correctly signed request timestamp outside the approved five
      minute freshness window.
- [ ] Altered changes a signed body commitment after signing.
- [ ] Unsigned removes the signature field from the submitted envelope.
- [ ] Each gateway call returns a generic rejection with no protected rows or policy
      details.
- [ ] Existing scope-denial and exact-replay actions remain unchanged.
- [ ] Focused console checks and the production build pass.

## Architecture Constraints

- These are fixed authenticated demo controls, not a free-form request editor.
- The gateway remains the real enforcement point. The console cannot synthesize a
  denied result without receiving the gateway rejection.
- No request can reach the transaction store after freshness, body, or signature
  validation fails.
- No proof, capability, receipt, or policy wire format changes.

## File Scope

- `apps/demo-console/app/api/reconciliation/query/`
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

- None. Docker and the SSH ControlMaster are available. Their loss is an external
  prerequisite failure that must be reported before related work continues.

## Work Completed

- Reconciled the remaining screen inventory with existing gateway validation order
  and approved public error codes.

## Validation Plan

- Add route tests that inspect each outbound mutation and require one real generic
  gateway rejection.
- Confirm no protected rows enter any browser response.
- Run focused console test, lint, type, format, and production build commands.
- Run `git diff --check` and inspect the final Denial Lab field set.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Source reconciliation against ADR-0002 and M5 screen inventory | PASS | Remaining denial controls | 2026-08-30T08:13:33Z |

## Risks And Private-Data Notes

- An operator-facing label can identify the selected controlled test. It cannot claim
  a gateway-internal failure branch.
- Malformed and unsigned requests do not create claim-derived receipts because the
  gateway has not established trusted public claims.

## Exact Next Action

Commit archived B-0055 locally, then extend the exact console query contract with the
three fixed controlled modes.
