# Collaboration Across Agents, People, And Repositories

## Authority And Ownership

Name the human sponsor, decision authorities, current coordinator, contributors,
reviewer, and release operator in the project profile. One person can hold several
roles. Do not invent separation of duties for a small PoC. A team can require an
independent reviewer for security-sensitive work.

An agent belongs to a human sponsor and execution session. Record both names plus
a session/task reference. A friendly agent label is not proof of human identity.
Do not imply that signed commits or Markdown alone establish organizational authority.
Use the team's authenticated review channel for disputed or high-risk approvals.

The coordinator alone updates `CURRENT-BATON.md`, allocates sequential baton IDs,
and merges evidence into the program board. This is one coordinating record, not
a ban on parallel implementation. Each contributor has one bounded active assignment
in `work/`, with an exclusive write scope and a clear parent objective.

## Claim Before Editing

For one coordinated checkout, the coordinator grants assignments before workers
edit. Workers do not acquire ownership by racing to change the same Markdown file.
Shared contract, generated output, lockfile, migration, or root configuration paths
need one designated owner. Read-only reviewers need no write claim.

For independent clones or contributors from different people, use an acknowledged
claim through the team's existing issue, PR, or coordinator channel. Record the
accepted base commit and scope revision. A private branch or local edit of the
current baton is a claim proposal, not a globally accepted lock. Start only after
coordinator acknowledgement. Without such a channel, serialize shared writes and
ask the human to coordinate. Do not install a lock service to solve a small task.

The coordinator uses a single accepted base revision to merge competing claims.
Reject stale updates and reconcile before ownership changes. If the team already has
an atomic compare-and-set issue/lock system, record its receipt. Do not claim that a
normal Markdown file provides compare-and-set behavior.

Record: work ID, objective, owner and sponsor, parent, claim revision, base commit,
repository/branch/workspace, exclusive paths, acceptance check, approval scope,
heartbeat expectations, and expiry policy if the team actually uses one.

A stale timestamp alone does not permit takeover. Attempt contact, inspect committed
and uncommitted state through permitted access, then obtain coordinator transfer.
The former owner's write authority must end before the new owner starts. If that
cannot be confirmed, isolate work in a branch and defer integration. Never delete
another person's workspace, locks, processes, or dirty files to force progress.

## Delegate Only Independent Slices

Good slices: separate code areas, source reviews, bounded research questions,
independent contract review, or a consumer change with a fixed interface.
Do not delegate a tiny edit or the next blocking step just to create activity.

An assignment contains enough context to work without the parent's private chat:
objective, canonical references, agreed contract, exact scope, exclusions, evidence,
authority, permitted tools/environments, cost limits, and return format.
Delegation does not grant new installs, remote access, spending, publication, or
permission to message unrelated humans or tasks.

Workers return changed files, commit or patch identity, checks with outcomes,
assumptions, open risks, approval needs, dirty paths, running operations, and the
next action. A reviewer finding is evidence, not permission to merge or deploy.

## Integrate Without Lost Updates

Review the worker's actual diff against the agreed base. Confirm its scope and
approval revision. Integrate through the existing branch/PR workflow. Do not
cherry-pick over unresolved changes or automatically resolve a semantic conflict.
After all slices meet, run the affected contract or integration check once on the
combined revision. Independent green tests do not prove the combined build works.

Keep one owner for shared edits. A necessary scope change requires an acknowledged
handoff, not two simultaneous writers. A contradictory invariant or acceptance gate
stops only the affected path. Record the issue and ask the authorized human.

## Multiple Repositories

Choose one program hub for the global objective, board, shared decisions, and
authorization references. Each repo keeps local instructions, worktree evidence,
and scoped work records. A local baton names its parent program item and accepted
contract revision. Dated handoff snapshots are references, not competing authorities.

Record compatibility at the checkpoint:

| Repository Or Package | Branch | Commit Or Version | Check | Pushed/Published | Deployed |
| --- | --- | --- | --- | --- | --- |
| Each actual component | Observed | Exact identity | Result/environment | Evidence or pending | Evidence or pending |

Plan release order from dependency direction. Establish the contract, validate the
producer and consumer, publish only with authority, then update the consumer's exact
pin. A local package archive or workspace link is a temporary development mode, not
evidence that public consumers can install it. Keep credentials out of all repos.

Cross-repo changes are not an atomic transaction. If a producer is published and a
consumer push fails, record the partial release, keep safe compatibility, and stop
at the agreed boundary. Do not claim program completion or roll back an immutable
public release by rewriting history.

## Interruptions And Handoffs

Release or transfer ownership explicitly. Preserve the current diff, unresolved
decisions, human edits, pending approvals, live-job IDs in approved storage, and
last verified environment state. Never transfer private credentials in a baton.

A replacement coordinator reconciles the accepted board revision, claims, Git, and
approvals before issuing assignments. Takeover requires the project's authority rule.
The old coordinator stops writing the central baton. Conflicting coordinator claims
require a human decision, not agent self-election.

If the host offers persistent goals or automatic continuations, follow its controls
separately. Updating a Markdown state neither pauses nor resumes a scheduler. Never
create a goal, timer, recurring task, or new chat solely to make the protocol work.
