# Engineering Baton: B-0033

- State at archive: REVIEW
- Milestone: M5, implementation plan and user-flow review
- Owner: Codex
- Started UTC: 2026-08-26T09:54:57Z
- Archived UTC: 2026-08-26T19:36:24Z
- Branch: `main`

## Objective

Write a feature-first M5 implementation plan and complete user-flow list, including a
bounded supported-policy form, then stop for maintainer review before UI work.

## Acceptance Criteria

- [x] One M5 plan orders feature-complete vertical slices from policy setup through
      the proof-backed reconciliation walkthrough.
- [x] The plan defines the bounded policy form, supported rule blocks, compiler and
      authority boundaries, and activation workflow.
- [x] The plan lists operator, agent, allowed, denied, replay, cache, receipt, local,
      and VM user flows.
- [x] The plan defines the screen inventory and the public data each screen can show.
- [x] The plan distinguishes recorded local development from real VM proof success.
- [x] The plan limits tests to visible acceptance behavior and security boundaries.
- [x] Documentation checks pass, and no SDK, gateway, or UI code changes.

## Architecture Constraints

- The policy form emits candidates only. `razield` cannot approve PolicyIR.
- The form can compose only the existing direct, subject-set, union, intersection,
  one-arrow, and caveat subset.
- No recorded or local mode can fake a successful proof or unlock a protected route.
- The agent and browser never receive PolicyIR, private relationships, caveat values,
  satisfying paths, Merkle openings, or salts.
- No package, protocol, proof, or runtime change belongs in this planning baton.

## File Scope

- `docs/spec/M5-IMPLEMENTATION-PLAN.md`
- `docs/agent/CURRENT-BATON.md`

## Out Of Scope

- SDK, gateway, console, compiler, service, or proof implementation
- Package installation or lockfile changes
- M6 real-data ingestion and reconciliation
- A second scenario or a free-form SpiceDB editor
- Public proof values, PolicyIR nodes, snapshot, capability, or verifier semantics

## Decisions

- Inherited: accepted ADR-0002 fixes the protected-request signature input,
  canonical method and target, freshness bounds, and gateway-owned nonce store.
- Inherited: accepted ADR-0014 fixes raw-key Ed25519 agent identity.
- Made: The maintainer selected a bounded visual rule editor using only supported
  policy constructs.
- Made: One selected policy runs through fresh proof generation at a time during the
  video demonstration.
- Approved: The maintainer requested M5 implementation on 2026-08-27 local time.

## Work Completed

- Added `docs/spec/M5-IMPLEMENTATION-PLAN.md` with the connected architecture,
  visual policy builder, six feature slices, eight screens, fourteen user flows,
  local and VM modes, state ownership, dependency gates, and completion gates.
- No package, lockfile, protocol, runtime, SDK, gateway, or UI file changed.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Source-of-truth review against `ARCHITECTURE.md`, the M5 technical specification, and ADR-0013 | PASS | Supported rules, trust boundaries, local recorded mode, and M5 exit gates | 2026-08-26T09:59:17Z |
| `git diff --check` | PASS | Planning documentation | 2026-08-26T09:59:17Z |

## Risks And Private-Data Notes

- A policy form creates a candidate, not an approved authorization policy.
- Policy activation keeps the authority key outside the browser and `razield`.
- Synthetic transaction records remain distinct from SpiceDB relationships and policy
  configuration.

## Handoff

Open B-0034 for the SDK and protected-gateway authorization foundation.
