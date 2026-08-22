# Engineering Baton History

- Baton ID: B-0023
- State: REVIEW
- Milestone: M4, trusted authorization input assembly
- Owner: Codex
- Started UTC: 2026-08-22T09:15:43Z
- Last updated UTC: 2026-08-22T09:33:01Z
- Branch: `main`
- Worktree: B-0023 implementation, current-status updates, archive, and B-0024 reset
  are ready to commit on `main` after `e7f4d84`. The branch remains five commits ahead
  of `origin/main` before this commit. B-0022 is archived at
  `docs/agent/history/B-0022-canonical-positive-path-witness-construction.md`.

## Objective

Implement the bounded host boundary that combines one admitted capability request,
one trusted approved policy, one trusted signed snapshot selection, and B-0022 witness
construction into a validated `AuthorizationGuestInputV1`. Do not execute SP1 or
start the production proof supervisor in this baton.

## Acceptance Criteria

- [x] Input assembly accepts trust-bearing policy and snapshot selections. It cannot
      approve its own PolicyIR or snapshot descriptor.
- [x] Snapshot ID, policy hash, relationship root, request scope, agent key, trusted
      issue time, and requested expiry bind to the exact assembled guest input.
- [x] The request-to-evaluation mapping uses the existing canonical request and
      authorization-core rules without duplicating commitment encoding.
- [x] B-0022 builds the minimum positive witness from the exact selected snapshot.
- [x] The assembled guest input passes host authorization before it can reach a
      one-shot prover.
- [x] Denial, registry mismatch, snapshot mismatch, invalid time, and witness failure
      produce closed typed errors with no private values.
- [x] Focused assembly tests, affected cross-component tests, strict Clippy,
      formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- `razield` cannot mark its own PolicyIR as approved.
- Every proof input binds an approved policy record and signed snapshot descriptor.
- Policy and snapshot selection happen before witness construction. Request data
  cannot select a different policy, root, salt, relationship set, ELF, or proof mode.
- Relationship tuples, salts, caveat values, paths, openings, and assembled guest
  input remain inside the data-owner proof boundary.
- Canonical encodings, hashes, PolicyIR semantics, snapshot leaves, public values,
  capability fields, and guest behavior do not change in this baton.
- No hosted prover or SP1 network path receives private input.

## File Scope

- `services/razield/src/input_assembly/`
- `services/razield/src/lib.rs`
- `services/razield/Cargo.toml`
- `crates/verifier/src/registry.rs` and focused tests for exact signed-snapshot ID
  resolution through the existing validation path
- `crates/authorization-core/src/statement.rs` and `src/lib.rs` only to export the
  existing evaluation-request mapping
- `Cargo.lock` only for existing internal workspace selections
- Focused input-assembly tests
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- New policy approval or snapshot signing behavior
- A new registry storage format or authority key format
- One-shot child-process isolation, cgroups, or SP1 execution
- Production supervisor activation or terminal queue publication
- `GetCapability`, `CancelCapability`, `verifierd`, ADR-0002, or O-007
- M5, real transaction data, M6 acceptance, M7, or another scenario
- New external dependencies, package versions, toolchains, or proof runs

## Decisions

- Inherited: B-0017 owns admission and the exact capability request binding.
- Inherited: B-0022 owns positive-path selection and validated Merkle witness assembly.
- Inherited: `authorization-core` owns guest input validation and the shared `ALLOW`
  statement.
- Inherited: Local authority and registry types remain the only source of approved
  policy and signed snapshot trust.
- Made: Input assembly owns a mutable `LocalRegistry` lookup by the signed request's
  `snapshot_id`. It does not accept a caller-fabricated resolved record.
- Made: Exact snapshot-ID resolution reuses the same signature, status, policy,
  revocation, and monotonic-time checks as root resolution.
- Made: Export the existing authorization-core evaluation-request function. Do not
  copy capability-claim derivation into `razield`.
- Made: Final proof-bound expiry is worker start plus 75 minutes. Assembly rejects any
  request, policy, or snapshot bound that ends sooner.
- Made: Host authorization returns the expected public values beside the validated
  guest input. The later executor can compare the proof journal without
  reinterpreting `ALLOW`.

## Open Questions and Blockers

- No blocker prevents B-0023 completion.
- The current `LocalRegistry` is the trust-bearing policy and snapshot source. Exact
  snapshot-ID resolution now covers signed requests that do not carry a private root.
