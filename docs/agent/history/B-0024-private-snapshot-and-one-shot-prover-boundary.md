# Engineering Baton History

- Baton ID: B-0024
- State: REVIEW
- Milestone: M4, one-shot local Plonk executor boundary
- Owner: Codex
- Started UTC: 2026-08-22T09:44:01Z
- Last updated UTC: 2026-08-22T18:54:02Z
- Branch: `main`
- Worktree: B-0024 implementation, ADR-0015, current-state updates, archive, and
  B-0025 reset are ready to commit on `main` after `dc90446`. The branch remains six
  commits ahead of `origin/main` before this commit. B-0023 is archived at
  `docs/agent/history/B-0023-trusted-authorization-input-assembly.md`.

## Objective

Implement the bounded one-shot prover boundary that obtains trusted local registry
and private snapshot state, calls B-0023 input assembly, generates one local Plonk
proof, compares its complete public journal, and returns a bounded public result to
the proof supervisor. Keep the production supervisor disabled until terminal result
validation exists.

## Acceptance Criteria

- [x] Fixed owner configuration selects the registry generation, private snapshot
      source, guest ELF, Plonk setup, proof mode, and resource profile.
- [x] Request data cannot select or override trusted files, proof settings, network
      access, resource limits, or output paths.
- [x] The one-shot process protocol is versioned, bounded, strict, and redacts every
      private input from debug and error output.
- [x] The executor uses B-0023 assembly and the approved local Plonk prover. It rejects
      host denial, proof failure, journal mismatch, malformed output, and cancellation.
- [x] The child feature graph has no hosted proving or SP1 network package. It handles
      one request and one response. B-0025 owns external deadline, cancellation, and
      confirmed child-exit enforcement through the fixed launcher.
- [x] Only the bounded proof artifact, approved public values, and safe aggregate
      metrics return to the supervisor.
- [x] Focused process and failure tests, affected cross-component tests, strict
      Clippy, formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- ADR-0012 governs local Plonk proving, proof artifacts, key identity, and host
  comparison.
- ADR-0013 governs separate supervisor and one-shot prover processes, the exact
  60,129,542,144-byte no-swap cgroup, four visible CPUs, one-hour deadline, no
  network, read-only root, and bounded private scratch.
- B-0023 remains the only service boundary that assembles trusted guest input.
- Relationship tuples, salts, caveat values, paths, openings, and guest input never
  enter Redis, standard logs, public results, or hosted services.
- Production supervisor activation and terminal capability issuance require a later
  strict result validator.

## File Scope

- Proposed after approval: `services/razield/src/private_snapshot/`
- Proposed after approval: `services/razield/src/one_shot/`
- Proposed after approval: `services/razield/src/bin/razield-prover.rs`
- Proposed after approval: the existing worker executor result contract
- Proposed after approval: shared verifier registry configuration reused by
  `raziel-admin` and the child
- Proposed after approval: `services/razield/Cargo.toml` and `Cargo.lock` only for
  existing internal workspace packages and features
- Proposed after approval: ADR-0013, current-state docs, and focused tests
- `docs/decisions/0015-private-snapshot-and-one-shot-prover-boundary.md`
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Production supervisor activation or terminal queue publication
- Capability signing, `GetCapability`, or `CancelCapability`
- Independent verifier service behavior
- ADR-0002, O-007, M5, M6, M7, or another scenario
- A hosted prover, SP1 network prover, CUDA path, Core capability, or mock production
  proof
- New dependencies, package versions, storage formats, or toolchains without explicit
  maintainer approval

## Decisions

- Inherited: ADR-0012 and ADR-0013 fix the local Plonk profile and process topology.
- Inherited: B-0021 owns BullMQ claim, deadline, cancellation, and envelope cleanup.
- Inherited: B-0023 owns trusted input assembly and expected public journal creation.
- Approved: ADR-0015 on 2026-08-23.
- Made through ADR-0015: First implement the real one-shot `razield-prover` child,
  strict request and result protocol, and owner-only private snapshot store. Keep the
  fixed Compose proof-sandbox launcher integration and supervisor activation in
  B-0025.
