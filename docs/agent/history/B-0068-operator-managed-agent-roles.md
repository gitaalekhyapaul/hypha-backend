# Current Engineering Baton

- Baton ID: B-0068
- State: REVIEW
- Milestone: M7, operator-managed agent roles
- Owner: Codex
- Started UTC: 2026-10-18 (exact start time not recorded)
- Last updated UTC: 2026-10-18T08:28:00Z
- Branch: `main`
- Worktree: clean baseline `1a297cb4fea1f1a1a736126606b1a1409c323c12`
- Rollback: annotated `v1` tag pushed before feature edits.

## Objective

Add operator-only role and team assignment to Agents. Fresh policy versions can
opt into operator-managed relationships. Issuance reads their existing graph and
never repairs missing memberships. Show missing-role rejection, add-role proof
success, and remove-role rejection on fresh requests. Existing capabilities keep
their current expiry and revocation behavior. The maintainer approved this scope
with "Go ahead then" and "Go ahead please" on October 18.

## Acceptance Criteria

- [x] `v1` rollback tag pushed before feature commits.
- [x] Agents lists, adds, and removes supported direct roles and team memberships.
- [x] Mode changes require a fresh installed approved version with no relationships.
- [x] Owner role writes use the existing exclusive SpiceDB writer and lock.
- [x] Managed issuance captures existing relationships without automatic grants.
- [x] Missing memberships fail the shared evaluator, and exact snapshot retries remain fixed.
- [x] Focused Rust, TypeScript, and controlled browser checks pass.
- [x] ADR, runbook, rollback instructions, and evidence match behavior.

## Architecture Constraints

- No SP1 guest, proof public value, hash, SDK, or agent-repository change.
- No live graph lookup on protected queries and no new revocation promise.
- Role and graph information stays behind operator auth and owner IPC.
- Missing authorization fails before expensive Plonk work, not as a DENY proof.
- Policy/registry authentication and bounded snapshot limits remain fixed.

## File Scope

- Rust: policy publication, provisioning, owner activation IPC/client, focused tests.
- Console: operator role route and adapter, Agents component, approval form, portal client, tests.
- Docs: ADR-0023, README/architecture/spec notes, M7 runbook, baton and history.
- Delegated UI and HTTP work have disjoint files. Only the coordinator edits this baton.

## Out Of Scope

Immediate membership revocation for issued capabilities, external identity systems,
caveated/organization role editing, new dependencies, VM source edits, state migration,
and changes to the public SDK or proof program.

## Decisions

- Fresh, empty policy versions opt into a sticky owner-managed mode. Old versions
  retain automatic grant provisioning. No hidden fallback after activation.
- Direct viewer/approver/auditor/owner and team membership form the editor subset.
- Team membership belongs to a policy-version namespace. Removal affects that
  team's datasets in the version, not just one display filter.
- Role removal affects fresh snapshots. In-flight snapshots and old capabilities
  do not change. Fresh requests are required for the demo's negative controls.
- No package or toolchain installation is approved by this feature task.

## Open Questions And Blockers

- None for implementation. Docker 29.8.2 is available locally.
- Full M7 acceptance still needs two real VM capabilities and reconciliation.
  Earlier notes remain in the pre-role checkpoint archive. Agent A's prior
  READY/query observation does not establish Agent B or the full gate.

## Work Completed

- Read canonical contracts and existing grant, provisioning, and query boundaries.
- Remote tag check found no `v1`. Created and pushed the annotated baseline tag.
- Archived the previous baton without marking its incomplete M7 gate complete.
- Implemented strict operator HTTP -> bounded CLI -> owner IPC role administration.
- Added sticky per-version mode, add/remove controls, and immutable exact-revision capture.
- Approval now hides automatic grant fields for managed versions and blocks on mode lookup errors.
- Real pinned SpiceDB gate passes with empty/add/remove graph capture, exact retry reuse,
  and unchanged legacy behavior. The local shared evaluator gives deny/allow/deny.
