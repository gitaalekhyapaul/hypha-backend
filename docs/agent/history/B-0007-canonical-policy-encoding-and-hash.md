# B-0007: Canonical Policy Encoding and Hash

- Final state: completed
- Milestone: M2, Canonical commitments and snapshots
- Owner: Codex
- Started UTC: 2026-07-21T17:48:58Z
- Closed UTC: 2026-07-21T18:59:04Z
- Branch: `main`
- Base commit: `6b47af1`

## Objective

Define and implement canonical PolicyIR encoding and `policy_hash` in Rust with
versioned deterministic vectors and a TypeScript vector parser. Domain-specific
relationship and snapshot commitments remain separate batons.

## Acceptance Criteria

- [x] One accepted decision record fixes the hash algorithm, signed integer encoding,
      UTF-8 normalization, optional presence bytes, length widths, and structural
      ordering.
- [x] The Rust canonical crate encodes every approved PolicyIR field without hashing
      Protobuf bytes.
- [x] `policy_hash` uses the exact ASCII domain `raziel/policy/v1` and rejects unknown
      or unsupported PolicyIR inputs.
- [x] Checked-in vectors include human input, canonical bytes hex, domain separator,
      expected digest, Rust result, TypeScript parser result, and negative mutations.
- [x] Focused and affected workspace gates pass.

## Architecture Constraints

- Rust remains the only commitment authority.
- TypeScript parses and displays vectors. It does not perform proof-critical hashing.
- The agent sees `policy_hash`, not PolicyIR. The verifier resolves the approved
  PolicyIR through its trusted local registry.
- Every commitment is versioned and domain-separated.
- Raw Protobuf bytes are never commitment inputs.
- Unknown fields, enum values, and unsupported PolicyIR nodes fail closed.
- Test vectors contain no private relationship tuples or caveat values.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `docs/decisions/` for the canonical encoding decision
- `crates/canonical/` for Rust canonical encoding and hashing
- `crates/policy-ir/` only for integration with validated PolicyIR access
- `scripts/run-equivalence` for routing compiler output through strict ingress
- `tests/vectors/` or `tests/canonical/` for deterministic vectors
- `packages/proto-ts/` for vector parsing and tests only
- Cargo metadata only after dependency approval
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Relationship leaf encoding, trees, salts, openings, and witness construction
- Snapshot descriptors, signatures, registries, and revocation
- Capability commitments
- Services, gateway, SDK, console, SP1 execution, and proof generation
- Real transaction data
- Production proof ceilings

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0007.
- Inherited scope decision: on 2026-06-22, descriptive dataset discovery and
  relevance were separated from Raziel's normative policy authorization. The
  repository contract formalized this boundary on 2026-07-18. B-0007 adds no
  relevance attestor, metadata digest, discovery API, or privacy-budget behavior.
- Accepted ADR-0008: SHA-256 over length-framed domain and canonical payload,
  fixed-width big-endian integers, exact UTF-8 bytes, explicit presence tags, `u32`
  lengths and counts, fixed record layouts, explicit recursive ordering, and strict
  descriptor-bound Protobuf ingress.

## Open Questions and Blockers

- None for B-0007.
- Resolved: the maintainer approved ADR-0008 and the stated Rust dependency changes on
  2026-07-22.
- The normal development `spicedb` service has a pre-existing command and key
  incompatibility. It belongs to a separate service-integration baton.
- Deferred follow-up, not a blocker: carry this item through each baton reset. After
  the M7 exit gate passes, remind the maintainer to revisit the separate Hypha
  relevance-attestation scope and the Obsidian `Current Implementation Discussion
  Post`.

## Work Completed

- B-0006 completed the M1 exit gate and is archived as
  `docs/agent/history/B-0006-spicedb-policy-ir-equivalence.md`.
- `crates/canonical/` exists as the M2 placeholder from the initial repository
  scaffold.
- Reconciled the B-0007 objective against the current PolicyIR model, canonical
  encoding requirements, repository history, and clean `HEAD` apart from baton edits.
- Drafted proposed ADR-0008 with the complete canonical encoding, hash preimage,
  ordering, strict ingress, vector, trust-boundary, and dependency rules.
- Completed two independent read-only audits. They identified the existing unknown
  Protobuf-field gap, the ADR-0006 schema-digest length mismatch, derived enum-order
  drift risk, and the smallest shared-vector and TypeScript test scope.
- Confirmed that the reviewed 1,509-byte deterministic Go compiler golden decodes and
  re-encodes byte-for-byte with pinned Prost 0.14.4.
- Enforced accepted ADR-0006 at the Rust type boundary. Validated PolicyIR now stores
  `source_schema_digest` as `[u8; 32]`, strict wire conversion rejects every other
  length, and the obsolete configurable digest ceiling is removed.
- Added frozen-descriptor strict ingress. It rejects oversized or malformed bytes,
  unknown fields at every depth, extensions, alternate Protobuf spellings, unsupported
  versions and enum values, and semantically invalid programs.
- Added explicit protocol tags and recursive structural comparison. Rust declaration
  order can no longer change PolicyIR ordering rules.
- Implemented canonical PolicyIR encoding and SHA-256 `policy_hash` framing in Rust.
  The accepted positive vector digest is
  `f48257d5529d52241aee6976c1579422680a68569c4ac54c01c1c04609d78f64`.
- Added one deterministic shared JSON vector with every supported PolicyIR field and
  node tag, primitive boundary cases, and 26 negative mutations.
- Added a strict test-only TypeScript parser. It validates and displays the Rust-owned
  vector without calculating the hash or exporting production APIs.
- Routed the live equivalence host through strict ingress. SpiceDB and Rust still
  match every approved case.
