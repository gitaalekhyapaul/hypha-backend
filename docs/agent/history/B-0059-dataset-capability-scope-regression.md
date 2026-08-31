# Dataset And Capability Scope Regression

- Baton ID: B-0059
- Final state: REVIEW, acceptance complete
- Milestone: M5, dataset and capability scope regression
- Owner: Codex
- Started UTC: 2026-08-31T12:58:08Z
- Completed UTC: 2026-08-31T13:58:12Z
- Branch: `main`
- Commit: `bb58440`

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
- [x] No proof, PolicyIR, snapshot, or backend service semantics changed.
- [x] A fresh NUS VM alternative-path request reached terminal success with a real
      local Plonk proof.

## Decisions

- Access Graph owns the selected dataset ID. Capability submission copies that exact
  ID instead of the policy builder's hidden default.
- Caveats with a compiled `resource_scope` literal fail before publication when
  Access Graph selects a different dataset.

## Work Completed

- Reproduced the dataset-to-capability scope mismatch on the NUS VM.
- Bound `resource_scope` to the normalized dataset retained with the accepted
  snapshot.
- Added scope-bound caveat initialization and pre-publication mismatch rejection.
- Updated both screens to show one coherent capability scope.
- Proved the corrected alternative-path request through the live M5 stack.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Demo-console Vitest suite | PASS, 69 tests | Access Graph and capability regressions | 2026-08-31T13:09:03Z |
| Demo-console format, lint, and typecheck | PASS | Console source | 2026-08-31T13:09:03Z |
| Demo-console production build | PASS, Next.js 16.2.10 | Portal routes and components | 2026-08-31T13:10:00Z |
| `git diff --check` | PASS | B-0059 worktree | 2026-08-31T13:10:00Z |
| Fresh NUS VM alternative-path proof | PASS, 2,537-byte Plonk proof | Live M5 service flow | 2026-08-31T13:36:38Z |

The live proof executed in 425 ms and proved in 938,301 ms. Peak proof-container
memory was 48,355,037,184 bytes. The worker recorded no out-of-memory event.

## Risks And Private-Data Notes

- The successful live run exposed no relationship tuple, caveat value, witness,
  Merkle opening, private key, or proof input.
- The first HTTP call returned a generic unavailable result after the job entered
  BullMQ. A retry signed a second request. B-0060 owns that separate lifecycle defect.
- M6 real-data source selection remains deferred in B-0058.

## Follow-Up

B-0060 adds stable M5 submission retries and rejects a distinct request while one
proof job is nonterminal.
