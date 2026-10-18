# Current Engineering Baton

- Baton ID: B-0073
- State: READY
- Milestone: M7, isolate the failed revision-2 Agent B proof
- Owner: unassigned
- Started UTC: not started
- Last updated UTC: 2026-10-18T12:15:03Z
- Branch: `main`
- Worktree: B-0072 backend and demo-agent checkpoints published to `origin/main`. SDK untouched.
- Rollback: Existing remote `v1` remains at `1a297cb4fea1f1a1a736126606b1a1409c323c12`.

## Objective

Identify the exact failure at the isolated prover boundary for the accepted Agent B
revision-2 request. Keep existing proof, authorization, and privacy semantics intact.

## Acceptance Criteria

- [ ] Review a closed owner-only diagnostic class/stage, without raw child stderr.
- [ ] Capture the exact failure class on one maintainer-approved controlled attempt.
- [ ] Fix only the confirmed fault and validate it without changing proof semantics.
- [ ] Preserve existing tasks, roles, capabilities, keys, and private runtime state.
- [x] Commit/push the separate B-0072 ordering checkpoint.
- [ ] Maintainer deploys B-0072 through the exact M7 runbook commands.
- [ ] Resume the still-open full M7 role and two-agent recording gate.

## Architecture Constraints

Follow ADR-0019, ADR-0023, and the approved guest/key. Agent errors stay generic.
No PolicyIR, tuples, caveat inputs, paths, openings, or witness bytes in logs.
No VM source changes, proof resubmission, or active-job restart without agreement.
Role removal affects fresh snapshots, not existing capabilities.

## File Scope

No diagnostic implementation claimed yet. Diagnostic changes require a bounded scope
and a logging/privacy review before editing. B-0072 release scope and checks remain
in [its archive](history/B-0072-recent-task-and-request-ordering.md).

## Out Of Scope

Changing dataset revision again without evidence, queue-state redesign, guest changes,
new SDK releases, raw private logging, deleting volumes, or claiming M7 complete.

## Decisions

- B-0072 ordering is implemented and locally validated, not deployed.
- Agent checkpoint is `cf96231`; the containing backend commit is its matching checkpoint.
- The VM revision-2 failure is not the earlier revision-1 admission error.
- `SERVICE_UNAVAILABLE` includes rejected witnesses and unavailable runtime state.
  It does not distinguish infrastructure faults from authorization-input rejection.
- Keep the older duplicate-task/revision recovery bug separate and unimplemented.
- The [preserved B-0071 gate](history/2026-10-18-b0071-vm-gate-deferred.md)
  retains all unfinished live acceptance criteria.

## Open Questions And Blockers

The exact closed child/supervisor failure class was not retained. Root cause remains
unconfirmed. SSH ControlMaster and Docker worked during the read-only investigation.

## Work Completed

See [B-0072 ordering](history/B-0072-recent-task-and-request-ordering.md) and
[revision-2 VM diagnosis](../reports/2026-10-18-agent-b-revision-2-failure.md).
Public guest-key, resource profile, setup checksums, policy/snapshot binding,
validity, publication time, and file modes passed their read-only checks.

## Validation Plan

Review closed diagnostic output for private-data safety, add a focused failure-class
test, and capture one controlled VM attempt only after maintainer approval.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0072 implementation checks | PASS | Native agent, real Redis, console, typechecks, formats, fixture browser | 2026-10-18 |
| VM failure, revision 2 | FAIL, generic SERVICE_UNAVAILABLE | 10,847 ms processing interval, exact internal cause unknown | 2026-10-18 |
| VM public runtime checks | PASS | No OOM, 8 CPU, no swap, approved guest, setup checksums, public registry metadata | 2026-10-18 |
| Full two-agent M7 gate | SKIP | Still pending | 2026-10-18 |

## Risks And Private-Data Notes

Do not infer a role denial or infrastructure fault from the generic error. Do not
replay the failed request or expose raw witness state to obtain detail. Retain only
closed diagnostic classes and approved public IDs/timings. No proof ran during checks.

## Exact Next Action

Review the owner-only diagnostic boundary needed to distinguish rejected input,
trusted-state, resource-profile, child startup, transport, and terminal validation.
Do not ask the maintainer to create another task until a diagnostic capture is ready.
