# Engineering Baton History

- Baton ID: B-0017
- State: REVIEW
- Milestone: M4, capability issuance pre-queue admission
- Owner: Codex
- Started UTC: 2026-08-12T08:20:09Z
- Last updated UTC: 2026-08-12T09:27:20Z
- Branch: `main`
- Worktree: B-0017 implementation, accepted decision updates, current-status changes,
  archive, and B-0018 reset are ready to commit on `main` after `3673b9c`.

## Objective

Implement a pure, bounded pre-queue admission boundary for generated
`CreateCapabilityRequest` values. Convert transport fields into the existing
ADR-0011 canonical request model, derive the agent-key thumbprint, rebuild the exact
request commitment, verify the Ed25519 issuance signature over that 32-byte digest,
enforce the accepted local Plonk validity window, and derive the deterministic job
identifier. Stop before storing private data or contacting Redis.

## Acceptance Criteria

- [x] Generated request values convert through strict typed validation into the
      proof-critical canonical request model.
- [x] Admission derives the agent thumbprint from the supplied Ed25519 public key and
      never trusts `agent_id` for authorization.
- [x] The request signature verifies over exactly the ADR-0011 request commitment.
- [x] The deterministic job identifier derives from that commitment.
- [x] Admission enforces the 75-minute local Plonk window against trusted API time and
      rejects malformed, expired, or insufficient upper bounds.
- [x] Every request field, context value, public key, and signature mutation fails or
      changes the derived commitment before any queue operation.
- [x] Errors and debug output contain no private context values, key bytes, signature
      bytes, or commitment bytes.
- [x] Focused tests, affected workspace tests, strict Clippy, formatting, and diff
      checks pass.

## Architecture Constraints

- `agent_id` remains presentation metadata and cannot select the PolicyIR subject.
- Raw Protobuf bytes are never commitment inputs.
- Canonical encodings and commitment semantics stay in proof-critical Rust crates.
- Private request context cannot enter public errors or standard logs.
- The API does not publish a BullMQ job until admission succeeds.
- The proof worker must recheck validity bounds against its trusted start time in a
  later baton.
- ADR-0002 governs protected gateway requests, not this issuance signature.

## File Scope

