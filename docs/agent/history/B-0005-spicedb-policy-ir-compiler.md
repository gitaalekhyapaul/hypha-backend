# B-0005: SpiceDB PolicyIR Compiler

- Final state: completed
- Milestone: M1, PolicyIR and SpiceDB compiler adapter
- Owner: Codex
- Started UTC: 2026-07-19T07:24:32Z
- Closed UTC: 2026-07-19T17:34:36Z
- Branch: `main`
- Base commit: `bb9db03`

## Objective

Implement the Go adapter from maintained SpiceDB compiler Protobuf output into the
approved PolicyIR subset. Defer live SpiceDB equivalence to a later M1 baton.

## Acceptance Criteria

- [x] The adapter consumes maintained SpiceDB compiler output, not ad hoc schema text.
- [x] Supported relations, subject sets, unions, intersections, one arrow, and named
      caveats translate into typed PolicyIR.
- [x] Exclusion, recursion, wildcards, unbounded traversal, unsupported CEL, invalid
      identifiers, ambiguous targets, and unsupported subject-set terminals fail closed.
- [x] Output follows ADR-0004 structural ordering and is deterministic.
- [x] Supported and rejected compiler fixtures have focused Go tests.
- [x] Go tests, Rust golden validation, Buf generation, and workspace gates pass.

## Architecture Constraints

- The maintained SpiceDB compiler owns schema parsing.
- The adapter is a build-time tool and performs no runtime authorization decision.
- The PolicyIR node set and Rust evaluator semantics remain unchanged.
- Unsupported SpiceDB and CEL constructs fail with typed, source-aware errors.
- Policy translation remains separate from dataset relevance.
- No runtime caveat values, relationship tuples, or private witness data enter output.

## Files Changed

- `.gitattributes` for binary Protobuf fixture classification
- `buf.gen.yaml` and generated Go message configuration
- `tools/spicedb-ir-compiler/go.mod`, `go.sum`, CLI, adapter, config, fixtures, and README
- `go.work.sum`
- `docs/decisions/0005-caveat-input-provenance-config.md`
- `docs/decisions/0006-source-schema-digest.md`
- `crates/policy-ir/tests/compiler_golden.rs`
- `crates/policy-ir/Cargo.toml` and `Cargo.lock` for the existing pinned `prost` test edge

Generated Go message files remain ignored and reproducible through Buf.

## Decisions

- SpiceDB uses the repository pin `v1.54.0`. An earlier `v1.53.0` proposal was removed
  before adapter code depended on it.
- Protobuf Go and the Buf Go generator use `v1.36.11`, generator revision 1.
- ADR-0005 defines strict caveat parameter provenance labels and explicit limits.
- ADR-0006 defines `source_schema_digest` as SHA-256 over exact `.zed` bytes with no
  normalization.
- Human-readable diagnostics use stable, indented Protobuf JSON. Deterministic
  Protobuf is the authoritative artifact.
- Compiler output files use mode `0600`. Both files stage before installation, the
  diagnostic installs first, and the authoritative binary installs last.

## Work Completed

- Added a public compile entry point that clones one schema byte snapshot, hashes it,
  invokes the pinned SpiceDB compiler, and translates maintained compiler records.
- Added strict JSON config decoding with duplicate-key, unknown-field, trailing-value,
  version, limit, provenance label, and exact caveat coverage checks.
- Translated direct relations, one-hop subject sets, unions, intersections, aliases,
  one arrow, named caveats, scalar values, comparisons, and Boolean CEL operations.
- Added explicit structural ordering and compile-time PolicyIR limit checks.
- Rejected exclusion, recursion, wildcards, expiration, functioned arrows, deep arrows,
  ambiguous references, caveated usersets, unsupported CEL, duplicate expressions,
  prefixed identifiers, and nested or caveated subject-set terminals.
- Added typed source-aware error codes for compiler, reference, cycle, caveat, and
  limit failures.
- Added a CLI with explicit schema, config, compiler version, binary output, and JSON
  diagnostic paths.
- Added path alias checks, private output modes, pre-install staging, and diagnostic
  rollback after a binary installation failure.
- Added reviewed binary and JSON golden files. Rust decodes and validates the Go binary
  and checks compiler metadata plus the exact schema digest.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| Dependency and release review | PASS | SpiceDB `v1.54.0`, Protobuf Go `v1.36.11`, Buf generator revision 1 |
| `./scripts/dev pnpm generate` | PASS | Go, Rust, and TypeScript Protobuf output |
| focused Go adapter and config tests | PASS | Supported and rejected compiler fixtures |
| Go versus Rust structural-order audit | PASS | PolicyIR declarations and commutative children |
| exact-byte SHA-256 tests | PASS | Known vector, repeatability, one-byte mutation |
| Go binary and JSON golden tests | PASS | Exact deterministic PolicyIR output |
| `cargo test -p raziel-policy-ir` | PASS, 21 tests | Go golden decode and Rust validation |
| `./scripts/dev pnpm generate:check` | PASS | Reproducible Protobuf generation |
| `./scripts/dev pnpm format:check` | PASS | Workspace formatting |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy, Go vet, Buf lint |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust, and Go |
| `./scripts/dev pnpm test` | PASS | TypeScript, Rust, Go, cross-language golden |
| `./scripts/dev pnpm build` | PASS | Apps, Rust workspace, SP1 guest, Go compiler |
| `./scripts/dev pnpm bootstrap:check` | PASS | Pinned Linux ARM64 toolchain |
| `./scripts/dev pnpm test:e2e` | PASS, no package defines tests yet | Workspace command surface |
| `buf breaking --against .git#branch=main` | PASS | Protobuf compatibility against B-0004 |
| Independent compiler review | FAIL, six findings | Initial fail-closed and output-safety review |
| Independent focused re-review | PASS, all six findings resolved | Final compiler semantics and output safety |
| Final scope, whitespace, credential, and checksum audit | PASS | Exact B-0005 worktree |

## Risks and Private-Data Notes

- Compiler version drift can change accepted syntax or compiled graph records.
- Fixtures contain policy examples only. They contain no transaction data.
- Diagnostics contain PolicyIR and policy constants, but no relationship tuples or
  runtime caveat values.
- A process crash between the two final renames can leave a new diagnostic without a
  new binary. The binary installs last and remains authoritative. Normal errors restore
  the prior diagnostic.
- Live SpiceDB execution equivalence remains unproven in this baton.
- Production PolicyIR ceilings wait for M3 cycle measurements.

## Follow-Up

Start B-0006 and implement the live SpiceDB-versus-Rust PolicyIR equivalence harness
for every approved supported construct. Keep SP1 proof generation and real transaction
data outside that baton.
