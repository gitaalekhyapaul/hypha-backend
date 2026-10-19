# B-0076 Portable Baton Protocol Skill

- Disposition: COMPLETE, local authoring and personal installation
- Owner: Codex coordinator
- Started UTC: 2026-10-19T15:25:26Z
- Completed UTC: 2026-10-19T16:00:45Z
- Branch/baseline: main, application commit `257c7b2`
- Commit/push: NOT PERFORMED, not requested for this authoring task
- App/VM/SDK/agent repositories: unchanged
- Rollback tag: unchanged

## Objective And Outcome

Created a portable, agent-neutral baton framework using the existing repository
protocol and the maintainer's documented working preferences. One concise skill
routes to three focused references and nine adaptable repository assets.

All requested local-authoring criteria are satisfied:

- A new agent can establish the framework without Raziel context.
- Preferences are configurable and separate from project authority and safety gates.
- Multi-person/multi-agent claims require acknowledgement, not a Markdown race.
- Approval evidence, ADRs, incremental edits, pause/resume, validation, and release
  stages remain distinct.
- An independent agent created a realistic setup in an isolated temporary repo.
- The shareable source is installed in the personal skills folder and packaged.
- Unfinished Agent B work remains visible in the current continuation baton B-0077.

## Files And Scope

- `.agents/skills/baton-protocol/SKILL.md`, 86-line entry.
- `references/setup.md`, `references/human-loop.md`, `references/collaboration.md`.
- `agents/openai.yaml`, optional UI metadata, no runtime dependency.
- Nine assets: protocol, profile, current baton, work board, work item,
  authorization ledger, decision record, repository contract excerpt, delivery runbook.
- `docs/agent/CURRENT-BATON.md` and the B-0075 deferral/history records.
- Installed copy: `~/.codex/skills/baton-protocol/`, filesystem permission granted.
- Shareable archive: `/tmp/baton-protocol-skill.tar.gz`.

No existing Raziel protocol, architecture, product contract, source, or live state changed.
No new package, runtime, plugin, hook, background goal, or scheduler was installed.

## Decisions

- One skill avoids overlapping triggers and duplicated process rules.
- Plain Markdown is canonical. Tools and vendor metadata are optional adapters.
- Use Gita's working defaults in an editable project profile, not universal mandates.
- Setup approval does not confer installation, publication, deployment, or retry rights.
- Distinguish design authority, human attribution, execution scope, and tool permissions.
- Shared current state has one coordinator. Independent contributors have disjoint
  assignments and acknowledged claims, with explicit transfer and integration.
- No mandatory audit, ADR, full test suite, commit, or remote action for every small edit.

## Validation Evidence

| Check | Result | Scope | UTC |
| --- | --- | --- | --- |
| Supplied quick_validate.py, system and bundled Python | NOT AVAILABLE | yaml/PyYAML missing, no install attempted | 2026-10-19 |
| Existing Ruby/Psych YAML checks | PASS | Skill frontmatter and UI metadata, naming and prompt checks | 2026-10-19 |
| Link, ASCII, final newline, and whitespace checks | PASS | All 14 skill files, 13 instruction links | 2026-10-19 |
| Scoped secret/project-pin scan | PASS | No copied credentials, private state, deployment endpoints, or Raziel pins found by targeted scan | 2026-10-19 |
| Independent setup exercise | PASS | Fresh Harbor repo, framework setup only, 34 local links, no unresolved template fields | 2026-10-19 |
| Fixture preservation | PASS | Four full-file hashes and original AGENTS prefix unchanged, human copy exact | 2026-10-19 |
| Isolated fixture test | PASS | Initial/final runs each 1 passed, 0 failed/skipped | 2026-10-19 |
| Collaboration/approval/pause scenarios | PASS, scenario responses | No private-branch lock assumption, no install under design-only approval, no status-triggered resume | 2026-10-19 |
| Installed copy comparison | PASS | Recursive equality with source skill | 2026-10-19 |
| Archive listing and digest | PASS | 14 source files, portable relative paths | 2026-10-19 |
| Git diff check | PASS | Tracked changes, plus separate untracked-skill checks | 2026-10-19 |
| App tests/live deployment | SKIP | Documentation-only task, no application change | 2026-10-19 |

The independent tester's first untracked-whitespace wrapper misread Git's expected
difference exit code as a failure. The corrected check passed. No file defect or
product failure was inferred from that wrapper error.

## Forward-Test Finding And Correction

The initial template did not spell out bootstrap authorship before a named coordinator
had acknowledged ownership. Added a setup-only exception: the editor may draft central
records under the direct setup request, must distinguish proposed coordinator from
objective owner, keep work unclaimed, and use SETUP-DRAFT until an allocator is available.
No implementation claim or identity acknowledgement is invented.

The independent exercise evaluated the pre-clarification bootstrap behavior. The
clarification received static checks and source/installation equality checks, not a
second full forward test. Approval receipt format remains deliberately channel-neutral.
The tester preserved a safe source summary rather than claiming cryptographic identity.

## Artifact And Release State

Archive SHA-256:
`6e3186e61e7bec1a8c435b20e87a7a990bfd0b9935877e108bc1e418b1718e56`

The source skill and baton records are uncommitted. Personal installation and archive
creation are complete. No Git push, public publication, VM deployment, proof retry,
or app acceptance is claimed. The isolated test repo remains under
`/tmp/baton-protocol-forward.DtN0LI` as disposable non-sensitive evidence.

## Exact Next Action

Provide the installed skill entry and shareable archive to the maintainer.
Application work remains unstarted under B-0077 until separately resumed.