- A future private snapshot store must supply the selected relationships and salt.
  B-0023 accepts that in-memory selection and binds it to the registry root. It does
  not define a storage format.
- No new external dependency is planned.
- Security review trigger: exact snapshot-ID resolution changes a verifier-registry
  trust API. The implementation routes root and ID lookup through one shared signature,
  status, policy, revocation, and monotonic-time validation function. It does not add a
  second trust rule.

## Work Completed

- B-0022 implemented and validated canonical positive-path witness construction.
- Read the registry, authority, descriptor, admission, private-envelope,
  authorization-core, and B-0022 witness contracts.
- Confirmed that the registry already validates signatures, status, revocation,
  monotonic time, policy identity, and policy hash before returning a root selection.
- Confirmed that authorization-core already owns the exact request-to-evaluation and
  capability-claim mapping needed by host witness construction.
- Added exact signed-snapshot ID resolution to `LocalRegistry` through the same
  validation path as relationship-root resolution.
- Exported authorization-core's existing evaluation-request mapping without changing
  its capability-claim derivation.
- Added the `razield` input-assembly boundary. It resolves one trusted snapshot,
  consumes one private snapshot selection, constructs the minimum positive witness,
  builds `AuthorizationGuestInputV1`, runs host authorization, and retains the expected
  public journal for the later prover result check.
- Added closed typed errors and redacted debug output for private selections and
  validated guest input.
- Added three focused service cases without a new test harness. They cover the allowed
  path, denial, registry mismatch, snapshot mismatch, request and policy time bounds,
  invalid worker time, exact journal binding, and private output.

## Validation Plan

- Read the authority, verifier registry, admission, private-envelope,
  authorization-core, and witness contracts before fixing the file map.
- Run focused assembly cases for a valid direct path, denial, policy mismatch,
  snapshot mismatch, time mismatch, and private output.
- Run affected service, authority, verifier, authorization-core, and witness tests.
- Run the non-guest workspace gate after focused feature checks pass.
- Run strict Clippy, repository formatting, privacy scans, and `git diff --check`.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0022 archive | PASS | Canonical positive-path witness construction | 2026-08-22T09:09:59Z |
| `cargo test -p razield input_assembly --lib` | PASS, 3 tests | Focused input assembly | 2026-08-22T09:34:00Z |
| `cargo test -p raziel-verifier --test registry` | PASS, 10 tests | Shared exact root and snapshot-ID trust path | 2026-08-22T09:24:00Z |
| `cargo test -p raziel-authorization-core --all-targets` | PASS, 16 tests | Shared request mapping and proof statement | 2026-08-22T09:24:00Z |
| Targeted strict Clippy | PASS | `razield`, verifier, and authorization-core | 2026-08-22T09:26:00Z |
| `cargo test -p razield --all-targets --no-fail-fast` | PASS, 63 active tests | Full issuer service. Eight live Redis cases ignored | 2026-08-22T09:27:00Z |
| Pinned `pnpm test` | PASS, 205 active Rust tests plus TypeScript and Go | Full non-guest workspace. Seventeen proof and live-service cases ignored | 2026-08-22T09:32:00Z |
| Pinned `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-22T09:32:00Z |
| `cargo tree -p razield --depth 1` | PASS | Only existing pinned packages and internal crates | 2026-08-22T09:33:00Z |
| Security and privacy review | PASS | Shared registry validation, redacted types, no production logging | 2026-08-22T09:33:00Z |
| Secret and stale-claim scans | PASS | No credential material and no superseded current-state claim | 2026-08-22T09:33:00Z |
| `git diff --check` and file-scope review | PASS | B-0023 patch hygiene and bounded scope | 2026-08-22T09:33:00Z |

## Privacy Checks

- PASS: Assembly errors and debug output omit requests, relationships, caveats, salts,
  paths, openings, and guest input.
- NOT APPLICABLE: No real transaction data or proof run is part of this baton.

## Risks and Private-Data Notes

- A structurally valid PolicyIR value is not proof of approval. Assembly must require
  a trust-bearing approved selection.
- A signed snapshot descriptor does not carry private relationships or salt. The
  selected private snapshot data must match its committed root and snapshot ID.
- Host authorization is a pre-prover validation gate. It does not replace the SP1
  proof or independent verification.

## Exact Next Action

Inspect the approved one-shot process, cgroup, registry, private snapshot, and
`raziel-prover` contracts before claiming B-0024 implementation scope.
