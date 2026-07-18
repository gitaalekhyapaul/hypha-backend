# Current Engineering Baton

- Baton ID: B-0004
- State: READY
- Milestone: M1, PolicyIR and SpiceDB compiler adapter
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-18T17:53:27Z
- Branch: `main`
- Worktree: Expected clean after the scoped M0 commit. Verify before claiming.

## Objective

Implement the versioned PolicyIR wire model and matching bounded Rust validator and
pure evaluator. Defer SpiceDB translation and equivalence to a later M1 baton.

## Acceptance criteria

- [ ] Protobuf models every approved permission and caveat node from the technical
      specification.
- [ ] Rust conversion rejects unknown versions, enum values, node variants, and
      unresolved references.
- [ ] Validation rejects cycles, malformed child counts, excess arrow depth, type
      mismatches, and each configured limit plus one.
- [ ] The pure evaluator covers relation references, subject relations, unions,
      intersections, one-hop arrows, caveated relations, typed comparisons, and
      Boolean caveat operators.
- [ ] Supported and rejected hand-authored fixtures have deterministic tests.
- [ ] Buf generation, Rust tests, lint, typecheck, and affected workspace tests pass.

## Architecture constraints

- Follow the source order: Protobuf, then Rust semantics, then tests.
- Preserve the exact PolicyIR baseline in the technical specification.
- Keep PolicyIR out of capabilities and agent responses.
- Keep private relationship and caveat values outside logs and public errors.
- Reject unsupported constructs and unknown fields.
- Use bounded collections and explicit proof-boundary limits.
- Do not define canonical commitment bytes in Protobuf.
- Record any semantic conflict in this baton and stop the conflicting edit.

## File scope

- `proto/raziel/v1/`
- `crates/policy-ir/`
- focused PolicyIR fixtures under `tests/`
- generated protocol output through `pnpm generate`
- `docs/agent/CURRENT-BATON.md`
- a decision record only for a durable semantic choice not settled by the spec

## Out of scope

- Go SpiceDB translation
- SpiceDB equivalence harness
- canonical hashes and encodings
- relationship snapshots and Merkle trees
- SP1 guest behavior
- service, gateway, SDK, or console integration
- final payment-reconciliation data

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0003.
- Made: None.

## Open questions and blockers

- The initial numeric PolicyIR ceilings need a measured and documented selection.
- ADR-0002 still needs maintainer approval before gateway integration. It does not
  block this objective.
- O-001 remains open. It does not block this objective.

## Work completed

- M0 passed and is archived as `docs/agent/history/B-0003-m0-scaffold.md`.
- The M0 toolchain, lockfiles, generated-code path, and root gates are ready.

## Validation plan

- `./scripts/dev buf lint`
- `./scripts/dev pnpm generate:check`
- `./scripts/dev cargo test -p raziel-policy-ir`
- `./scripts/dev cargo clippy -p raziel-policy-ir --all-targets -- -D warnings`
- `./scripts/dev pnpm typecheck`
- affected workspace tests
- `git diff --check`
- privacy and private-artifact scan

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Not run | NOT AVAILABLE | B-0004 | 2026-07-18T17:53:27Z |

## Risks and private-data notes

- PolicyIR semantics are proof-critical and need deterministic positive and negative
  fixtures.
- Evaluator traces must not expose a satisfying path or stored caveat values.
- Protobuf bytes are never commitment inputs.
- Keep new fixtures synthetic and free of real transaction data.

## Exact next action

Check Git and current baton state, claim B-0004, then map the PolicyIR specification to
the first Protobuf schema edit before changing Rust code.
