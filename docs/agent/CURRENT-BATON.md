# Current Engineering Baton

- Baton ID: B-0006
- State: READY
- Milestone: M1, PolicyIR and SpiceDB compiler adapter
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-19T17:34:36Z
- Branch: `main`
- Expected repository state: B-0005 compiler complete. Verify HEAD and worktree on claim.

## Objective

Implement a live equivalence harness that runs approved authorization cases through
the pinned SpiceDB `CheckPermission` API and the Rust host PolicyIR evaluator, then
fails on any decision mismatch.

## Acceptance Criteria

- [ ] The harness loads the approved supported schema into pinned SpiceDB `v1.54.0`.
- [ ] The harness uses the B-0005 compiler output for the same exact schema bytes.
- [ ] Shared relationship and caveat fixtures drive SpiceDB and the Rust evaluator.
- [ ] Allowed and denied cases cover direct relations, one-hop subject sets, unions,
      intersections, one arrow, named caveats, and every approved scalar operation.
- [ ] Any SpiceDB conditional result, missing context, fixture drift, or decision
      mismatch fails with a typed test error.
- [ ] One documented container command runs the harness reproducibly on Apple Silicon.
- [ ] Focused equivalence tests and affected workspace gates pass.

## Architecture Constraints

- SpiceDB and Rust must evaluate the same policy, relationships, request context, and
  capability claim values.
- The harness tests equivalence. It does not add runtime authorization behavior.
- Keep relationship tuples and caveat values inside test processes and fixtures.
- Do not expose PolicyIR or relationship paths to an agent-facing surface.
- Do not expand the approved PolicyIR or CEL subset inside this baton.
- Keep policy authorization separate from dataset relevance.
- Add no dependency without maintainer approval.

## File Scope

- `tests/` for live equivalence orchestration and non-sensitive fixtures
- `tools/spicedb-ir-compiler/` for compiler test hooks only when needed
- `crates/policy-ir/` for host-evaluator fixture adapters only when needed
- `docker-compose.yml` and existing scripts only when the harness needs a reproducible
  command surface
- package or lock files only after dependency approval
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- SP1 guest execution, proof generation, or verifier integration
- canonical policy commitments, snapshots, Merkle trees, or witnesses
- service, gateway, SDK, or console integration
- payment-reconciliation source ingestion or real transaction data
- new PolicyIR nodes, caveat operators, recursion, exclusion, or wildcards

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0006.
- Inherited: B-0005 pins SpiceDB `v1.54.0` and exact-byte schema hashing.
- Made: None.

## Open Questions and Blockers

- Determine the smallest existing client and process boundary for driving live
  `CheckPermission` without duplicating policy semantics.
- Confirm whether the harness can use only approved, already pinned dependencies.
- Production PolicyIR ceilings wait for M3 cycle measurements.
- ADR-0002 still needs maintainer approval before gateway integration.
- O-001 remains open for the later real-data acceptance run.

## Work Completed

- B-0005 is archived as
  `docs/agent/history/B-0005-spicedb-policy-ir-compiler.md`.
- The compiler emits deterministic PolicyIR Protobuf and stable JSON diagnostics.
- Rust decodes and validates the reviewed Go compiler golden.
- Supported and rejected compiler fixtures are ready for live execution cases.

## Validation Plan

- focused live SpiceDB equivalence cases
- deterministic fixture import and cleanup checks
- Rust host evaluator tests over the same case table
- mismatch and missing-context mutation tests
- `./scripts/dev pnpm format:check`
- `./scripts/dev pnpm lint`
- `./scripts/dev pnpm typecheck`
- `./scripts/dev pnpm test`
- `git diff --check`

## Validation Evidence

None. B-0006 is not claimed.

## Risks and Private-Data Notes

- SpiceDB consistency tokens and conditional permissions need explicit test handling.
- A duplicated case table can hide drift. Prefer one neutral fixture source.
- Test diagnostics must not print full relationship paths or caveat values.
- Fixture-only equivalence does not satisfy the real-data reconciliation gate.

## Exact Next Action

Claim B-0006, inspect the pinned SpiceDB client APIs and existing test command surface,
then propose any missing dependency with its exact version and fallback.
