# Project Baton Protocol

Status: {{ADOPTION_STATUS_AND_AUTHORITY}}

This is the repository's durable work record for humans and agents. It must let a
new contributor find the current objective, safe scope, fixed decisions, missing
approvals, evidence, stop boundary, and next action in five minutes.

## Canonical Records

- `AGENTS.md`: repository contract and required reading order.
- [PROJECT-PROFILE.md](PROJECT-PROFILE.md): authorities, preferences, environment,
  commands, permissions, repo map, and source-of-truth order.
- [CURRENT-BATON.md](CURRENT-BATON.md): the coordinator's bounded active objective.
- [WORK-BOARD.md](WORK-BOARD.md): milestones, priorities, dependent work, assignments.
- [AUTHORIZATIONS.md](AUTHORIZATIONS.md): attributable scoped approvals and revocations.
- `work/`: independently owned assignments, only as needed.
- `history/`: completed, paused, deferred, or superseded checkpoint evidence.
- `../decisions/`: durable proposals and accepted choices, only as needed.

Adopt existing equivalent paths instead of creating competing records. Current
facts outrank historical status claims. Accepted architecture and contracts remain
constraints. Conflicts require reconciliation, not silent reinterpretation.

## States And Ownership

| State | Meaning | Exit |
| --- | --- | --- |
| READY | Bounded objective available, no current owner | Acknowledged claim to ACTIVE |
| ACTIVE | Named owner executes the agreed objective | READY, BLOCKED, REVIEW, PAUSED, or archive |
| BLOCKED | Concrete external fact prevents remaining required work | Resolved to ACTIVE, replan to READY, or explicit PAUSED |
| REVIEW | Named acceptance/review gate awaits a decision | Approved archive/handoff or changes to ACTIVE |
| PAUSED | Human explicitly stopped execution | Human resume to READY or a revalidated ACTIVE claim |

Use review state only for an actual review gate. A routine edit does not require an
independent audit. Mark a partial blocked check without blocking independent scoped
work. Complexity, a failed local test, or an absent optional environment is not alone
a blocker. A human pause is not failure or cancellation.

The active coordinator baton does not use DONE. Archive the final disposition and
create the next READY objective, or record an explicit PAUSED handoff. Deferred work
keeps its incomplete gate on the board. Do not describe it as completed.

One coordinating owner edits the current baton. Other contributors get one scoped
assignment each. The team can work in parallel. A local Markdown edit does not lock
work across clones. Use the profile's acknowledged claim channel before shared work.

IDs use the profile's allocation rule. Default: coordinator-allocated `B-0001` sequence.
Reserve IDs before assignment and never reuse them. Do not independently allocate
the same number in separate clones. Record parent and continuation IDs for deferrals.

## Start Or Resume

1. Read the repository contract, profile, current baton, protocol, relevant accepted
   decisions, then active files/tests. Inspect branch, worktree, and recent commits.
2. Compare claims with files, available commands, approvals, dated evidence, and
   environment state. Mark stale or unavailable facts without deleting unknown work.
3. Confirm the objective is authorized. Claim ownership through the agreed channel.
   Record owner, human sponsor, accepted base revision, workspace, and timestamp.
4. Bound the outcome, file scope, acceptance criteria, constraints, validation,
   permission limits, and next action. Name excluded work and the stop boundary.
5. State the intended edit, then implement the smallest usable slice.

A status-only request does not claim work or resume a pause. Changing this Markdown
does not change an agent platform's scheduler, goal, or process state.

## Work And Approval Loop

Inspect, edit, run a useful narrow check, record the result, and continue toward the
observable outcome. Add broader checks for shared behavior or an existing gate.
Do not substitute test infrastructure or repeated reviews for the requested feature.

Update the baton for meaningful decisions, scope/approval changes, discovered faults,
blockers, acceptance progress, or handoff. Do not keep a transcript of every command.

Use three decision levels:

- Routine in-scope choice: owner decides and records material rationale in the baton.
- Durable repository choice: reviewable ADR with alternatives, impact, and status.
- Protected contract/trust/privacy change: stop the affected path for named approval.

Check authority and prior approval first. Reuse an existing approval only for its
exact scope and conditions. A material revision reopens the affected decision.
Keep rejected and superseded decisions traceable. An agent cannot approve itself.

