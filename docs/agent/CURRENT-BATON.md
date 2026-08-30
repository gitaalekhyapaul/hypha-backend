# Current Engineering Baton

- Baton ID: B-0057
- State: ACTIVE
- Milestone: M5, live VM desktop walkthrough
- Owner: Codex
- Started UTC: 2026-08-30T08:45:14Z
- Last updated UTC: 2026-08-30T20:32:13Z
- Branch: `main`
- Worktree: `main` and the VM ran commit `8700537`. One scoped navigation and typed
  error fix, M5 status documents, the expanded runbook, and live screenshots are
  uncommitted locally.

## Objective

Run and document the complete M5 desktop feature path on the NUS VM in live Plonk
mode. Record one successful proof-backed query, cache reuse, all five Denial Lab
controls, public receipt export, and screenshots of the verified panel states.

## Acceptance Criteria

- [x] The VM pulls the reviewed B-0051 through B-0056 commits from `origin/main`.
- [x] `./scripts/start-m5-demo` starts the live stack from Docker-only host tooling.
- [x] Dataset, policy, access graph, and capability flows complete through one real
      Plonk proof.
- [x] Reconciliation returns one verifier-backed result and one cache-backed result.
- [x] Scope, replay, expired, altered, and unsigned controls return no protected rows.
- [x] Receipts browse, filter, and export the validated public projection.
- [x] Runtime findings and exact evidence are recorded without private witness data.
- [x] The runbook maps every panel, UF-01 through UF-14, unlock prerequisite, and
      recorded-versus-live boundary.
- [x] The runbook includes screenshots captured from the passing live VM flow.

## Architecture Constraints

- The NUS VM remains a deployment target, not a source-editing worktree.
- Source changes happen locally and require maintainer approval before each push.
- The live flow uses the networkless 60 GiB Plonk sandbox and independent verifier.
- The walkthrough remains labeled synthetic and cannot satisfy M6.
- If Docker or the SSH ControlMaster becomes unavailable, stop and notify the
  maintainer with the failed prerequisite and blocked step.

## File Scope

- Read-only VM deployment and runtime logs
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0057-live-vm-desktop-walkthrough.md`
- `apps/demo-console/components/operator-portal.tsx`
- `apps/demo-console/lib/policy-activation/process.ts`
- `apps/demo-console/test/policy-activation-route.test.ts`

## Out Of Scope

- Direct source edits on the VM
- M6 real transaction data
- Mobile layout polish
- New proof, policy, capability, or receipt semantics

## Decisions

- Inherited: `scripts/start-m5-demo` is the supported entry point and defaults to live
  mode.
- Inherited: the browser forwards only port 3000 from the VM.
- Inherited: each source fix is made and checked locally before an approved push.

## Open Questions And Blockers

- Maintainer approval is required before pushing the scoped runtime fix and M5
  evidence commit to `origin/main` for the final VM navigation regression.

## Work Completed

- B-0051 through B-0056 implement the complete synthetic M5 desktop feature set.
- The VM ran the live stack from commit `8700537` through the Mac SSH tunnel.
- One Plonk capability succeeded after 937,078 ms and produced a 2,536-byte proof.
- Verifier and cache queries returned three synthetic rows with separate receipts.
- All five Denial Lab controls returned no protected rows.
- Exact receipt filtering returned the two linked records, and JSONL export
  downloaded.
- Fourteen live screenshots now cover the public panel states.
- Browser navigation exposed two presentation-state defects. Unmounting panels
  discarded their public review state, and typed activation conflicts became false
  `503` errors.
- The local fix keeps unlocked panels mounted for the page session and maps fixed
  Rust conflict and rejection messages to their existing HTTP failure codes.
- The runbook and current-status documents now record M5 completion and the M6
  synthetic-versus-real-data boundary.

## Validation Plan

- Verify every screenshot exists and renders.
- Check all current-state milestone claims for stale M5 text.
- Run `git diff --check` and inspect the scoped diff.
- Run privacy and secret scans over the new documentation and images.
- After an approved push, pull the commit on the VM and verify that sidebar navigation
  retains the active public records without another activation request.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0056 demo-console tests, lint, typecheck, and build | PASS | Final local M5 desktop feature controls | 2026-08-30T08:45:14Z |
| Docker Compose running-service check | PASS | Local recorded M5 stack | 2026-08-30T08:42:19Z |
| Runbook reconciliation against the M5 plan and current panel source | PASS | Eight panels and UF-01 through UF-14 | 2026-08-30T08:58:22Z |
| Live VM browser walkthrough | PASS | UF-01 through UF-14, one real Plonk proof | 2026-08-30T19:33:58Z |
| Exact request-ID receipt filter and JSONL download | PASS | Two linked public records and export | 2026-08-30T20:13:25Z |
| Browser console warning and error check | PASS, no entries | Final live receipt state | 2026-08-30T20:13:25Z |
| Live conflict diagnosis through the fixed local client boundary | PASS, typed trusted-state conflict found | Policy reactivation after panel remount | 2026-08-30T20:22:00Z |
| `pnpm --filter @raziel/demo-console test` in pinned toolchain | PASS, 65 tests | Demo console and typed conflict regression | 2026-08-30T20:26:41Z |
| Demo-console format, lint, typecheck, and production build | PASS | Scoped M5 navigation and error mapping fix | 2026-08-30T20:29:54Z |
| PNG format and visual inspection | PASS, 14 images | Live public panel evidence | 2026-08-30T20:32:13Z |
| Current-state stale-claim search and scoped private-data scan | PASS | M5 completion documents | 2026-08-30T20:32:13Z |
| `git diff --check` | PASS | Complete uncommitted B-0057 diff | 2026-08-30T20:32:13Z |

## Risks And Private-Data Notes

- A live proof takes about 16 minutes and peaks near 47 GiB on the accepted VM
  profile.
- VM logs must not contain private relationship tuples, caveat values, Merkle
  openings, witness traces, raw capability bytes, or authority keys.
- A recorded-mode walkthrough cannot validate protected queries or Denial Lab.

## Exact Next Action

Finish the documentation, image, privacy, and diff checks. Commit locally, request
push approval, then run the scoped VM navigation regression before archiving B-0057.
