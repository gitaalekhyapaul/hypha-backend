# Current Engineering Baton

- Baton ID: B-0062
- State: READY (planning completed and archived)
- Milestone: M6-M7 scope and implementation planning
- Owner: Codex
- Started UTC: 2026-09-25T07:11:50Z
- Last updated UTC: 2026-09-25T07:22:18Z
- Branch: `main`
- Worktree: clean at `26b92a7` before planning edits. Both sibling repositories exist.

## Objective

Record the maintainer's revised SDK and two-agent reconciliation scope, publish one
cross-repository implementation plan, and reconcile current roadmap documentation.

## Acceptance Criteria

- [x] ADR-0019 records the decisions and their authorization in this conversation.
- [x] The plan covers three dashboards, external SDK consumption, registration,
      operator review, request-triggered snapshots, serial proving, reuse, and messages.
- [x] OpenAI-compatible inference and the selected evidence reply are explicit.
- [x] Current roadmap documents defer real data without claiming that gate passed.
- [x] B-0061 remains archived as unstarted, deferred work.
- [x] Documentation links and whitespace checks pass.

## Architecture Constraints

- The maintainer approved synthetic demo data in place of the former M6 source gate.
- Proof generation remains real local Plonk with one proof executor at a time.
- No change to guest, public proof values, PolicyIR, or canonical commitments.
- Policy publication is independent of request-triggered snapshot formation.
- Agents own their private keys. The operator registers a public key and agent token.
- `verifierd` remains independent. Agent responses contain no private witness material.

## File Scope

- `AGENTS.md`, `README.md`, and `ARCHITECTURE.md`
- `docs/decisions/0019-public-sdk-and-two-agent-reconciliation.md`
- `docs/spec/M6-M7-AGENT-DEMO-PLAN.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/spec/SCENARIO-AUTHORING.md`
- `docs/spec/M5-IMPLEMENTATION-PLAN.md` (supersession note only)
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0061-real-data-source-selection-deferred.md`
- `docs/agent/history/B-0062-agent-demo-planning.md`

## Out Of Scope

- Runtime code, dependency installation, proof execution, and deployment
- SDK publication, remote pushes, and edits to sibling repositories
- Real-data acquisition, a second domain scenario, and guest changes

## Decisions

- Made: use an OpenAI-compatible API configured by endpoint, model, and secret.
- Made: share a bounded reconciliation result and selected synthetic evidence fields.
- Inherited: the maintainer approved three repos, three dashboards, synthetic records,
  per-agent tokens bound to registered public keys, and sequential live proofs.

## Open Questions And Blockers

- No remaining product question blocks planning.
- Sibling filesystem write access is not granted to this task yet.
- Endpoint, model, API secret, and npm publishing credentials are deployment inputs.
- Exact new authentication and access-request wire contracts need a concrete design
  before implementation. They are not covered by the existing capability signature.

## Work Completed

- Added accepted scope ADR-0019 and a plan with 15 user flows and six feature steps.
- Reconciled AGENTS, README, architecture, specification, and scenario guidance.
- Preserved the previous real-data goal in the deferred B-0061 archive.
- Added a supersession note to the dated M5 plan without rewriting its original scope.
- No runtime code, remote repository, dependency, or deployment changed.
- Read current architecture, specification, ADR-0018, scenario contract and skill.
- Confirmed M0-M5 status and the old real-data gate in B-0061.
- Checked sibling repositories and preserved the SDK repo's untracked `.DS_Store`.
- Confirmed the current one-distinct-job admission rule and worker expiry recheck.

## Validation Plan

- Review the plan against every maintainer answer and the existing API/SDK surface.
- Check new relative Markdown links and document consistency.
- Run `git diff --check`. Runtime tests are not needed for documentation-only changes.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Repository and contract inspection | PASS | Existing M5 baseline and SDK gaps | 2026-09-25T07:11:50Z |
| Runtime tests | SKIP | Documentation-only work | 2026-09-25T07:11:50Z |
| Relative Markdown links and plan JSON | PASS | 46 file targets and one JSON example | 2026-09-25T07:22:18Z |
| git diff --check | PASS | Documentation whitespace | 2026-09-25T07:22:18Z |
| Maintainer-answer and state consistency review | PASS | Scope, 15 flows, six steps, planned status | 2026-09-25T07:22:18Z |
| Privacy review | PASS | Documentation only, no credentials or witness artifacts | 2026-09-25T07:22:18Z |

## Risks And Private-Data Notes

- New API metadata must not expose PolicyIR, relationships, caveats, or openings.
- A reconciliation result is business evidence, not a proof of reconciliation accuracy.
- Old milestone references in dated evidence keep their original meaning.
- No credentials, raw private data, or proof artifacts are in scope.

## Exact Next Action

B-0063 defines the concrete agent API signatures, request states, and policy provisioning migration.