- Resolved all five independent security-review findings: list-order ambiguity,
  descriptor checksum enforcement, vector coverage, TypeScript parsing, and total
  payload length checking.

## Validation Plan

- Deterministic positive and negative canonical vectors
- Mutated-field and malformed-input tests
- Rust canonical crate tests and workspace tests
- TypeScript parser tests over the same vectors
- Affected format, lint, typecheck, build, and generation gates
- `git diff --check`

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Baton start reconciliation | PASS | `main` at `6b47af1`; only current-baton planning edits present | 2026-07-21T17:48:58Z |
| Canonical boundary inspection | PASS | Empty canonical crate, validated PolicyIR model, M2 protocol choices identified | 2026-07-21T17:48:58Z |
| Independent protocol and vector audits | PASS | Two read-only reviews completed; findings incorporated into proposed ADR-0008 | 2026-07-21T18:05:12Z |
| Go compiler golden Prost round-trip | PASS | Existing deterministic 1,509-byte PolicyIR matched byte-for-byte | 2026-07-21T18:05:12Z |
| `git diff --check` | PASS | Current baton and proposed ADR edits | 2026-07-21T18:05:12Z |
| `./scripts/dev cargo fmt --all -- --check` | PASS | Rust formatting after fixed schema-digest type | 2026-07-21T18:09:03Z |
| `./scripts/dev cargo test -p raziel-policy-ir --test wire_validation` | PASS | 12 passed, including non-32-byte digest rejection | 2026-07-21T18:09:03Z |
| `./scripts/dev cargo test -p raziel-policy-ir` | PASS | 24 passed across unit, golden, evaluator, and wire suites | 2026-07-21T18:09:03Z |
| `./scripts/test-equivalence` | PASS | SpiceDB and Rust matched every live case | 2026-07-21T18:09:03Z |
| `git diff --check` | PASS | Current B-0007 changes | 2026-07-21T18:09:03Z |
| `./scripts/dev cargo test -p raziel-policy-ir --features strict-protobuf` | PASS, 30 tests | PolicyIR, strict ingress, descriptor checksum, fixed digest boundary | 2026-07-21T18:50:16Z |
| `./scripts/dev cargo test -p raziel-canonical` | PASS, 4 tests | Primitive encoding, checked-in bytes and hash, protobuf negative vectors | 2026-07-21T18:50:29Z |
| Proto-TS typecheck and tests | PASS, 8 tests | Strict vector envelope, canonical parser, primitive and mutation cases | 2026-07-21T18:50:52Z |
| Focused Clippy with warnings denied | PASS | `raziel-policy-ir` and `raziel-canonical`, all targets and features | 2026-07-21T18:51:21Z |
| `./scripts/dev pnpm generate` and `generate:check` | PASS | Generated contracts and deterministic regeneration | 2026-07-21T18:51:42Z |
| `./scripts/dev pnpm format:check` | PASS | Biome, Rustfmt, Gofmt | 2026-07-21T18:51:57Z |
| `./scripts/dev pnpm lint` | PASS | Biome, workspace Clippy, Go vet, Buf | 2026-07-21T18:52:20Z |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust workspace, Go compile checks | 2026-07-21T18:52:42Z |
| `./scripts/dev pnpm test` | PASS | TypeScript, Rust workspace, and Go suites | 2026-07-21T18:53:15Z |
| `./scripts/test-equivalence` | PASS | Live SpiceDB and strict-ingress Rust results matched every case | 2026-07-21T18:53:37Z |
| `./scripts/dev pnpm build` | PASS | Next.js, TypeScript, Rust, SP1 guest, and Go builds | 2026-07-21T18:54:20Z |
| Policy vector regeneration checksum | PASS | Two runs produced SHA-256 `29d993b43f4932ab9777a8b66b246a21a7171b774d918336a819807588fd2f97` | 2026-07-21T18:55:02Z |
| `cargo tree -p raziel-canonical -e normal` | PASS | `prost-reflect` absent from the normal canonical and guest-facing graph | 2026-07-21T18:56:44Z |
| `./scripts/dev pnpm bootstrap:check` | PASS | Pinned Linux ARM64 Node, Rust, Go, Buf, and SP1 tools | 2026-07-21T18:57:11Z |
| `./scripts/dev pnpm test:e2e` | PASS, no package defines tests | Current workspace command surface | 2026-07-21T18:57:17Z |
| Independent review and credential-pattern scan | PASS | Protocol, trust boundary, vector coverage, scoped files, no credential material | 2026-07-21T18:57:27Z |
| `git diff --check` | PASS | Final B-0007 worktree before archive | 2026-07-21T18:59:04Z |

## Privacy Checks

- PASS: Agent responses and runtime surfaces contain no PolicyIR, relationship tuple,
  satisfying path, Merkle opening, or stored caveat value.
- PASS: Logs and errors contain no caveat secret or private witness material.
- NOT APPLICABLE: Capability claims, verifier registry access, and prover connections
  did not change in B-0007.
- PASS: Real transaction data remains outside Git and generated output.

## Risks and Private-Data Notes

- The first encoding version becomes a cryptographic compatibility boundary. Field
  tags, integer bytes, normalization, ordering, and hash framing cannot drift silently.
- `prost-reflect` remains host-only. The SP1 guest receives validated PolicyIR and does
  not parse Protobuf or carry reflection and descriptor code.
- Canonical vectors use synthetic policy declarations only. They contain no
  relationship tuples, runtime caveat values, transaction data, or witness material.
- PolicyIR remains verifier-visible. Agents receive only `policy_hash` and approved
  public capability claims.

## Follow-Up

Start B-0008. Define an accepted relationship-leaf and blinded-tree protocol, then
implement deterministic Rust tree construction from fixed relationship input and a
caller-supplied private salt.
