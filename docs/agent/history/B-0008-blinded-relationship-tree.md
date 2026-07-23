# B-0008: Blinded Relationship Tree

- Final state: completed
- Milestone: M2, Canonical commitments and snapshots
- Owner: Codex
- Started UTC: 2026-07-21T19:34:48Z
- Closed UTC: 2026-07-23T05:00:24Z
- Branch: `main`
- Base commit: `c1359e1`

## Objective

Define and implement deterministic blinded relationship-tree construction from fixed
relationship input and a caller-supplied private snapshot salt.

## Acceptance Criteria

- [x] One accepted decision record fixes relationship record encoding, leaf and node
      domains, salt handling, leaf ordering, odd-node behavior, and empty-tree policy.
- [x] Rust builds a deterministic relationship root without logging or returning the
      private salt through a public interface.
- [x] Rust produces bounded membership openings that verify against the root and
      rejects malformed, duplicate, unsorted, and oversized inputs.
- [x] Positive and one-field mutation vectors cover relationship records, leaf hashes,
      internal nodes, roots, and openings.
- [x] Focused and affected workspace gates pass.

## Architecture Constraints

- Raw Protobuf bytes are never commitment inputs.
- Relationship tuples, stored caveat values, salts, openings, and satisfying paths are
  Raziel-private witness material.
- The public snapshot root binds one exact relationship set at one SpiceDB revision.
- Leaf ordering uses the canonical unblinded tuple key. The private salt does not alter
  ordering.
- Commitment inputs remain versioned, length-framed, and domain-separated.
- The agent never receives relationship tuples, branch choices, salts, or openings.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `ARCHITECTURE.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/decisions/0009-blinded-relationship-tree.md`
- `crates/canonical/`
- `crates/snapshot/`
- `tests/vectors/relationship-tree-v1.json`
- `packages/proto-ts/test/`
- `biome.json`
- Cargo workspace metadata
- Baton records

## Out of Scope

- Snapshot descriptor signatures and authority key storage
- Policy and accepted-root registries
- SpiceDB relationship loading and ZedToken retrieval
- Witness path search and PolicyIR evaluation
- Services, gateway, SDK, console, SP1 execution, and proof generation
- Real transaction data

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0008.
- Accepted ADR-0009: policy-bound canonical relationship records, structural identity
  order, index-bound salted leaves, a left-balanced unpadded tree, private salt and
  count bound into the public root, and direction-free openings.
- Deferred: O-004 snapshot signing algorithm and key storage remains open for the next
  M2 objective.

## Open Questions and Blockers

- None for B-0008.
- The normal development `spicedb` service has a pre-existing command and key
  incompatibility. It did not block deterministic tree work.
- Deferred follow-up: after the M7 exit gate passes, remind the maintainer to revisit
  the separate Hypha relevance-attestation scope and the Obsidian
  `Current Implementation Discussion Post`.

## Work Completed

- Reconciled the objective against the snapshot Protobuf, PolicyIR relationship model,
  equivalence fixture, canonical crate, architecture, and technical specification.
- Identified the duplicate-last leaf-index alias and replaced that proposed shape with
  a left-balanced unpadded tree.
- Completed two independent protocol audits before approval.
- Received explicit maintainer approval and marked ADR-0009 accepted.
- Added policy-bound canonical relationship records with strict structural identity,
  context provenance, scalar types, size ceilings, and redacted debug output.
- Added versioned leaf, node, empty-tree, and count-bound root commitments.
- Added deterministic left-balanced tree construction and direction-free membership
  openings derived from the private leaf index and count.
- Added a shared private witness set with checked relationship, context, depth, and
  exact private-input byte accounting.
- Applied the same private-input ceiling to single-opening verification.
- Added a deterministic synthetic vector with Rust commitment checks, strict
  TypeScript parsing, byte mutations, and executable Rust semantic mutations.

## Validation Plan

