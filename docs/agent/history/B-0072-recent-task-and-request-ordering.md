# Current Engineering Baton

- Baton ID: B-0072
- State: READY
- Milestone: M7, recent task and request ordering
- Owner: Codex
- Started UTC: 2026-10-18T10:37:34Z
- Last updated UTC: 2026-10-18T12:15:03Z
- Branch: `main`
- Worktree: Backend and demo-agent repositories clean at start.
- Rollback: Existing remote `v1` remains at `1a297cb4fea1f1a1a736126606b1a1409c323c12`.

## Objective

Show recently updated tasks first in both agent dashboards and the operator access
inbox, with creation time as fallback and deterministic ties across request pages.

## Acceptance Criteria

- [x] New and changed agent tasks appear first, including existing persisted tasks.
- [x] Unchanged status polling does not refresh local task activity timestamps.
- [x] Agent selection survives task reordering and refreshed DOM nodes move correctly.
- [x] Operator ordering applies before pagination and preserves agent ownership filters.
- [x] Focused checks pass and deployment instructions identify both repositories.

## Architecture Constraints

No proof, signing, SDK, authorization, or queue-state semantics change. Only existing
public task/request metadata determines order. Preserve private runtime state.

## File Scope

- `services/razield/src/access_request/{redis_store,service,tests}.rs`
- `apps/demo-console/components/access-request-workspace.tsx`
- Sibling `raziel-demo-agents/src/{access-tasks,runtime}.ts`, `public/app.js`, focused tests.
- Current baton, history, Agent API list documentation, M7 runbook.

## Out Of Scope

The earlier Agent B duplicate-task/revision recovery issue, LLM transport, new SDK
release, real proof runs, VM state changes, role semantics, and queue transitions.

## Decisions

- Keep the existing Redis identity index and queue writes unchanged.
- Read retained metadata in bounded batches and retain only the next page candidates.
- Scope the opaque recency cursor to the existing owner filter.
- Preserve optional local timestamps for old agent state without a reset or migration.

## Open Questions And Blockers

Sibling filesystem edits were approved. No dependencies were installed.
The full VM gate remains pending in the preserved B-0071 record.

## Work Completed

Implemented recent-first operator ordering before pagination, with owner-scoped
timestamp/identity cursors and deterministic ties. Read metadata in batches of 100
and retain only page candidates. Existing Redis keys and state transitions stay intact.
The console reorders its current page after detail updates and shows activity time.

Agent tasks retain optional millisecond activity timestamps. Unchanged patches do not
bump activity. Existing state falls back to backend request times and incoming message
times, then reverse saved order for unknown/tied history. Existing DOM nodes move
without replacing task selection. Native and browser checks passed.

The separate maintainer-requested VM investigation is recorded in
[revision-2 failure evidence](../../reports/2026-10-18-agent-b-revision-2-failure.md).
It changed no remote source, state, service, or proof job.

## Validation Plan

Focused agent timestamp/order tests, agent typecheck and formatting, access-service
tests with a disposable Redis, console checks, scoped diff and privacy review.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Worktree inspection | PASS | Backend and agents clean | 2026-10-18T10:37:34Z |
| Docker availability | PASS, 29.8.2 | Local only | 2026-10-18T10:37:34Z |
| Agent native suite | PASS, 15 tests | Includes persistence, old-state fallback, unchanged polls | 2026-10-18 |
| Agent TypeScript | PASS | Installed compiler | 2026-10-18 |
| Agent formatter | PASS, 4 edited files | Existing Linux Biome 2.5.4 in pinned Docker, no download | 2026-10-18 |
| Access-service library | PASS, 34 tests, 17 ignored | Offline pinned toolchain | 2026-10-18 |
| Access-request integration | PASS, 11 tests, no ignored | Disposable real Redis, ordering across pages, ties, update promotion, owner isolation | 2026-10-18 |
| Console suite | PASS, 673 tests, 18 skipped | Pinned Docker toolchain | 2026-10-18 |
| Console TypeScript and scoped Biome | PASS | Updated inbox | 2026-10-18 |
| Rust formatting and both diff checks | PASS | Scoped files | 2026-10-18 |
| Playwright desktop refresh | PASS | Controlled HTTP fixture, real HTML/JS/CSS, task nodes reordered and selection retained | 2026-10-18 |
| Live VM sorting deployment | SKIP | Local implementation only | 2026-10-18 |
| Failed VM proof diagnosis | NOT AVAILABLE, exact cause | Generic failure retained, no closed stage/class audit | 2026-10-18 |

## Risks And Private-Data Notes

Operator ordering scans retained public metadata, not witnesses or capability bytes.
Keep bounded memory for page candidates. This PoC tradeoff avoids a new persistent
index migration, but list work grows with retained request count. No VM job restart.

## Exact Next Action

Deploy the backend and sibling agent checkpoints using the exact recent-ordering
steps in the M7 runbook. No SDK release or guest rebuild. Keep VM failure diagnosis
separate from ordering. The known Agent B duplicate-task/revision recovery fix is
still unimplemented.

## Validation Notes

An initial agent test patch contained invalid diff markers and was corrected.
An initial local Redis test used an unreachable Docker network and was stopped.
The rerun against the correctly networked disposable Redis passed. Host Biome lacked
its macOS binary, so the existing pinned Linux formatter ran offline in Docker.
The temporary Redis and Playwright page were removed. No new tool was installed.

## Privacy Review

PASS: no PolicyIR, tuples, paths, Merkle openings, caveat inputs, credentials, or
witness bytes were added to responses or logs. Existing approved public claims and
verifier independence stay unchanged. No proof semantics or stateful mutation changed.

## Checkpoint State

Implementation and validation complete. The maintainer requested VM pull commands.
Agent checkpoint: `cf96231` on `main`. The containing backend commit records the
matching release and deployment steps. Both checkpoints are published to their
respective `origin/main` branches before handoff. VM deployment remains pending.
SDK remains untouched. Existing rollback tag `v1` was not moved.
