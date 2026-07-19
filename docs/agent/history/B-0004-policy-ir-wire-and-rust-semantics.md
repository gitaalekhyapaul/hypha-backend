# B-0004: PolicyIR Wire and Rust Semantics

- Final state: completed
- Milestone: M1, PolicyIR and SpiceDB compiler adapter
- Owner: Codex
- Started UTC: 2026-07-18T17:59:44Z
- Closed UTC: 2026-07-18T18:36:34Z
- Branch: `main`

## Objective

Implement the versioned PolicyIR wire model and matching bounded Rust validator and
pure evaluator. Defer SpiceDB translation and equivalence to later M1 batons.

## Acceptance Criteria

- [x] Protobuf models every approved permission and caveat node.
- [x] Rust conversion rejects unknown versions, enum values, absent or unknown nodes,
      unresolved references, missing operands, and excessive syntax depth.
- [x] Validation rejects cycles, malformed child counts, two-arrow paths, type errors,
      noncanonical order, and every configured limit plus one.
- [x] Evaluation covers direct relations, one-hop subject sets, unions, intersections,
      one-hop arrows, tuple-bound caveats, typed comparisons, and Boolean operators.
- [x] Twenty deterministic supported and rejected fixture tests pass.
- [x] Generation, tests, lint, typecheck, build, and affected workspace gates pass.

## Architecture Constraints

- PolicyIR remains verifier-visible and absent from capabilities and agent responses.
- Relationship tuples and caveat values enter only through the read-only resolver.
- Evaluation returns one Boolean or a typed error. It returns no path or trace.
- Protobuf remains a transport contract, not a canonical commitment encoding.
- Program limits cannot exceed caller-supplied trusted ceilings.
- Policy authorization remains separate from dataset relevance.

## Files Changed

- `proto/raziel/v1/policy_ir.proto` and the typed registry field in `policy.proto`
- `crates/proto/` and Rust generation layout in `buf.gen.yaml`
- `crates/policy-ir/` model, conversion, validation, evaluation, errors, and fixtures
- `packages/proto-ts/` generated-contract exports and package test
- root Cargo manifests and lockfile
- `ARCHITECTURE.md` and ADR-0004

## Decisions

- Added ADR-0004 for typed declarations and the evaluator boundary.
- `RelationRef` accepts direct uncaveated tuples.
- `SubjectRelationRef` makes exactly one declared userset hop.
- `Arrow` requires one direct uncaveated target type and one arrow per path.
- `CaveatedRelation` binds context to the exact matching tuple.
- Permission and caveat composites evaluate all children and propagate any error.
- Set-like collections require strict structural order and reject duplicates.
- Prost messages and Tonic service stubs generate into separate directories.
- Test ceilings are fixtures. Production ceilings wait for M3 cycle measurement.

## Work Completed

- Added all six permission nodes and all thirteen caveat nodes to Protobuf.
- Added object, relation, permission, caveat, parameter, and limit declarations.
- Replaced the opaque M0 registry placeholder with typed `PolicyProgram`.
- Added strict generated-message conversion into a validated domain model.
- Added reference, ordering, type, cycle, arrow, syntax, and limit validation.
- Added a bounded resolver-based evaluator with tuple-local caveat context.
- Added runtime accounting for expression nodes, caveat nodes, path depth, witnesses,
  Merkle depth, string bytes, context entries, and private-input bytes.
- Added positive, denied, malformed, mutation, boundary, and privacy fixtures.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev buf lint` | PASS | Protobuf | 2026-07-18T18:08:17Z |
| `./scripts/dev pnpm generate:check` | PASS, two generations matched | Generated contracts | 2026-07-18T18:27:29Z |
| First `cargo check -p raziel-proto` | FAIL, Tonic stubs entered message output | Rust protocol crate | 2026-07-18T18:08:17Z |
| First split-output generation | FAIL, Tonic insertion target absent | Rust generation | 2026-07-18T18:08:17Z |
| `cargo check -p raziel-proto` with `no_include` | PASS | Rust protocol crate | 2026-07-18T18:08:17Z |
| First `cargo test -p raziel-policy-ir` | FAIL, test ceiling below fixture metadata | PolicyIR | 2026-07-18T18:27:29Z |
| `cargo test -p raziel-policy-ir` | PASS, 20 tests | PolicyIR | 2026-07-18T18:36:34Z |
| First focused Clippy | FAIL, test type alias needed | PolicyIR | 2026-07-18T18:27:29Z |
| Focused Clippy with `-D warnings` | PASS | PolicyIR | 2026-07-18T18:28:41Z |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust, and Go | 2026-07-18T18:27:29Z |
| `./scripts/dev pnpm test` | PASS, 7 TypeScript and 27 Rust tests plus Go | Workspace | 2026-07-18T18:28:41Z |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy, Go vet, and Buf | 2026-07-18T18:28:41Z |
| `./scripts/dev pnpm format:check` | PASS | Workspace | 2026-07-18T18:28:41Z |
| `./scripts/dev pnpm bootstrap:check` | PASS | Pinned toolchain | 2026-07-18T18:36:34Z |
| `./scripts/dev pnpm build` | PASS, including SP1 guest | Workspace | 2026-07-18T18:36:34Z |
| `buf breaking --against .git#branch=main` | FAIL as expected, M0 placeholder became typed | Pre-release wire API | 2026-07-18T18:36:34Z |
| Private-artifact and log scan | PASS | B-0004 source and fixtures | 2026-07-18T18:36:34Z |
| `git diff --check` | PASS | B-0004 worktree | 2026-07-18T18:36:34Z |

## Privacy Review

- Capabilities and public values contain no PolicyIR: PASS.
- Evaluator errors contain no relationship or caveat values: PASS.
- Evaluator emits no path, trace, opening, or log: PASS.
- Relationship context binds to its exact tuple: PASS.
- Fixtures contain synthetic values only: PASS.
- Generated and proof artifacts remain ignored: PASS.

## Remaining Risks

- Prost discards ordinary unknown fields. M2 must reject them at strict canonical
  ingress before commitment conversion.
- M2 must publish the cross-language structural comparator and canonical byte vectors.
- M3 cycle counts must select production numeric ceilings.
- The witness boundary must derive resolver metrics from validated witness material.
- The typed registry field is an intentional pre-release break from the M0 placeholder.
- ADR-0002 still needs maintainer approval before gateway integration.
- O-001 still needs a real transaction source and usage basis.

## Handoff

Continue with B-0005 in `docs/agent/CURRENT-BATON.md`. Inspect the maintained SpiceDB
compiler Protobuf API, then request approval for exact pinned Go dependencies before
implementing translation.
