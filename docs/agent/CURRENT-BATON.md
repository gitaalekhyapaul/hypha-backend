# Current Engineering Baton

- Baton ID: B-0059
- State: REVIEW
- Milestone: M5, dataset and capability scope regression
- Owner: Codex
- Started UTC: 2026-08-31T12:58:08Z
- Last updated UTC: 2026-08-31T13:10:08Z
- Branch: `main`
- Worktree: Clean at `529e08d` before claiming this regression. The NUS VM live
  alternative-path job reached `PROVING` and failed before SP1 generation began.

## Objective

Bind each capability request to the dataset published by Access Graph, and reject a
scope-bound caveat whose compiled resource scope differs from that dataset before a
proof job enters BullMQ.

## Acceptance Criteria

- [x] Alternative-path access can publish any valid dataset ID and request a
      capability for that exact dataset.
- [x] The Capability screen displays the same dataset as both dataset and resource
      scope.
- [x] Scope-bound caveats initialize Access Graph from the compiled policy scope and
      reject a conflicting dataset before publication.
- [x] Focused console tests, lint, typecheck, build, and `git diff --check` pass.
- [x] No proof, PolicyIR, snapshot, or backend service semantics change.
- [ ] One fresh NUS VM alternative-path request reaches terminal success with a real
      local Plonk proof.

## Architecture Constraints

- Capability scope must identify the exact dataset resource object ID used by the
  signed relationship snapshot.
- TypeScript may orchestrate public claims but cannot duplicate proof-critical hash
  or PolicyIR evaluation rules.
- The browser receives no relationship tuple, stored caveat value, PolicyIR, Merkle
  opening, or private key.
- M6 real-data source selection remains pending and resumes after this regression.

## File Scope

- `apps/demo-console/components/access-graph-workspace.tsx`
- `apps/demo-console/components/agent-capability-workspace.tsx`
- `apps/demo-console/lib/access-graph/model.ts`
- `apps/demo-console/lib/agent-capability/request.ts`
- `apps/demo-console/test/access-graph-model.test.ts`
- `apps/demo-console/test/agent-capability-request.test.ts`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0058-real-data-source-selection-deferred.md`
- `docs/agent/history/B-0059-dataset-capability-scope-regression.md`

## Out Of Scope

- Generating another local or VM Plonk proof during implementation
- Changing guest, witness, snapshot, PolicyIR, compiler, or capability wire semantics
- Clearing stale SpiceDB relationships during policy-schema changes
- M6 source selection or ingestion

## Decisions

- Inherited: the capability resource scope is the dataset resource object ID used by
  the guest authorization statement.
- Made: Access Graph owns the selected dataset ID. Capability submission copies that
  exact ID instead of the policy builder's hidden default.
- Made: caveats with a compiled `resource_scope` literal fail before publication when
  Access Graph selects a different dataset.

## Open Questions And Blockers

- The implementation is ready. The NUS VM must pull the commit and run one fresh
  alternative-path capability to close the live regression gate.

## Work Completed

- Reproduced two failed alternative-path jobs on the NUS VM.
- Confirmed each worker reached `PROVING`, failed in about eight seconds, stayed below
  3 GiB, and recorded no OOM event. Real Plonk generation never started.
- Confirmed each snapshot dataset differed from the submitted default capability
  scope `reconciliation-demo`.
- Confirmed recreating SpiceDB repaired schema publication but did not repair the
  dataset-to-capability binding.
- Changed capability request construction to bind `resource_scope` to the normalized
  dataset ID retained with the accepted snapshot.
- Added scope-bound caveat initialization and pre-publication mismatch rejection.
- Updated both screens to show one coherent capability scope.

## Validation Plan

- Run focused Access Graph and capability request tests.
- Run the demo-console test suite, lint, and typecheck through the pinned workspace.
- Run `git diff --check` and inspect the final diff for private-data exposure.
- Hand the committed change to the VM for one live alternative-path retry.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| NUS VM job and cgroup inspection | FAIL, scope mismatch before SP1 proving | Live alternative-path capability | 2026-08-31T12:50:00Z |
| Demo-console Vitest suite | PASS, 69 tests | Access Graph, capability request, and console regressions | 2026-08-31T13:09:03Z |
| Demo-console format check | PASS, 103 files | Console source | 2026-08-31T13:09:03Z |
| Demo-console lint | PASS, 102 files | Console source | 2026-08-31T13:09:03Z |
| Demo-console typecheck | PASS | Console types | 2026-08-31T13:09:03Z |
| Demo-console production build | PASS, Next.js 16.2.10 | Portal routes and components | 2026-08-31T13:10:00Z |
| `git diff --check` | PASS | B-0059 worktree | 2026-08-31T13:10:00Z |
| Fresh NUS VM alternative-path proof | NOT AVAILABLE | Awaiting pull and retry | 2026-08-31T13:10:08Z |

## Risks And Private-Data Notes

- A mismatch currently consumes a queue attempt but creates no proof. The fix rejects
  scope-bound caveat mismatches before publication.
- Policy-family switching can still leave SpiceDB relationships that reject a later
  incompatible schema. This is a separate owner-state lifecycle issue.
- Diagnostics used only counts and equality booleans. No relationship tuple or
  private caveat value entered logs or this baton.

## Exact Next Action

Commit and push B-0059. Then pull it on the NUS VM, restart the M5 stack, and run one
fresh alternative-path capability through terminal proof state.