- Independent bounded Rust/IPC review found no concrete correctness or security issue.
- Desktop browser checks pass with controlled HTTP fixtures. The temporary memory-only
  SpiceDB test container and network were removed without touching other resources.
- Existing approval fixtures now resolve the new legacy-mode lookup. Their original
  behavioral assertions remain intact. The complete console test suite passes.
- No source or runtime changes were made on the VM. SDK and agent repositories are unchanged.

## Validation Plan

- Rust role operations, malformed/unsupported inputs, bounds, exact revisions/retries.
- Managed provisioning never touches relationships. Legacy provisioning still passes.
- Owner HTTP authentication, CSRF, agent-key binding, bounded process transport.
- Typecheck, focused client/component tests, connected desktop UI check.
- `git diff --check`, staged artifact inspection, remote commit/tag verification.
- No local full Plonk proof. Provide exact VM add/prove/remove/new-request steps.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Git status | PASS, clean baseline | Backend | 2026-10-18 |
| `git push origin refs/tags/v1` | PASS | Rollback tag | 2026-10-18 |
| Docker availability | PASS, 29.8.2 | Local toolchain | 2026-10-18 |
| `cargo test --locked --offline -p raziel-admin --lib` | PASS, 131 passed, 4 ignored | Owner tools | 2026-10-18T08:22:00Z |
| Explicit ignored live provisioning gate against pinned SpiceDB | PASS, 1 test | Managed and legacy graph preparation | 2026-10-18T08:20:00Z |
| `cargo test --locked --offline -p razield --no-default-features --features access-service --lib` | PASS, 34 passed, 17 ignored | Access-service regression | 2026-10-18T08:22:00Z |
| `cargo build --locked --offline -p raziel-admin --bin raziel-policy-activate` | PASS | Real CLI build | 2026-10-18 |
| Console typecheck and scoped Biome check | PASS | Edited TypeScript | 2026-10-18T08:24:00Z |
| Focused role HTTP/client tests | PASS, 134 tests at initial checkpoint | Operator auth, bounded transport, parsing | 2026-10-18T08:22:00Z |
| Initial existing approval-form run | FAIL, resolved by legacy-mode fixture update | 13 fixtures lacked the new lookup; no assertion removed | 2026-10-18T08:24:00Z |
| Existing and new approval-mode tests | PASS, 22 tests | Legacy form, managed form, stale and failed lookup | 2026-10-18 |
| Full console Vitest | PASS, 673 passed, 18 skipped, zero failed; 33 passed files, 3 skipped | Existing optional compiler/live-service gates stay skipped | 2026-10-18 |
| Role editor and existing registration browser flows | PASS | Real Next UI with intercepted synthetic HTTP fixtures; desktop screenshot inspected | 2026-10-18 |
| `cargo fmt -p raziel-admin -- --check` | PASS | Rust formatting | 2026-10-18T08:24:00Z |
| Independent bounded Rust/owner IPC review | PASS, no concrete findings | Managed writes, scope/key binding, retries, approval trust | 2026-10-18 |
| Scoped credential/private-artifact scan | PASS, no matching credentials; no environment or proof files changed | New feature inputs and staged-file review | 2026-10-18 |
| Full real Plonk role demonstration | SKIP | Requires VM run; no local proof claimed | 2026-10-18 |

## Risks And Private-Data Notes

- Owner graph operations must not become agent endpoints or log tuples.
- Team removal affects all datasets using that team in one version.
- Code rollback does not restore edited relationships. Use a fresh demo version.
- Browser checks do not establish successful real VM proof generation.

## Exact Next Action

Archive this implementation checkpoint, commit and push, then hand off B-0069's
VM role demonstration. Use the M7 runbook's Operator Role Assignment Demo section.
The local implementation gate passes. The real-proof and full M7 acceptance gates
remain open and must not be inferred from these tests.
