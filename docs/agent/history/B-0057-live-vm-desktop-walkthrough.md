# Current Engineering Baton

- Baton ID: B-0057
- State: REVIEW
- Milestone: M5, live VM desktop walkthrough
- Owner: Codex
- Started UTC: 2026-08-30T08:45:14Z
- Last updated UTC: 2026-08-30T20:42:29Z
- Branch: `main`
- Worktree: The M5 implementation and evidence are committed and pushed at
  `c5264ff`. The NUS VM ran that commit for the final regression.

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
- [x] Sidebar navigation retains approved policy state without another activation.
- [x] Duplicate policy activation returns a typed HTTP `409` conflict.

## Architecture Constraints

- The NUS VM remains a deployment target, not a source-editing worktree.
- Source changes happen locally and require maintainer approval before each push.
- The live flow uses the networkless 60 GiB Plonk sandbox and independent verifier.
- The walkthrough remains labeled synthetic and cannot satisfy M6.
- Docker and the SSH ControlMaster are user-managed prerequisites.

## File Scope

- Read-only VM deployment and runtime logs
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/images/m5-live/`
- `README.md`
- `ARCHITECTURE.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
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
- Made: unlocked portal panels stay mounted for the page session so navigation keeps
  their public review state.
- Made: fixed Rust activation conflict and rejection messages map to their existing
  typed HTTP failure codes.
- Made: the maintainer performs VM pulls. The VM PAT requires interactive decryption.

## Open Questions And Blockers

- None for M5. O-001 remains the M6 data-source blocker.

## Work Completed

- B-0051 through B-0056 implemented the complete synthetic M5 desktop feature set.
- The VM ran the live stack from commit `8700537` through the Mac SSH tunnel.
- One Plonk capability succeeded after 937,078 ms and produced a 2,536-byte proof.
- Verifier and cache queries returned three synthetic rows with separate receipts.
- All five Denial Lab controls returned no protected rows.
- Exact receipt filtering returned the two linked records, and JSONL export
  downloaded.
- Fourteen live screenshots cover the public panel states.
- Browser navigation exposed two presentation-state defects. Unmounting panels
  discarded their public review state, and typed activation conflicts became false
  `503` errors.
- Commit `c5264ff` keeps unlocked panels mounted and preserves typed activation
  failures at the HTTP boundary.
- The VM ran `c5264ff`. Navigation retained the approved policy record, hash, and
  registry generation. Duplicate activation returned HTTP `409` with the trusted
  registry conflict message.
- The [M5 portal runbook](../../runbooks/M5-LOCAL-PORTAL.md) and current-state
  documents record M5 completion and the M6 synthetic-versus-real-data boundary.

## Validation Plan

- Verify every screenshot exists and renders.
- Check current-state milestone claims for stale M5 text.
- Run demo-console tests, format, lint, typecheck, and production build.
- Run `git diff --check` and inspect the scoped diff.
- Scan the new documentation and images for private witness data.
- Deploy the committed fix to the VM and run the navigation and conflict regressions.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0056 demo-console tests, lint, typecheck, and build | PASS | Final local M5 desktop feature controls | 2026-08-30T08:45:14Z |
| Docker Compose running-service check | PASS | Local recorded M5 stack | 2026-08-30T08:42:19Z |
| Runbook reconciliation against the M5 plan and current panel source | PASS | Eight panels and UF-01 through UF-14 | 2026-08-30T08:58:22Z |
| Live VM browser walkthrough | PASS | UF-01 through UF-14, one real Plonk proof | 2026-08-30T19:33:58Z |
| Exact request-ID receipt filter and JSONL download | PASS | Two linked public records and export | 2026-08-30T20:13:25Z |
| Browser console warning and error check | PASS, no entries | Final live receipt state | 2026-08-30T20:13:25Z |
| Live conflict diagnosis through the fixed local client boundary | PASS | Typed trusted-state conflict found | 2026-08-30T20:22:00Z |
| `pnpm --filter @raziel/demo-console test` in pinned toolchain | PASS, 65 tests | Demo console and typed conflict regression | 2026-08-30T20:26:41Z |
| Demo-console format, lint, typecheck, and production build | PASS | Scoped M5 navigation and error mapping fix | 2026-08-30T20:29:54Z |
| PNG format and visual inspection | PASS, 14 images | Live public panel evidence | 2026-08-30T20:32:13Z |
| Current-state stale-claim search and scoped private-data scan | PASS | M5 completion documents | 2026-08-30T20:32:13Z |
| `git diff --check` | PASS | Complete B-0057 implementation diff | 2026-08-30T20:32:13Z |
| VM deployment of `c5264ff` | PASS | Recreated demo console served on port 3000 | 2026-08-30T20:38:38Z |
| Sidebar Dataset-to-Policy navigation | PASS | Approved record, hash, and registry generation retained | 2026-08-30T20:39:56Z |
| Duplicate policy activation | PASS | HTTP `409` and trusted registry state message | 2026-08-30T20:40:51Z |
| Exact request-ID receipt filter after regression | PASS, 2 records | Public receipt projection | 2026-08-30T20:42:00Z |
| Browser console regression review | PASS | Only expected sign-in `401` and conflict `409`, with no warnings | 2026-08-30T20:42:29Z |

## Privacy Checks

- PASS: Agent responses contain no PolicyIR.
- PASS: Agent responses contain no relationship tuple or satisfying path.
- PASS: Agent responses contain no Merkle opening.
- PASS: Logs and checked-in evidence contain no caveat secrets or witness material.
- PASS: Capability claims expose only approved public fields.
- PASS: The verifier reads policy from its own trusted registry.
- PASS: The verifier completed without a prover connection.
- NOT APPLICABLE: M5 used labeled synthetic data. No real transaction data entered
  Git or generated output.

## Risks And Private-Data Notes

- A live proof takes about 16 minutes and peaks near 47 GiB on the accepted VM
  profile.
- VM logs must not contain private relationship tuples, caveat values, Merkle
  openings, witness traces, raw capability bytes, or authority keys.
- A recorded-mode walkthrough cannot validate protected queries or Denial Lab.
- This M5 result remains synthetic. It does not satisfy the M6 acceptance gate.

## Exact Next Action

Select and approve the M6 real transaction source, usage basis, and provenance record
under O-001 before any ingestion adapter work begins.
