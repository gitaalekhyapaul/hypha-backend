# Current Engineering Baton

- Baton ID: B-0054
- State: ACTIVE
- Milestone: M5, exact-envelope replay rejection
- Owner: Codex
- Started UTC: 2026-08-30T07:48:58Z
- Last updated UTC: 2026-08-30T07:48:58Z
- Branch: `main`
- Worktree: B-0053 is archived. Its controlled scope-denial implementation awaits a
  scoped local commit.

## Objective

Add the UF-11 desktop action that sends one valid protected request, resubmits the
exact signed envelope and nonce, and shows a generic replay rejection without
returning the first request's transaction rows to the browser.

## Acceptance Criteria

- [ ] Denial Lab offers one replay action after a live capability succeeds.
- [ ] The console sends one valid request and then resubmits the exact same signed
      envelope and nonce.
- [ ] The first request completes normally and records its accepted receipt.
- [ ] The replay is rejected by the gateway-owned nonce store before a second proof
      verification or dataset read.
- [ ] The browser receives a replay result with no protected rows or policy details.
- [ ] The gateway records one coarse rejected replay receipt from an existing trusted
      cache entry without accepting unverified claim data.
- [ ] Existing in-scope, cache-hit, and scope-denial paths remain unchanged.
- [ ] Focused gateway and console checks pass.

## Architecture Constraints

- The gateway remains the sole replay authority.
- A service failure never releases a consumed nonce.
- The replay receipt can use only a previously verified cache entry bound to the same
  capability ID and proof digest.
- The replay path cannot call `razield`, rerun the SP1 verifier, or read transactions.
- Public responses remain generic and disclose no policy branch or private witness.

## File Scope

- `apps/demo-console/app/api/reconciliation/query/`
- `apps/demo-console/components/denial-lab-workspace.tsx`
- `apps/demo-console/lib/portal-client/`
- `apps/demo-console/test/reconciliation-route.test.ts`
- `apps/demo-gateway/src/protected-request/`
- `apps/demo-gateway/test/protected-query.test.ts`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0054-exact-envelope-replay.md`

## Out Of Scope

- A second replay store in `verifierd`
- Free-form request mutation controls
- Receipt browsing, filtering, and export
- Real transaction data
- Mobile layout polish

## Decisions

- Inherited: ADR-0002 fixes the gateway as the sole atomic nonce authority.
- Inherited: UF-11 requires an exact signed-envelope replay, not a newly signed request
  that happens to reuse one field.
- Local implementation choice: the console performs the valid request and exact
  resubmission inside one authenticated server route. It returns only the replay
  outcome to the browser.
- Local implementation choice: a replay receipt is written only when the matching
  public verification cache entry already exists and its signature and claims still
  validate. A cache miss remains a generic rejection without claim-derived fields.

## Open Questions And Blockers

- None. Docker and the SSH ControlMaster are available. Their loss is an external
  prerequisite failure that must be reported before related work continues.

## Work Completed

- Reconciled UF-11 with ADR-0002, FR-085, FR-086, and the existing nonce-first
  gateway order.

## Validation Plan

- Add a service test proving replay rejection uses the same envelope and causes no
  second verification or dataset read.
- Add a console route test proving the two gateway calls carry byte-identical
  envelopes and only the generic replay result reaches the browser.
- Run focused gateway and console test, lint, type, format, and production builds.
- Run `git diff --check` and inspect the replay receipt field set.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Source reconciliation against ADR-0002 and FR-085/FR-086 | PASS | UF-11 replay boundary | 2026-08-30T07:48:58Z |

## Risks And Private-Data Notes

- An arbitrary replay rejection cannot trust claim fields from the submitted
  capability. Receipt creation needs the bound cached verification from the accepted
  first request.
- The console must discard the first request rows during this controlled action. The
  normal Reconciliation path remains the place to display protected results.

## Exact Next Action

Commit archived B-0053 locally, then add a trusted-cache replay receipt path to the
gateway before wiring the two-send console action.