Task approval does not imply install, publication, deployment, destructive change,
or unlimited retry permission. Follow the profile's standing grants. For missing
dependencies, give exact pin/command, reason, work unlocked, and fallback before install.

Preserve human edits. Show substantive proposed changes as a diff. Apply direct
approved edits without repeating the same approval loop. Do not rewrite accepted
wording, structure, or examples outside the request.

## Parallel Work And Conflict

Assign exclusive write paths, contract revision, base commit, check, and return format.
Keep one owner for shared contracts, lockfiles, generated files, and central records.
Do not let two writers solve overlap by overwriting each other's work.

Across clones, an acknowledged coordinator/issue/PR claim establishes assignment.
Stale timestamps do not grant takeover. Transfer ownership explicitly and end the
old write claim first. If authority or write overlap is unresolved, isolate a branch
and defer integration. Preserve all contributor changes.

Integrate reviewed diffs against their bases. Resolve semantic conflicts with the
contract owner. Validate the combined revision at affected boundaries. Workers'
individual green checks do not establish combined acceptance.

## Evidence And Diagnosis

Use concise records:

| Check/Command | Result | Revision And Environment | Artifact | UTC |
| --- | --- | --- | --- | --- |
| Actual check | PASS / FAIL / SKIP / NOT AVAILABLE | Observed identity | Safe path or none | Timestamp |

PASS means the named check ran and passed. FAIL means it ran and failed. SKIP means
it was intentionally not run, with reason. NOT AVAILABLE means evidence, access, or
the required check could not be obtained. Record observed skipped test counts.
Not applicable is a reason for SKIP, not an invented successful result.

Keep observed facts, hypotheses, and decisions separate. A generic failure is not a
root cause. Record a safe error class, stage, time, affected identity, attempted
diagnostic, and next discriminating check. Do not change tests or contracts just to
make a check pass. Do not print secrets or sensitive payloads to obtain detail.

Record mock, replay, synthetic, local, integration, or live scope. A healthy container
does not prove a user flow. A local passing test does not prove package availability,
deployment, or end-to-end acceptance. Cheap checks can be rerun. Costly/stateful
attempts need a recorded budget and a stopping condition. Do not blindly replay them.

## Block, Pause, And Replan

A blocker record names the fact, owner, exact resource/decision, attempted work,
fallback with cost, and first action after resolution. Continue independent authorized
work inside scope. Do not poll unchanged state endlessly or start unrelated work.

On human pause, record the resumable point and running operations, then stop new work.
Do not cancel a remote operation or delete state without authority. On "finish X,
commit/push, then pause," complete only the named checkpoint and required checks.

For reprioritization, archive the prior objective as DEFERRED or PAUSED, not COMPLETE.
Carry all unfinished criteria to the board and link the continuation. Materially new
acceptance uses a new ID. Preserve decisions, evidence, and unresolved risks, not an
obsolete chain of thought or diary.

## Handoff And Completion

1. Run the agreed focused checks and required affected integration/security gates.
2. Inspect the actual diff, unknown changes, documentation, and private artifacts.
   Run the repo's whitespace/diff check if available. Do not stage secrets or raw logs.
3. Record acceptance results, known gaps, review feedback, approvals, current repos,
   local dirty paths, running work, and exact release state.
4. Obtain any required review. Completion requires the agreed gate, not only the
   implementer's confidence. Routine tasks do not acquire new review gates by default.
5. Commit, push, publish, or deploy only within granted scope. Verify each separately.
6. Archive the outcome with its true disposition, evidence, risks, and follow-ups.
7. Reset current state to the next READY objective or explicit PAUSED boundary. Update
   board and named current-state docs at milestone close. Link durable acceptance.

Do not rewrite dated evidence to resemble today. Remove superseded claims from
current docs and preserve useful rationale in decisions/history. Handoffs must be
usable without private chat, credentials, or a particular agent runtime.

## Recovery

After interruption or context loss, reconcile Git/files, accepted approvals, current
claims, active operations, and the recorded next action. Record contradictions.
Do not erase unknown work, infer a deployment, or retry a live operation from a stale
note. Obtain a coordinator handoff before taking another owner's scope.

## Status Report

State current objective and owner, completed usable flows, actual blocker and evidence,
ordered remaining outcomes, approval needs, release/deployment state, and next action.
Use short plain language. Disclose the date of drift-prone remote observations.
