# Current Engineering Baton

- Baton ID: B-0053
- State: ACTIVE
- Milestone: M5, controlled scope denial
- Owner: Codex
- Started UTC: 2026-08-30T07:04:52Z
- Last updated UTC: 2026-08-30T07:04:52Z
- Branch: `main`
- Worktree: B-0052 is archived. Its UF-09 cache implementation awaits a scoped local
  commit. No B-0053 source edits have started.

## Objective

Add one explicit desktop action that sends a correctly signed request outside the
capability dataset scope. The gateway must deny it without returning rows or policy
details and must append a bounded public rejected receipt.

## Acceptance Criteria

- [ ] The Reconciliation view offers one clearly labeled out-of-scope denial action.
- [ ] The console signs a fresh request for a controlled synthetic dataset outside
      the live capability scope.
- [ ] The request reaches the gateway and passes shape, freshness, nonce, and agent
      signature checks before scope enforcement rejects it.
- [ ] The browser receives one generic authorization failure and no transaction rows,
      policy path, relationships, caveat values, or witness data.
- [ ] The gateway appends one public rejected receipt with a coarse
      `PUBLIC_ERROR_CODE_NOT_AUTHORIZED` reason.
- [ ] The accepted first-use and cache-hit query paths remain unchanged.
- [ ] Focused gateway and console checks pass.

## Architecture Constraints

- Gateway denials cannot identify the failing relationship, policy node, or caveat.
- A rejected request cannot read the transaction store.
- The denial receipt contains public claim identifiers and digests only.
- The console cannot forge a success response or evaluate PolicyIR locally.
- No new dependency, proof field, capability field, or policy semantic is added.

## File Scope

- `apps/demo-console/app/api/reconciliation/query/`
- `apps/demo-console/components/reconciliation-workspace.tsx`
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

## Risks And Private-Data Notes

- A denial receipt cannot include the requested out-of-scope identifier if that value
  is not an approved public claim. It records the capability public scope and a coarse
  denial reason.
- Receipt failures must fail closed. No transaction read can occur first.

## Exact Next Action

Commit archived B-0052 locally, then add a typed denied receipt path to the gateway
service before wiring the console action.
