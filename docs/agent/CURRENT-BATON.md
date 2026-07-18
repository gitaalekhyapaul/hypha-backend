# Current Engineering Baton

- Baton ID: B-0002
- State: ACTIVE
- Milestone: M0 preparation, contract correction
- Owner: Codex
- Started UTC: 2026-07-18T12:03:44Z
- Last updated UTC: 2026-07-18T12:21:24Z
- Branch: `main`
- Worktree: Documentation baseline is staged, with review corrections pending

## Objective

Resolve the documentation contract conflicts found during the baseline review, then
commit one coherent implementation contract before workspace scaffolding starts.

## Acceptance criteria

- [x] Protected request signatures bind the method, canonical target, body, nonce, and
      timestamp in every contract.
- [x] One reconciliation source-manifest schema and package layout is authoritative.
- [x] Reconciliation action names agree across the architecture and scenario contract.
- [x] Ignore rules match the completed-manifest storage policy.
- [x] Architecture invariant copies match the canonical list or refer to it directly.
- [x] Tool wrappers preserve the required reading order.
- [x] Markdown links, code fences, skill metadata, secret patterns, and staged diff
      checks pass.
- [ ] The corrected documentation baseline is committed.

## Architecture constraints

- Preserve all 16 invariants in `AGENTS.md`.
- Keep PolicyIR and private witness material outside agent-visible artifacts.
- Keep policy authorization separate from dataset relevance.
- Treat request-signature changes as a security review trigger.
- Do not add application code or install dependencies in this baton.

## File scope

- `AGENTS.md`
- `ARCHITECTURE.md`
- `CLAUDE.md`
- `.gitignore`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/spec/SCENARIO-AUTHORING.md`
- `docs/decisions/`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/`
- trailing-newline corrections in the staged documentation set

## Out of scope

- Workspace manifests and application shells
- Runtime and dependency installation
- Authorization implementation
- PolicyIR evaluation
- Snapshot or proof implementation
- Real transaction data selection

## Decisions

- Inherited: D-001 through D-016 in `docs/spec/POC-TECHNICAL-SPEC.md`
- Inherited: ADR-0001 in `docs/decisions/0001-baseline-trust-model.md`
- Proposed: ADR-0002 will define the protected-request signature envelope.

## Open questions and blockers

- Maintainer review is requested for the proposed request-signature envelope. The
  baseline proposal binds capability ID, method, canonical target, body hash, nonce,
  and timestamp.
- Mermaid rendering remains unavailable because `mmdc` is not installed. This does not
  block text-level contract correction.

## Work completed

- Read the canonical repository contract, baton protocol, architecture, PoC technical
  specification, scenario contract, wrappers, skill, and decision record.
- Confirmed the worktree contains documentation only.
- Recorded the host tool inventory without installing software.
- Ran an independent read-only documentation review.
- Found request-signature, source-manifest, action-name, ignore-rule, invariant-copy,
  and wrapper-order conflicts.
- Corrected all six conflicts and added proposed ADR-0002.
- Rejected duplicate query keys to prevent parser-order ambiguity in signed requests.

## Validation plan

- Run local Markdown link and code-fence checks.
- Parse the repo skill metadata with Ruby YAML.
- Scan tracked and staged files for common credential patterns.
- Run `git diff --cached --check` after restaging all corrections.
- Review the staged file list and diff summary.
- Commit the corrected baseline, archive this baton, and create the M0 scaffold baton.

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Required contract reading | PASS | Repository documentation | 2026-07-18T12:03:44Z |
| Host tool inventory | PASS, no installs | M0 prerequisites | 2026-07-18T12:03:44Z |
| Independent documentation review | FAIL, six contract conflicts found | Documentation baseline | 2026-07-18T12:03:44Z |
| Local Markdown link scan | PASS before corrections | Documentation baseline | 2026-07-18T12:03:44Z |
| Code-fence balance scan | PASS before corrections | Documentation baseline | 2026-07-18T12:03:44Z |
| Repo skill metadata parse | PASS before corrections | `raziel-scenario` | 2026-07-18T12:03:44Z |
| Secret-pattern scan | PASS before corrections | Repository | 2026-07-18T12:03:44Z |
| `git diff --cached --check` | FAIL, extra blank lines at EOF in ten files | Staged baseline | 2026-07-18T12:03:44Z |
| Contract correction review | FAIL, duplicate query-key ambiguity found | ADR-0002 | 2026-07-18T12:15:12Z |
| Duplicate query-key correction | PASS, duplicate keys now fail closed | ADR-0002 | 2026-07-18T12:15:12Z |
| Local Markdown link scan | PASS | Documentation baseline | 2026-07-18T12:15:12Z |
| Code-fence balance scan | PASS | Documentation baseline | 2026-07-18T12:15:12Z |
| Repo skill metadata parse | PASS | `raziel-scenario` | 2026-07-18T12:15:12Z |
| Secret-pattern scan | PASS | Repository | 2026-07-18T12:15:12Z |
| `git diff --cached --check` | PASS | Staged baseline | 2026-07-18T12:15:12Z |
| Final independent contract review | PASS, no high or medium findings | Corrected baseline | 2026-07-18T12:21:24Z |

## Risks and private-data notes

- An incomplete signature envelope permits cross-route request replay.
- Conflicting source manifests make deterministic ingestion ambiguous.
- Completed local source manifests can contain custody details and must remain ignored.
- No real transaction data, credentials, private keys, witnesses, or proofs are present.

## Exact next action

Commit the corrected documentation baseline with B-0002 active, then record the commit
hash and archive the baton.
