# B-0001: Repository Documentation Contract

- Final state: completed
- Milestone: M0 preparation
- Owner: Codex
- Closed UTC: 2026-07-16T14:13:04Z
- Branch: `main`

## Objective

Create the implementation handoff, cross-tool agent instructions, baton process, and
scenario-authoring contract for the Raziel payment-reconciliation PoC.

## Delivered

- `ARCHITECTURE.md` with a human overview, diagrams, stack choices, trust boundaries,
  and detailed agent implementation rules
- `docs/spec/POC-TECHNICAL-SPEC.md` with protocol, service, PolicyIR, capability,
  milestone, and acceptance requirements
- canonical `AGENTS.md` plus `AGENT.md`, `CLAUDE.md`, and Cursor wrappers
- baton state machine, active M0 baton, and history process
- ADR-0001 for the verifier-visible PolicyIR baseline
- scenario authoring contract with real-data and effort gates
- repo-local `raziel-scenario` skill and planning template
- README and ignore rules for the planned polyglot workspace

## Decisions

- Keep one canonical instruction source in `AGENTS.md`.
- Keep Cursor and Claude files as thin pointers.
- Keep policy approval outside `razield` and bind snapshots to an approved `policy_hash`.
- Make payment reconciliation the only acceptance scenario before M6 passes.
- Require real, provenance-recorded data for acceptance.
- Treat new proof semantics as research work, not scenario extension.

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `git diff --check` | PASS | Repository diff | 2026-07-16T14:13:04Z |
| Local Markdown link scan | PASS | 15 files before archive creation | 2026-07-16T14:13:04Z |
| Code-fence balance scan | PASS | 15 files before archive creation | 2026-07-16T14:13:04Z |
| Skill metadata checks reproduced with Ruby YAML | PASS | `raziel-scenario` | 2026-07-16T14:13:04Z |
| Official skill `quick_validate.py` | NOT AVAILABLE, Python `yaml` module missing | `raziel-scenario` | 2026-07-16T14:13:04Z |
| Mermaid renderer check | NOT AVAILABLE, `mmdc` missing | Architecture diagrams | 2026-07-16T14:13:04Z |
| Application build and tests | NOT AVAILABLE, no application scaffold | Workspace | 2026-07-16T14:13:04Z |

## Privacy review

- Agent-visible PolicyIR: PASS, prohibited by architecture and agent rules.
- Agent-visible relationship witness: PASS, prohibited.
- Verifier policy source: PASS, local registry is required.
- Verifier independence: documented, not executable before M4.
- Real transaction data in Git: PASS, none added.

## Remaining risks

- Toolchain versions need selection during B-0002.
- Mermaid blocks need renderer validation once an approved tool is available.
- Cross-language contracts and tests do not exist yet.
- The real transaction source needs selection before scenario adapter work.

## Handoff

Continue with B-0002 in `docs/agent/CURRENT-BATON.md`. Inspect installed tools, then
propose pinned versions and bootstrap commands before installing dependencies.