- Made through ADR-0015: Key private snapshot files by the registry-approved
  relationship-root digest. Apply the existing ceilings of 256 relationships, 4 KiB
  per canonical relationship, and 1 MiB total canonical relationship bytes. Use a
  2 MiB transport ceiling for the versioned private Protobuf file.

## Open Questions and Blockers

- None for B-0024.
- B-0025 must connect the supervisor to the fixed Compose launcher and validate the
  returned proof before any terminal success or production activation.

## Work Completed

- B-0023 implemented and validated trusted authorization input assembly.
- Read ADR-0012, ADR-0013, the BullMQ executor handoff, current supervisor settings,
  `raziel-prover`, registry loading, Compose isolation, and cluster cgroup checks.
- Confirmed that `Sp1Prover` already performs local CPU Plonk proving, fixed setup
  validation, exact public-journal comparison, and bounded artifact encoding.
- Confirmed that no private snapshot persistence format or production one-shot
  launcher exists.
- Confirmed that the registry's fixed `current.pb` manifest and generation files can
  supply the child trust state without a second registry format.
- Confirmed that a root-digest file name avoids request-controlled paths and needs no
  new hashing dependency. The file still carries and checks the exact snapshot ID,
  root, salt, ordered relationships, and stored caveat values.
- Mapped a bounded child request to the validated private envelope and trusted worker
  start time. Mapped the result to the Plonk artifact, exact public values,
  verification-key ID, and aggregate timing and size metrics.
- Drafted ADR-0015 with the private record, registry binding, child protocol,
  privilege limits, security effects, rejected options, and exact approval request.
- Refined ADR-0015 after maintainer review. It now distinguishes ZedTokens from
  private snapshots, fixes the 56 GiB versus 60.13 GB unit, makes Docker controls
  mandatory, selects a fixed Unix-socket proof sandbox, explains database tradeoffs,
  and diagrams the supervisor.
- Maintainer approved ADR-0015 and resumed B-0024 on 2026-08-23.
- Added the immutable private snapshot store. Its strict Protobuf codec preserves
  typed caveat values, enforces the 2 MiB transport cap, binds exact snapshot identity,
  and rebuilds the blinded root under the existing relationship ceilings.
- Added owner, mode, file-type, link-count, immutable-publication, root-derived-name,
  mutation, ordering, duplicate, and redaction checks for private snapshots.
- Moved the shared registry limits into `raziel-verifier`, reused them from
  `raziel-admin`, and added fixed `current.pb` loading for the child.
- Added a strict 32 KiB request and 2 MiB result protocol with canonical Protobuf,
  one big-endian frame, EOF enforcement, exact fixed deadline, typed generic failure,
  proof and public-value limits, canonical key IDs, and safe metrics.
- Added the feature-gated `razield-prover` binary. Fixed owner configuration selects
  the Unix socket, current registry, private snapshot root, guest ELF, Plonk setup,
  trust domain, expected key ID, and resource profile.
- Added startup checks for exact cgroup memory, zero swap, four visible CPUs, Gnark
  process settings, safe SP1 environment, pinned Plonk artifacts, trusted file shapes,
  and an owner-only `0600` Unix socket before the child reads private input.
- Wired the child through B-0023 assembly, SP1 execute, local CPU Plonk proving, exact
  journal comparison, expected key-ID comparison, and a bounded public result.
- Changed the supervisor executor contract to return that bounded result. The runtime
  still discards every success and records `SERVICE_UNAVAILABLE`, so issuance remains
  closed before B-0025 terminal validation.
- Kept `raziel-prover` optional behind `one-shot-prover`. The default `razield` graph
  excludes the SP1 SDK. The feature graph contains local `sp1-sdk` but no SP1 network
  package.
- Updated ADR-0015's test split so B-0025 owns launcher, process-exit, cancellation,
  timeout, crash, and rendered Compose gates, matching its approved staging section.
- Updated README, architecture, and the technical specification with the implemented
  boundary and exact pending production work.

## Validation Plan

- Read ADR-0012, ADR-0013, the worker executor contract, `raziel-prover`, registry
  loading, snapshot persistence, and existing cgroup scripts before fixing scope.