- `Cargo.lock`
- `crates/authority/src/key.rs`
- `crates/authority/src/lib.rs`
- `crates/authorization-core/src/lib.rs`
- `crates/authorization-core/src/statement.rs`
- `services/razield/Cargo.toml`
- `services/razield/src/lib.rs`
- `services/razield/src/admission.rs`
- `services/razield/tests/admission.rs`
- `docs/decisions/0011-sp1-guest-io-and-public-values.md`
- `docs/decisions/0013-m4-service-and-verifier-execution.md`
- `docs/decisions/0014-agent-key-thumbprint-and-issuance-identity.md`
- `README.md`
- `ARCHITECTURE.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- BullMQ publication, queue state reads, cancellation, or workers
- Private envelope serialization, filesystem storage, or cleanup
- Witness construction, guest execution, proof generation, or proof verification
- Tonic handlers, listeners, interceptors, TLS, or transport policy
- Capability artifact issuance or protected-request verification
- ADR-0002 or O-007
- Packages or versions not already approved and workspace-pinned
- Another proof run, M5, M6, M7, or another scenario

## Decisions

- Inherited: ADR-0011 defines the exact capability request commitment and issuance
  nullifier.
- Inherited: ADR-0013 accepts Ed25519 issuance authentication over that exact digest.
- Inherited: ADR-0013 accepts the 75-minute local Plonk capability window and a
  15-minute minimum usable period after a successful proof.
- Made: The maintainer approved ADR-0014 on 2026-08-12.
- Made: Version 1 hashes the validated raw 32-byte Ed25519 public key with SHA-256.
- Made: The issuance signature covers exactly the 32-byte request commitment and uses
  strict Ed25519 verification.
- Made: The BullMQ custom job ID is the lowercase hexadecimal request commitment. The
  issuance nullifier remains a separate proof-bound value.
- Made: `agent_id` is unsigned, untrusted display metadata.
- Made: Admission accepts whole-second expiry values and requires at least 75 minutes
  from trusted API time.
- Made: Existing workspace-pinned Rust crates were selected without adding a package
  or version.

## Open Questions and Blockers

- None block completion of the pure Rust admission boundary.
- The proof worker must repeat time-window validation at its trusted start time.
- RPC request-size ceilings and generic public error mapping remain handler work.
- The ADR-0014 fixed thumbprint vector still needs a TypeScript match in the agent SDK
  slice.
- ADR-0002 and O-007 remain open before protected-request gateway integration.
- A later positive isolated-verifier gate still needs explicit proof-run approval.

## Work Completed

- Claimed B-0017 from a clean `3673b9c` worktree after B-0016.
- Confirmed that ADR-0011 and ADR-0013 lacked a key-to-thumbprint formula and had
  conflicting duplicate-identity wording.
- Drafted ADR-0014 with a fixed RFC 8032 vector, signature rule, queue ID, metadata
  classification, timestamp rule, migration cost, and rejected options.
- Checkpointed the approval blocker in `a7a8f37` and `3673b9c`.
- Recorded the maintainer's ADR-0014 approval and changed its status to accepted.
- Added one shared SHA-256 raw Ed25519 key helper. Existing authority key IDs now use
  the same digest helper. The fixed ADR-0014 RFC 8032 vector passes.
- Exported the existing authorization-core request validator. Guest validation calls
  the same function, so proof semantics did not change.
- Added the pure `razield` admission module. It strictly parses and rejects weak
  Ed25519 keys, maps typed request context, validates canonical fields, checks whole
  seconds and the 75-minute floor, verifies the exact commitment signature, and
  derives the 64-character lowercase job ID.
- Bounded the context collection before conversion. The module imports no queue,
  filesystem, network, witness, guest, or proof API.
- Added typed, value-free admission errors and redacted debug output.
- Added eight focused tests for valid admission, every authorization-bound field,
  unsigned `agent_id`, malformed and weak keys, signature forms, timestamp bounds,
  context limits, and redaction.
- Clarified ADR-0011 and ADR-0013 identity wording without changing hash formulas or
  M3 vectors.
- Updated README, architecture, and the technical spec to separate completed pure
  admission from pending handlers, storage, publication, and proving.

## Validation Plan

- Run focused admission and mutation tests.
- Confirm no queue or filesystem operation occurs in the admission module.
- Run affected `razield`, authorization-core, and authority tests.
- Run the non-guest Rust workspace in the pinned toolchain.
- Run strict Clippy and repository formatting.
- Run `git diff --check`, link checks, dependency-scope inspection, and private-data
  inspection.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0016 archive | PASS | Capability-window documentation consistency | 2026-08-12T08:18:16Z |
| `git status --short` | PASS, empty | B-0017 start state | 2026-08-12T08:20:09Z |
| Initial focused admission test | FAIL, 6 of 7 passed | Invalid-point fixture was a valid Ed25519 point and reached signature rejection | 2026-08-12T09:27:20Z |
| Corrected focused admission test | PASS, 7 tests | Key and signature fixture correction | 2026-08-12T09:27:20Z |
| Affected three-crate test run | PASS, 41 active tests | Authority, authorization core, and `razield`; 4 Redis tests ignored | 2026-08-12T09:27:20Z |
| Host `cargo test --workspace` | FAIL during compile | SP1 native Gnark build could not find the Go compiler | 2026-08-12T09:27:20Z |
| Pinned-container Rust workspace | PASS, 150 active tests | Non-guest workspace; 9 SP1 and 4 Redis tests ignored | 2026-08-12T09:27:20Z |
| Final focused admission test | PASS, 8 tests | Post-hardening admission suite | 2026-08-12T09:27:20Z |
| Strict affected Clippy | PASS | All targets with warnings denied | 2026-08-12T09:27:20Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-12T09:27:20Z |
| Side-effect import search | PASS, no matches | Admission source and tests | 2026-08-12T09:27:20Z |
| Cargo lock inspection | PASS, no new package or version | Existing workspace selections only | 2026-08-12T09:27:20Z |
| Changed-link targets | PASS | Current-status and decision links | 2026-08-12T09:27:20Z |
| Private-material diff inspection | PASS | Synthetic test value remains test-only | 2026-08-12T09:27:20Z |
| `git diff --check` and file-scope review | PASS | B-0017 patch hygiene | 2026-08-12T09:27:20Z |

## Privacy Checks

- PASS: Agent responses from this boundary expose no PolicyIR, relationship, path, or
  Merkle opening.
- PASS: Debug and error strings contain no request context, key, signature, nonce,
  commitment, or job ID.
- PASS: The boundary has no queue, filesystem, network, witness, guest, or proof API.
- PASS: Tests use one clearly synthetic private-looking context value.
- NOT APPLICABLE: No real transaction data, witness, or proof artifact is processed.
- NOT APPLICABLE: Verifier registry and prover independence are outside this baton.

## Risks and Private-Data Notes

- The later RPC handler must enforce its request-byte ceiling before Protobuf decoding.
- The later proof worker must repeat the 75-minute upper-bound check against trusted
  worker start time and subordinate registry bounds.
- The `AdmittedCapabilityRequest` carries private context in memory. Callers must pass
  it only to the owner-only private-envelope boundary.
- The TypeScript SDK must match the fixed thumbprint vector without becoming a
  client-side policy evaluator.
- Direct host workspace tests need pinned Go 1.26.5 for SP1's native Gnark build. The
  existing toolchain container supplies it and passed the full non-guest workspace.

## Exact Next Action

Commit the B-0017 admission slice. Then inspect and claim B-0018 for the versioned,
owner-only private request envelope without starting Redis or proof work.
