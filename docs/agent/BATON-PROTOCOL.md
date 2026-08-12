# Engineering Baton Protocol

Status: active repository process

This protocol keeps long-running work coherent across Codex, Cursor, Claude, and human
contributors. The baton is a small state record, not a project diary. It identifies
the current objective, file scope, constraints, evidence, and next action.

`docs/agent/CURRENT-BATON.md` is the only active baton.

## Goals

The baton must let a new agent answer these questions in five minutes:

- What is the current milestone?
- What exact outcome is in progress?
- Which files are safe to edit?
- Which decisions are fixed?
- Which questions remain open?
- What tests ran, and what passed?
- What is the next command or edit?
- Which privacy and proof boundaries apply?

## Current-state hygiene

The repository separates current engineering state from dated evidence.

Current-state documents are:

- `docs/agent/CURRENT-BATON.md`
- `ARCHITECTURE.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- the status section in `README.md`
- accepted decision records under `docs/decisions/`

Historical evidence is:

- archived batons under `docs/agent/history/`
- dated reports under `docs/reports/`
- benchmark records under `docs/benchmarks/`
- raw or ignored run logs

Do not rewrite a historical result to resemble the present. Keep its date and result
intact. Do not use a historical file as the source for current milestone status. A
current-state claim must agree with Git, accepted decisions, and durable validation
evidence.

## Baton states

| State | Meaning | Allowed transition |
| --- | --- | --- |
| `READY` | No agent owns the next bounded objective | `ACTIVE` |
| `ACTIVE` | One agent owns one objective | `READY`, `BLOCKED`, `REVIEW` |
| `BLOCKED` | Progress needs a user decision or external state | `ACTIVE`, `READY` |
| `REVIEW` | Implementation is complete and needs independent review | `ACTIVE`, `READY` |

Do not use `DONE` in the active file. Archive completed work, then reset the current
baton to `READY` with the next objective.

## Baton identifiers

Use monotonic identifiers in this format:

```text
B-0001
B-0002
```

One baton identifier covers one bounded objective. A scope change that alters the
acceptance criteria creates a new baton.

## Required fields

The active baton contains:

- baton ID and state
- milestone
- owner or `unassigned`
- start and last-update timestamps in UTC
- branch and observed worktree state
- objective
- acceptance criteria
- architecture constraints
- file scope
- out-of-scope list
- decisions inherited and decisions made
- open questions and blockers
- work completed
- validation plan and evidence
- risks and private-data notes
- exact next action

Do not remove a required field. Use `None` for an empty field.

## Start loop

### Inspect

Run or inspect:

```bash
git status --short
git branch --show-current
git log -5 --oneline
```

Read the canonical files in the order from `AGENTS.md`. Then inspect the files and
tests named by the baton.

### Reconcile state

Compare the baton's claimed state with the repository:

- Check that completed files exist.
- Check that recorded tests match available scripts.
- Check for user changes outside the listed file scope.
- Check whether an open decision now has an answer.
- Check whether the next action is still valid.

Record any mismatch before editing. Preserve unknown work.

### Claim

Set:

- state to `ACTIVE`
- owner to the active assistant or human label
- start or resume timestamp
- one objective
- bounded file scope
- acceptance criteria
- planned validation

Do not claim a second objective. Split independent work into separate batons or use
read-only specialist reviews.

## Work loop

Use this cycle until the objective passes its gate:

1. Inspect the smallest relevant surface.
2. State the intended file change.
3. Make one coherent edit.
4. Run the narrowest useful check.
5. Record the result.
6. Recheck the objective and architecture constraints.
7. Continue or hand off.

Update the baton after:

- an architecture or protocol decision
- a failed check that changes the plan
- a dependency or data blocker
- a change to public proof values
- a change to file scope
- completion of an acceptance criterion
- discovery of user work that affects the task

Do not update the baton for every minor edit.

## Validation evidence format

Record concise evidence in a table:

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `cargo test -p policy-ir` | PASS, 34 tests | PolicyIR | 2026-07-16T10:00:00Z |
| `pnpm typecheck` | FAIL, missing generated client | Workspace | 2026-07-16T10:02:00Z |
| Not run | SP1 toolchain not installed | Guest | 2026-07-16T10:03:00Z |

Do not paste full logs into the baton. Link to a durable artifact or quote the first
useful error line.

Allowed result labels:

- `PASS`
- `FAIL`
- `SKIP`
- `NOT AVAILABLE`

Never convert `SKIP` into `PASS`.

## Decision handling

Use three decision levels.

### Local implementation choice

Record it in the baton. Examples include a local helper name or a test file split.

### Durable repository choice

Create `docs/decisions/NNNN-short-title.md`. Link it from the baton and any affected
specification. Examples include a signing algorithm, token representation, or public
proof-value change.

### Architecture invariant change

Stop implementation. Draft a decision record that lists:

- proposed change
- reason
- trust-boundary effect
- public and private data effect
- migration cost
- test changes
- rejected options

Wait for explicit maintainer approval.

## Blocked state

Set `BLOCKED` only for a concrete external need, such as:

- missing user approval to install a required dependency
- no approved real dataset or usage basis
- a required key or service is unavailable
- conflicting protocol decisions need maintainer selection

The baton must include:

- the blocking fact
- the exact decision or resource needed
- work already attempted
- a fallback and its cost
- the first action after resolution

Keep working on independent, in-scope tasks. Do not mark the whole baton blocked for a
single optional check.

## Handoff loop

### Verify

Run the checks in the baton's validation plan. Include:

- focused tests
- affected cross-component tests
- format and static checks
- `git diff --check`
- secret and private-artifact checks for security-sensitive work

For protocol changes, run mutation and cross-language vector tests.

### Review the diff

Check:

- no unrelated file changed
- no architecture invariant changed silently
- no private witness field became public or logged
- docs and contracts match behavior
- generated files came from the documented command
- test-only fixtures are clearly marked

### Archive

Copy the completed active baton to:

```text
docs/agent/history/B-0001-short-objective.md
```

The archive must retain the final evidence, decisions, risks, and follow-up work.

### Reset

Rewrite `CURRENT-BATON.md` with:

- the next baton ID
- state `READY`
- owner `unassigned`
- next milestone and objective
- inherited constraints
- known repository state
- one exact next action

Do not leave the active baton describing completed work as if it remains active.

### Reconcile milestone status

A milestone close is incomplete until all current-state documents agree. The
coordinating owner must:

1. Link the durable exit-gate evidence from the archived baton.
2. Mark the milestone status in `docs/spec/POC-TECHNICAL-SPEC.md`.
3. Update the status and diagrams in `ARCHITECTURE.md`.
4. Update the status section in `README.md`.
5. Reset `CURRENT-BATON.md` to the next bounded objective.
6. Search current-state and undated documentation for superseded milestone claims.

Exclude dated reports, archived batons, and benchmark records from the stale-claim
search. They preserve the state that existed at their recorded time.

## Parallel work

Use parallel agents for independent reads, audits, or separate file groups. The active
baton names one coordinating owner.

Each writing agent needs:

- exclusive file scope
- its own validation command
- a short result returned to the coordinating owner

Two agents must not edit `CURRENT-BATON.md` at the same time. The coordinating owner
merges evidence and updates the baton.

## Recovery from stale baton state

When code and baton disagree:

1. Trust Git and file contents for observed state.
2. Preserve all unknown changes.
3. Mark stale claims as `unverified`.
4. Rerun cheap checks.
5. Update the baton with corrected facts.
6. Record any protocol or acceptance ambiguity as an open question.

Remove superseded claims from current-state documents after recording the correction.
Preserve durable rationale, failed experiments, and risk evidence in an archived baton
or decision record. Add a correction note only when the stale claim affected a risk,
approval, or acceptance decision. Do not retain obsolete planning or abandoned
thought process in the active baton.

## Privacy checks for every handoff

Answer each item with `PASS`, `FAIL`, or `NOT APPLICABLE`:

- Agent responses contain no PolicyIR.
- Agent responses contain no relationship tuple or path.
- Agent responses contain no Merkle opening.
- Logs contain no caveat secrets or witness material.
- Capability claims reveal only approved public fields.
- Verifier reads policy from its own registry.
- Verifier completes without a prover connection.
- Real data remains outside Git and generated output.

## Current baton template

```markdown
# Current Engineering Baton

- Baton ID: B-0000
- State: READY
- Milestone: M0
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: YYYY-MM-DDTHH:MM:SSZ
- Branch: main
- Worktree: Describe observed state

## Objective

One bounded outcome.

## Acceptance criteria

- [ ] Observable criterion

## Architecture constraints

- Relevant invariant

## File scope

- `path/`

## Out of scope

- Explicit exclusion

## Decisions

- Inherited: `D-000`
- Made: None

## Open questions and blockers

- None

## Work completed

- None

## Validation plan

- Command or check

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Not run | NOT AVAILABLE | None | YYYY-MM-DDTHH:MM:SSZ |

## Risks and private-data notes

- Relevant note

## Exact next action

One command or edit.
```