- Deterministic relationship, leaf, tree, root, and opening vectors
- Positive, malformed, oversized, ordering, and one-field mutation tests
- Rust focused tests and affected workspace tests
- Format, lint, typecheck, build, generation, `git diff --check`, and private-data scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Baton start reconciliation | PASS | Clean `main` at `c846ab1`; B-0008 scope matches M2 | 2026-07-21T19:34:48Z |
| Architecture and model inspection | PASS | Snapshot, canonical, PolicyIR, fixture, Protobuf, and M2 requirements reconciled | 2026-07-21T19:39:59Z |
| Two independent protocol audits | PASS after findings incorporated | Tree aliasing, salt binding, ordering, limits, witness shape, and digest width | 2026-07-21T19:53:42Z |
| Documentation checkpoint | PASS | Proposed ADR-0009 committed as `c1359e1` | 2026-07-22T05:34:51Z |
| `./scripts/dev cargo test -p raziel-snapshot` | PASS, 14 tests | Tree, openings, exact limits, vectors, and all recorded Rust mutations | 2026-07-23T04:49:31Z |
| `./scripts/dev cargo clippy -p raziel-snapshot --all-targets -- -D warnings` | PASS | Snapshot crate and all targets | 2026-07-23T04:50:11Z |
| Local protocol and privacy review | PASS | Code matches ADR-0009 and private debug output is redacted | 2026-07-23T04:55:14Z |
| `./scripts/dev pnpm test` | PASS | TypeScript, Rust workspace, and Go suites | 2026-07-23T04:56:12Z |
| `./scripts/dev pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-07-23T04:56:20Z |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust workspace, and Go compile checks | 2026-07-23T04:57:03Z |
| `./scripts/dev pnpm lint` | PASS | Biome, workspace Clippy, Go vet, and Buf | 2026-07-23T04:57:36Z |
| `./scripts/dev pnpm generate:check` and `pnpm generate` | PASS | Deterministic and current Protobuf output | 2026-07-23T04:59:04Z |
| Relationship vector regeneration | PASS | Two runs produced SHA-256 `dac4f41f78c18ac088d9ac9ba214d478ddd276c52cfd98869803e65f48a326aa` | 2026-07-23T04:57:53Z |
| `./scripts/dev pnpm build` | PASS | Next.js, TypeScript, Rust, SP1 guest, and Go builds | 2026-07-23T04:58:35Z |
| `./scripts/dev pnpm bootstrap:check` | PASS | Pinned Linux ARM64 Node, Rust, Go, Buf, and SP1 tools | 2026-07-23T04:58:49Z |
| `./scripts/dev pnpm test:e2e` | SKIP | No package defines an end-to-end test script | 2026-07-23T04:58:56Z |
| `./scripts/test-equivalence` | PASS | Live SpiceDB and Rust matched every approved case | 2026-07-23T04:59:37Z |
| `git diff --check` and credential/log scan | PASS | No whitespace error, logging call, credential pattern, or private runtime artifact | 2026-07-23T05:00:24Z |

## Privacy Checks

- PASS: Runtime types redact relationship records, stored caveat values, salts, and
  openings from debug output.
- PASS: Changed production code contains no logging call.
- PASS: The checked-in vector contains synthetic records and one fixed salt that is
  marked test-only with an explicit production warning.
- PASS: No real transaction row, credential, key, proof, or private runtime artifact
  entered the worktree.

## Risks and Private-Data Notes

- Production snapshot orchestration must generate a fresh salt and store it only with
  protected local snapshot material.
- The signed descriptor and accepted-root registry must preserve the exact root,
  policy hash, revision, epoch, validity, and canonicalization version.
- The future guest must reject unused witness entries as part of evaluator binding.

## Follow-Up

Start B-0009. Resolve O-004, then implement signed snapshot descriptors and local
policy and accepted-root registries without exposing private snapshot material.
