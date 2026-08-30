# Current Engineering Baton

- Baton ID: B-0053
- State: REVIEW
- Milestone: M5, controlled scope denial
- Owner: Codex
- Started UTC: 2026-08-30T07:04:52Z
- Last updated UTC: 2026-08-30T07:48:58Z
- Branch: `main`
- Worktree: B-0052 is committed locally as `b73b6e8`. B-0053 is implemented and
  awaits final diff inspection and a scoped local commit.

## Objective

Add one explicit desktop action that sends a correctly signed request outside the
capability dataset scope. The gateway must deny it without returning rows or policy
details and must append a bounded public rejected receipt.

## Acceptance Criteria

- [x] Denial Lab offers one clearly labeled out-of-scope denial action after a live
      capability succeeds.
- [x] The console signs a fresh request for a controlled synthetic dataset outside
      the live capability scope.
- [x] The request reaches the gateway and passes shape, freshness, nonce, and agent
      signature checks before scope enforcement rejects it.
- [x] The browser receives one generic authorization failure and no transaction rows,
      policy path, relationships, caveat values, or witness data.
- [x] The gateway appends one public rejected receipt with a coarse
      `PUBLIC_ERROR_CODE_NOT_AUTHORIZED` reason.
- [x] The accepted first-use and cache-hit query paths remain unchanged.
- [x] Focused gateway and console checks pass.

## Architecture Constraints

- Gateway denials cannot identify the failing relationship, policy node, or caveat.
- A rejected request cannot read the transaction store.
- The denial receipt contains public claim identifiers and digests only.
- The console cannot forge a success response or evaluate PolicyIR locally.
- No new dependency, proof field, capability field, or policy semantic is added.

## File Scope

- `apps/demo-console/app/api/reconciliation/query/`
- `apps/demo-console/components/denial-lab-workspace.tsx`
- `apps/demo-console/components/operator-portal.tsx`
- `apps/demo-console/lib/portal-client/`
- `apps/demo-console/test/reconciliation-route.test.ts`
- `apps/demo-gateway/src/protected-request/`
- `apps/demo-gateway/test/protected-query.test.ts`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0053-controlled-scope-denial.md`

## Out Of Scope

- Exact replay, expiry, alteration, and unsigned-request controls
- Receipt history, filtering, and export
- Real transaction data
- Mobile layout polish

## Decisions

- Inherited: FR-089 requires generic public denials.
- Inherited: FR-100 requires receipts for protected requests.
- Local implementation choice: the controlled target uses a fixed safe synthetic
  dataset identifier. It is not a free-form request editor.
- Local implementation choice: a rejected receipt is written only after the gateway
  has trusted verified claims and a valid agent signature. Earlier malformed input
  remains a generic rejection without claim-derived receipt fields.

## Open Questions And Blockers

- None. Docker and the SSH ControlMaster are available. Their loss is an external
  prerequisite failure that must be reported before related work continues.

## Work Completed

- Reconciled UF-10 with FR-087, FR-089, FR-100, and the existing generic route error.
- Added an authenticated Denial Lab action that uses the live server-held agent key
  and a fresh nonce to sign one fixed out-of-scope dataset request.
- Added an exact browser contract for the in-scope or controlled out-of-scope mode.
  The mode does not enter the signed gateway body.
- Added the gateway rejected-receipt path after proof and signature verification but
  before the transaction store.
- Kept the denied target out of the receipt. The receipt records approved public
  capability claims and one coarse not-authorized reason.
- Updated the M5 runbook with the UF-09 reuse and UF-10 denial walkthrough.

## Validation Plan

- Add focused service tests proving valid signature and nonce processing precede the
  scope denial, no dataset read occurs, and one coarse receipt is appended.
- Add console route and presentation checks for the controlled denial action.
- Run focused gateway and console test, lint, type, format, and production builds.
- Run `git diff --check` and inspect the denied response and receipt field set.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Source reconciliation against FR-087, FR-089, and FR-100 | PASS | UF-10 boundary | 2026-08-30T07:04:52Z |
| Demo-gateway test, lint, and typecheck | PASS, 19 tests | Scope denial order, no dataset read, rejected receipt | 2026-08-30T07:12:05Z |
| Demo-console test, lint, and typecheck | PASS, 56 tests | Signed controlled target and generic browser denial | 2026-08-30T07:12:05Z |
| Demo-gateway and demo-console production builds | PASS | Next.js route and Denial Lab compilation | 2026-08-30T07:12:42Z |
| Live Compose `config --quiet` | PASS | M5 deployment graph | 2026-08-30T07:13:00Z |
| `git diff --check` | PASS | UF-10 worktree | 2026-08-30T07:48:58Z |

## Risks And Private-Data Notes

- A denial receipt cannot include the requested out-of-scope identifier if that value
  is not an approved public claim. It records the capability public scope and a coarse
  denial reason.
- Receipt failures must fail closed. No transaction read can occur first.

## Exact Next Action

Commit B-0053 locally, then implement B-0054 exact-envelope replay rejection. Do not
push until the maintainer approves a VM handoff.
