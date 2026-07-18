# B-0002: Documentation Contract Corrections

- Final state: completed
- Milestone: M0 preparation
- Owner: Codex
- Started UTC: 2026-07-18T12:03:44Z
- Closed UTC: 2026-07-18T12:24:33Z
- Branch: `main`
- Commit: `2e4b61b03ee6bb033fe1294612915cfb4b12879a`

## Objective

Resolve the documentation contract conflicts found during the baseline review, then
commit one coherent implementation contract before workspace scaffolding starts.

## Acceptance criteria

- [x] Protected request signatures bind the method, canonical target, body, nonce, and
      timestamp in every contract.
- [x] One reconciliation source-manifest schema and package layout is authoritative.
- [x] Reconciliation action names agree across the architecture and scenario contract.
- [x] Ignore rules match the completed-manifest storage policy.
- [x] Architecture invariant copies refer to the canonical 16-item list.
- [x] Tool wrappers preserve the required reading order.
- [x] Markdown links, code fences, skill metadata, secret patterns, and staged diff
      checks pass.
- [x] The corrected documentation baseline is committed.

## Architecture constraints

- Preserved all 16 invariants in `AGENTS.md`.
- Kept PolicyIR and private witness material outside agent-visible artifacts.
- Kept policy authorization separate from dataset relevance.
- Recorded the request-signature change as a security review item in ADR-0002.
- Added no application code and installed no dependencies.

## Files changed

- `ARCHITECTURE.md`
- `CLAUDE.md`
- `.gitignore`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/spec/SCENARIO-AUTHORING.md`
- `docs/decisions/0002-protected-request-signature-envelope.md`
- staged documentation files with terminal blank-line corrections
- `docs/agent/CURRENT-BATON.md`

## Decisions

- Inherited: D-001 through D-016 in `docs/spec/POC-TECHNICAL-SPEC.md`
- Inherited: ADR-0001 in `docs/decisions/0001-baseline-trust-model.md`
- Proposed for maintainer review: ADR-0002 defines the protected-request signature
  envelope and rejects duplicate query keys.

## Work completed

- Bound protected request signatures to capability ID, method, canonical target, body
  hash, nonce, and timestamp.
- Required agent-key and request-signature checks on cold and cached requests.
- Unified the source manifest schema and scenario package layout.
- Moved completed source manifests to an ignored local path.
- Aligned the first reconciliation action on `query` and resource type on `dataset`.
- Replaced the shortened architecture invariant copy with a reference to the canonical
  16-item list.
- Corrected the Claude wrapper reading order.
- Removed extra terminal blank lines from eight staged files.

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Required contract reading | PASS | Repository documentation | 2026-07-18T12:03:44Z |
| Host tool inventory | PASS, no installs | M0 prerequisites | 2026-07-18T12:03:44Z |
| Initial independent review | FAIL, six conflicts found | Documentation baseline | 2026-07-18T12:03:44Z |
| First correction review | FAIL, duplicate query-key ambiguity found | ADR-0002 | 2026-07-18T12:15:12Z |
| Duplicate query-key correction | PASS, duplicate keys fail closed | ADR-0002 | 2026-07-18T12:15:12Z |
| Local Markdown link scan | PASS | Documentation baseline | 2026-07-18T12:15:12Z |
| Code-fence balance scan | PASS | Documentation baseline | 2026-07-18T12:15:12Z |
| Repo skill metadata parse | PASS | `raziel-scenario` | 2026-07-18T12:15:12Z |
| Secret-pattern scan | PASS | Repository | 2026-07-18T12:15:12Z |
| `git diff --cached --check` | PASS | Staged baseline | 2026-07-18T12:21:24Z |
| Final independent contract review | PASS, no high or medium findings | Corrected baseline | 2026-07-18T12:21:24Z |
| Documentation commit | PASS, `2e4b61b` | Corrected baseline | 2026-07-18T12:24:33Z |

## Privacy review

- Agent responses contain no PolicyIR: PASS.
- Agent responses contain no relationship tuple or path: PASS.
- Agent responses contain no Merkle opening: PASS.
- Logs contain no caveat secrets or witness material: PASS.
- Capability claims reveal only approved public fields: PASS at contract level.
- Verifier reads policy from its own registry: PASS at contract level.
- Verifier completes without a prover connection: NOT APPLICABLE before M4.
- Real data remains outside Git and generated output: PASS.

## Remaining risks

- ADR-0002 still needs maintainer approval before gateway integration.
- Mermaid rendering remains unchecked because `mmdc` is absent.
- M0 toolchain pins and workspace shells remain incomplete.
- The real transaction source remains open as O-001.

## Handoff

Continue with B-0003 in `docs/agent/CURRENT-BATON.md`. Close O-002, request approval
for missing toolchain installations, then build the M0 workspace shells.