- Run focused process-protocol, cancellation, malformed-result, privacy, and journal
  comparison tests.
- Run affected service, prover, verifier, authorization-core, witness, and snapshot
  tests without executing SP1 unless the maintainer approves a proof run.
- Run the pinned non-guest workspace, strict Clippy, formatting, privacy, dependency,
  and diff gates.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0023 archive | PASS | Trusted authorization input assembly | 2026-08-22T09:33:01Z |
| B-0024 contract review | PASS | Existing prover, worker, registry, snapshot, Compose, and cgroup boundaries mapped | 2026-08-22T09:44:01Z |
| B-0024 field and limit review | PASS | Existing snapshot, registry, envelope, and proof ceilings reused | 2026-08-22T09:47:16Z |
| ADR-0015 draft | PASS | Both blocking decisions written with rationale and approval boundaries | 2026-08-22T09:58:13Z |
| ADR-0015 maintainer review amendments | PASS | Docker cap, ZedToken, storage, and supervisor questions incorporated | 2026-08-22T10:22:12Z |
| ADR-0015 approval | PASS | Maintainer approved both proposed items | 2026-08-22T18:22:35Z |
| `cargo test -p razield private_snapshot` | PASS, 5 tests | Private snapshot codec, binding, filesystem, limits, and redaction | 2026-08-22T18:30:00Z |
| `cargo test -p razield --lib` | PASS, 64 active tests | Full issuer library including worker result contract | 2026-08-22T18:39:00Z |
| Pinned `cargo check` and strict Clippy with `one-shot-prover` | PASS | Real child and local proving feature compile without host Go | 2026-08-22T18:44:00Z |
| Pinned one-shot feature tests | PASS, 7 tests | Protocol, owner configuration, cgroup, limits, and redaction. No proof run | 2026-08-22T18:47:00Z |
| `cargo test -p raziel-admin` | FAIL, then PASS on isolated rerun | Existing nanosecond temp-directory fixture collided once. Clean rerun passed 4 tests | 2026-08-22T18:50:00Z |
| Affected verifier and issuer tests | PASS, 87 active tests | Shared registry config, `current.pb`, service, worker, snapshot, and protocol | 2026-08-22T18:50:00Z |
| Pinned `pnpm test` | PASS | Full non-guest TypeScript, Rust, and Go workspace. Proof and live Redis tests stayed ignored | 2026-08-22T18:53:00Z |
| Pinned `pnpm format:check` and `pnpm lint` | PASS | Biome, Rustfmt, Gofmt, strict workspace Clippy, Go vet, and Buf | 2026-08-22T18:54:00Z |
| Dependency graph and privacy review | PASS | Default graph excludes SP1 SDK, feature graph has no SP1 network package, private output stays redacted | 2026-08-22T18:54:00Z |
| `git diff --check` and file-scope review | PASS | B-0024 patch hygiene and approved scope | 2026-08-22T18:54:00Z |

## Privacy Checks

- PASS: Snapshot, request, configuration, result, and store debug output redact
  private fields, paths, salts, relationships, caveat values, envelopes, and proof
  bytes.
- PASS: The child has no logging call. Its returned `main` error uses only closed
  generic transport, configuration, resource, and execution text.
- PASS: Redis and BullMQ types remain unchanged and receive no snapshot, salt,
  relationship, caveat, witness, path, opening, guest input, or proof bytes.
- NOT APPLICABLE: No real transaction data or SP1 proof run occurred.

## Risks and Private-Data Notes

- B-0025 must redirect and review launcher-managed child output, remove private
  scratch after every exit, and confirm process death after cancellation or timeout.
- The child validates cgroup and owner state, but only B-0025's rendered Compose gate
  can prove no network, read-only root, fixed mounts, and bounded scratch.
- The one-shot result is not a capability. B-0025 must verify the proof, journal,
  request commitment, correlation, and trusted program key before recording success.

## Exact Next Action

Archive B-0024, reset the current baton to B-0025, commit the completed child-first
slice, then inspect the fixed Compose launcher and terminal result validator scope.
