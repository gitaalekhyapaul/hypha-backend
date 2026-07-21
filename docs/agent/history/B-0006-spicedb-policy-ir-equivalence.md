# B-0006: SpiceDB PolicyIR Equivalence

- Final state: completed
- Milestone: M1, PolicyIR and SpiceDB compiler adapter
- Owner: Codex
- Started UTC: 2026-07-20T08:41:51Z
- Closed UTC: 2026-07-20T19:44:03Z
- Branch: `main`
- Base commit: `4cff965`

## Objective

Implement a live equivalence harness that runs approved authorization cases through
the pinned SpiceDB `CheckPermission` API and the Rust host PolicyIR evaluator, then
fails on any semantic mismatch.

## Acceptance Criteria

- [x] The harness loads the approved supported schema into pinned SpiceDB `v1.54.0`.
- [x] The harness uses the B-0005 PolicyIR output for the same exact schema bytes.
- [x] One shared fixture drives relationships, source-separated caveat inputs, and
      expected results through both engines.
- [x] Allowed and denied cases cover every approved permission and scalar construct.
- [x] Explicit conditional probes match Rust `MissingCaveatInput`. Unexpected
      conditionals, drift, malformed input, or result mismatches fail with typed
      errors.
- [x] One documented command runs reproducibly on Linux ARM64 and Apple Silicon.
- [x] Focused, live, workspace, build, and review gates pass.

## Architecture Constraints

- The harness is test-only and adds no runtime authorization surface.
- Policy authorization remains separate from dataset relevance.
- Relationship tuples and caveat values remain inside synthetic fixtures and test
  processes.
- PolicyIR, satisfying paths, and witness material remain outside agent-facing data.
- Compiler configuration provenance remains distinct from runtime caveat values.
- Production evaluation remains Boolean-or-error.

## Decisions

- Maintainer approved `github.com/authzed/authzed-go v1.9.0`, `serde 1.0.228`, and
  `serde_json 1.0.145` for the typed test harness.
- A profile-gated SpiceDB service uses no host ports or persistent volume. The runner
  is hardcoded to its service name and test key, so direct CLI use cannot target an
  arbitrary datastore.
- One strict versioned JSON fixture binds exact schema bytes, compiler configuration,
  reviewed PolicyIR Protobuf, relationships, source-separated contexts, and expected
  outcomes.
- ADR-0007 supersedes ADR-0004's missing-input propagation clause. `And` absorbs
  missing input only after a false child. `Or` absorbs missing input only after a true
  child. Every child is visited. Non-missing errors propagate.
- `CONDITIONAL` exists only in the equivalence fixture. Go accepts it only from
  SpiceDB for an explicitly conditional case. Rust accepts it only from
  `MissingCaveatInput` for that case.

## Work Completed

- Added the shared fixture with 13 relationships and 21 sorted cases. Four cases cover
  `false && missing`, `true && missing`, `true || missing`, and `false || missing`.
- Added strict Go and Rust fixture loaders with bounded files and collections, path
  confinement, exact schema digest checks, complete compiler-config binding, typed
  scalars, deterministic ordering, and value-safe errors.
- Added a Go runner that writes the exact schema, resets only fixture resources,
  writes relationships, and checks every case at the exact relationship-write token.
- Added a Rust host runner over the same fixture and reviewed PolicyIR binary.
- Added deterministic, private-mode result files and a shell comparator with
  signal-safe cleanup.
- Added the live gate to CI and documented `./scripts/test-equivalence`.
- Added full CEL missing-input absorption in the Rust evaluator. Type, malformed data,
  resolver, and budget errors remain non-absorbable.
- Added ADR-0007 and aligned ADR-0004 plus the technical specification.
- Regenerated the supported compiler binary and JSON goldens. The exact schema digest
  is `6585fe03c001ea948b40faa3050101910954df4347e5a84d9d4ec4266e7201a2`.
- Resolved all independent review findings for datastore safety, config binding, CI,
  Compose isolation, bounded reads, CEL semantics, full-child evaluation coverage,
  and baton accuracy.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| `./scripts/test-equivalence` | PASS, 21 cases | Live SpiceDB `v1.54.0` versus Rust at exact write revision |
| `cargo test -p raziel-policy-ir` | PASS, 23 tests | Evaluator, compiler golden, wire validation, full-child regression |
| Rust equivalence example tests | PASS, 16 tests | Fixture, provenance, drift, missing-input, mismatch handling |
| `go test ./...` | PASS | Compiler, fixture loader, live runner, expected and unexpected conditional handling |
| `go mod tidy -diff` | PASS | Go dependency graph is tidy |
| `pnpm generate:check` | PASS | Deterministic Protobuf generation |
| `pnpm format:check` | PASS | Biome, Rustfmt, Gofmt |
| `pnpm lint` | PASS | Biome, Clippy with warnings denied, Go vet, Buf |
| `pnpm typecheck` | PASS | TypeScript, Rust, Go |
| `pnpm test` | PASS | Full workspace suite |
| `pnpm build` | PASS | Apps, Rust workspace, SP1 guest, Go commands |
| `pnpm bootstrap:check` | PASS | Pinned Linux ARM64 toolchain |
| `pnpm test:e2e` | PASS, no package defines tests | Current workspace command surface |
| Compose model and shell syntax | PASS | Equivalence profile and both scripts |
| Independent final review | PASS after two resolved findings | CEL semantics, fixture boundary, prior safety fixes |
| Cleanup, scope, whitespace, dependency, and credential audit | PASS | Final B-0006 worktree |

## Privacy Checks

- PASS: Agent responses contain no private PolicyIR, relationship path, or Merkle
  opening.
- PASS: Logs and diagnostics contain no private caveat value or witness material.
- NOT APPLICABLE: No capability, verifier registry, or prover connection changed.
- PASS: Real transaction data remains outside Git and generated output.

## Risks and Remaining Work

- Synthetic equivalence fixtures do not satisfy the real-data reconciliation gate.
- Test-only `CONDITIONAL` must never become an agent-facing authorization response.
- Final production PolicyIR ceilings wait for M3 cycle measurements.
- The normal development `spicedb` service retains its pre-existing
  `serve-testing` plus preshared-key incompatibility with SpiceDB `v1.54.0`. B-0006
  leaves it unchanged. Resolve it in a separate service-integration baton.
- Docker Desktop's configured credential helper stalled the first public image pull.
  A temporary empty `DOCKER_CONFIG` pulled the exact pinned ARM64 digest without
  changing user configuration.

## Follow-Up

Start B-0007. Define canonical PolicyIR encoding and `policy_hash` in Rust with
versioned deterministic vectors plus a TypeScript vector parser. Defer relationship
leaves, blinded trees, snapshot descriptors, signatures, and registries.
