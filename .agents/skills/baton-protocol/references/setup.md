# Establish The Framework

## Inspect And Classify

Read existing repository instructions, current work records, architecture, decisions,
roadmap, and relevant delivery documents. Inspect the branch and dirty files without
resetting them. Inventory linked repositories and existing issue or PR systems.

Classify the request as new setup, incremental adoption, or protocol repair. A mature
repo with a working protocol normally needs a small correction, not replacement.
Do not copy this starter over an active baton or another author's instructions.

Establish these facts from the user or existing records:

- Project outcome, immediate milestone, and explicit non-goals.
- Human sponsor and authority for design, security, dependencies, and releases.
- Coordinator, contributors, and whether they share a checkout or use separate clones.
- Canonical repository or workspace hub, source-of-truth order, and protected contracts.
- Validation commands already available and environments that can run them.
- Review, commit, push, publication, deployment, and pause boundaries.

Ask only material unresolved questions. Offer defaults for low-risk naming and paths.
Do not stall local documentation work for an optional remote environment. Ask before
installing tools or adding external services. No new dependency is needed for this
Markdown framework.

## Adapt The Assets

Use the existing doc locations where possible. Suggested new-repo layout:

```text
AGENTS.md
ARCHITECTURE.md                     existing or genuinely needed project architecture
docs/
  agent/
    PROJECT-PROFILE.md
    BATON-PROTOCOL.md
    CURRENT-BATON.md
    WORK-BOARD.md
    AUTHORIZATIONS.md
    work/                          only for delegated or independently owned work
    history/                       dated, sanitized checkpoints
  decisions/                       create records as durable decisions arise
  runbooks/                        create for actual operations
```

| Asset | Destination Or Use |
| --- | --- |
| `assets/PROJECT-PROFILE.md` | `docs/agent/PROJECT-PROFILE.md` |
| `assets/BATON-PROTOCOL.md` | `docs/agent/BATON-PROTOCOL.md` |
| `assets/CURRENT-BATON.md` | `docs/agent/CURRENT-BATON.md` |
| `assets/WORK-BOARD.md` | `docs/agent/WORK-BOARD.md` |
| `assets/AUTHORIZATIONS.md` | `docs/agent/AUTHORIZATIONS.md` |
| `assets/WORK-ITEM.md` | Per assignment in `work/`, or final checkpoint in `history/` |
| `assets/DECISION.md` | A numbered proposal under `docs/decisions/`, only as needed |
| `assets/REPO-CONTRACT.md` | Merge its relevant section into existing `AGENTS.md` |
| `assets/DELIVERY-RUNBOOK.md` | Actual scoped operation under `docs/runbooks/` |

Do not create empty ADRs, fake architecture, placeholder test scripts, or cosmetic
directories. Omit runbooks until actual commands are known. Resolve all template
fields in adopted current-state files, or label each unresolved fact and its owner.
Paths inside assets target the suggested final layout, not the skill asset folder.
Adapt links if the repo uses different destinations.

Build the project profile first. Keep the team authority rules separate from one
person's preferences. Gita's defaults are provided, not silently imposed on another
team. Retain existing repository safety gates and review requirements.

Add a small pointer to the canonical protocol in existing agent wrappers only where
the tool needs one. Keep substantive instructions in `AGENTS.md` and repository docs.
Do not install hooks, enable autonomous runs, or alter agent settings as part of setup.

## Populate Real Work

Convert the accepted roadmap into observable feature or delivery outcomes. Record
priority, dependencies, acceptance evidence, owner, and current status on the board.
Keep a feature checklist distinct from implementation subtasks. Record deferred work
explicitly without making it block the agreed deliverable.

Populate one next bounded objective. A time or turn estimate is a range with clear
assumptions, not an acceptance criterion. State what must work at the next checkpoint.
Do not invent a deadline or start every queued item.

For a multi-repo project, select one program hub and adopt the collaboration reference.
Sibling repos contain their own local instructions plus a pointer to the shared
program record. Do not create several competing global current batons.

Record existing approvals only from attributable evidence. Proposed architecture
remains proposed. Setup approval is not approval of every populated roadmap item.

## Bootstrap Without An Existing Coordinator Claim

A direct request to create the framework permits the setup editor to draft its
central records within that request. It does not make that editor the coordinator
or imply acknowledgement from a separately named coordinator. Label the editor and
proposed coordinator separately. Keep the first record READY and unclaimed, with a
temporary `SETUP-DRAFT` label if no authorized allocator is available. Reserve a real
work ID only through the agreed allocator. Do not invent a base commit in an unborn repo.

The exact next action is coordinator review and acknowledgement of the setup and
claim channel. Product work remains queued. A coordinator named by the human can
adopt the draft through the accepted channel, allocate the ID, then grant scopes.
Record this bootstrap exception in the new profile so later agents do not reuse it
to bypass normal claims. An existing active protocol never gets this exception.

## Verify And Hand Off

Run a five-minute newcomer check using only repo files:

- What outcome is next, and what is explicitly excluded?
- Who owns it, and which files and environment can they change?
- Which decisions are fixed, and which approval is still missing?
- Which check establishes completion, and which evidence is still local only?
- Where must work stop, and what is the next exact action?

Check relative links in their final destinations. Validate every listed command
against repository files. Do not run a deployment just to validate runbook wording.
Review the documentation diff and preserve all unknown worktree changes.

Deliver the framework file map, selected defaults, unresolved decisions, current
objective, and release state. State whether files are merely written, committed,
pushed, or installed. Creating a framework does not grant permission to commit it.
