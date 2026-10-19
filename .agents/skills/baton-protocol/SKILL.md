---
name: baton-protocol
description: Set up, operate, or repair a repository baton protocol for long-running projects, human approvals, multi-agent and multi-person collaboration, review checkpoints, and durable handoffs. Use for adopting the framework or managing an existing baton, not as an extra process for unrelated one-off tasks.
metadata:
  short-description: Portable project batons and human approval loops
---

# Baton Protocol

Make the next safe action clear to a new contributor without relying on chat memory.
Use one coherent project record, bounded work ownership, attributable approvals,
and evidence that distinguishes local progress from delivery.

This skill is plain Markdown. A human, CLI agent, editor assistant, or hosted agent
can follow it. No particular model, plugin, task API, paid service, or scheduler is
required. Optional `agents/openai.yaml` metadata is not part of the protocol.

## Choose The Mode

- **Set up a repo:** read [setup](references/setup.md), then adapt the assets it names.
- **Continue, pause, or hand off:** read the target repo's instructions, profile,
  current baton, protocol, and relevant architecture before editing.
- **Coordinate contributors or repos:** read [collaboration](references/collaboration.md).
- **Request approval, review edits, or deliver:** read [human loop](references/human-loop.md).
- **Repair stale records:** compare Git, files, approvals, and dated evidence. Correct
  the current record, preserve history, and name unresolved contradictions.

Prefer the target repo's adopted protocol over this starter. Do not silently migrate
its state machine, identifiers, gates, or authority model. Propose material process
changes for review. A status request alone calls for inspection and a report.

## Operating Rules

1. Read current canonical instructions and inspect worktree changes first. Historical
   batons explain past work, not today's acceptance or authorization state.
2. Claim one bounded objective per owner. One project coordinator maintains the
   shared current baton. Parallel contributors use separate work records and scopes.
3. Record the outcome, acceptance criteria, constraints, file scope, validation,
   approvals, and one exact next action before substantive work.
4. Treat approval of the objective, authority to approve, and tool permissions as
   separate facts. A collaborator's agent cannot approve its own privilege expansion.
5. Preserve human edits and unknown changes. Show substantive proposals as diffs.
   Apply requested edits within scope without repeatedly requesting the same approval.
6. Build a usable slice, then test its actual risks. Do not replace requested features
   with test infrastructure, speculative hardening, or a broad audit.
7. Stop at the agreed boundary. A request to commit and pause does not authorize
   the next feature, deployment, package publication, or a costly live retry.
8. Record exact evidence and environment. Local tests, mock checks, remote pushes,
   registry publication, deployment, and live acceptance are different milestones.
9. Keep secrets, private data, credentials, and raw sensitive logs out of all shared
   records. Link to access-controlled evidence and retain a safe failure class.
10. Archive completed or deferred checkpoints with their true disposition. Keep
    unfinished acceptance visible. Never archive a failure as a completed milestone.

## Starter Resources

The setup reference maps these assets to their repository destinations:

- [Protocol](assets/BATON-PROTOCOL.md): states, work loop, evidence, recovery, handoff.
- [Project profile](assets/PROJECT-PROFILE.md): authorities, workflow preferences,
  commands, environments, repository map, and release permissions.
- [Current baton](assets/CURRENT-BATON.md): one coordinating objective and next action.
- [Work board](assets/WORK-BOARD.md): milestones, priorities, dependencies, ownership.
- [Work item](assets/WORK-ITEM.md): delegated assignment or archived checkpoint.
- [Authorization ledger](assets/AUTHORIZATIONS.md): scoped approval and revocation.
- [Decision record](assets/DECISION.md): durable choices needing a reviewable rationale.
- [Repository contract excerpt](assets/REPO-CONTRACT.md): append or merge into existing
  instructions, never overwrite them.
- [Delivery runbook](assets/DELIVERY-RUNBOOK.md): separate host/repo commands, checks,
  secrets setup, stop conditions, and rollback ownership.

These assets are templates, not approved project state. Replace `{{...}}` fields
from evidence. Leave an unknown authority or command explicitly unresolved rather
than inventing a person, approval, endpoint, or successful result. Create only files
needed for the chosen workflow. Small projects do not need empty delegation records.

## Completion Check

For setup, a new agent must find the objective, owner, safe scope, existing approvals,
validation, stop boundary, and next action from the repo alone. Confirm links and
commands exist. Disclose unresolved configuration. Setup does not close product gates.

For continuing work, report what changed, checks and gaps, decision needs, repo
release state, and one next action. Never turn an unverified claim into `PASS`.
Use the target repo's communication style. The included profile carries Gita's
defaults as editable preferences, not universal rules for every maintainer.
