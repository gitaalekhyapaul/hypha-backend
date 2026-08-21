# Engineering Baton History

- Baton ID: B-0018
- State: REVIEW
- Milestone: M4, private capability request envelope
- Owner: Codex
- Started UTC: 2026-08-21T03:48:23Z
- Last updated UTC: 2026-08-21T04:05:32Z
- Branch: `main`
- Worktree: B-0018 implementation, current-status updates, archive, and B-0019 reset
  are ready to commit on `main` after `280f703`.

## Objective

Implement the bounded private-envelope boundary used after successful capability
admission and before BullMQ publication. Encode the validated canonical request in a
versioned internal Protobuf envelope, persist it through an owner-only atomic file
transition, and return an opaque reference for the later queue publisher.

## Acceptance Criteria

- [x] The envelope contains only validated post-admission fields needed by the proof
      worker and never uses its Protobuf bytes as a commitment input.
- [x] Serialization and storage apply fixed byte ceilings before durable publication.
- [x] Directories use owner-only access and published envelope files use mode `0600`.
- [x] Creation uses a private temporary file, data sync, atomic no-clobber
      publication, temporary-name removal, and directory sync without replacing an
      existing job envelope.
- [x] Reads reject symbolic links, non-regular files, wrong modes, malformed bytes,
      wrong versions, and oversized files.
- [x] Deletion supports later queued-job cancellation and orphan cleanup without
      exposing private paths or values.
- [x] Private structs, errors, and debug output redact request context, nonces, keys,
      signatures, commitments, and filesystem details.
- [x] Focused tests, affected workspace tests, strict Clippy, formatting, and diff
      checks pass.

## Architecture Constraints

- The envelope boundary runs only after B-0017 admission succeeds.
- Raw Protobuf bytes are transport data and never commitment inputs.
- Redis receives only the opaque envelope reference and public job metadata.
- Private request context cannot enter queue payload metadata, public errors, or
  standard logs.
- The store must fail closed on links, replacement races, partial writes, malformed
  data, and cleanup errors.
- No witness construction, guest execution, proof generation, or verification occurs
  in this boundary.

## File Scope

- `services/razield/Cargo.toml`
- `services/razield/src/lib.rs`
- `services/razield/src/admission.rs` only for a narrow ownership handoff if required
- New private-envelope and store modules under `services/razield/src/`
- `services/razield/tests/` only for private-envelope and store coverage
- Internal Protobuf source and generated Rust output only if the inspected repository
  pattern requires them
- `Cargo.lock` only for selecting existing approved workspace pins
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- BullMQ publication, queue reads, cancellation orchestration, or workers
- Tonic handlers, listeners, interceptors, TLS, or transport policy
- Trusted policy, snapshot, delegation, or key registry resolution
- Witness construction, guest execution, proof generation, or proof verification
- Capability artifact issuance or protected-request verification
- ADR-0002, O-007, M5, M6, M7, or another scenario
- New packages, versions, toolchains, or proof runs without explicit approval

## Decisions

- Inherited: ADR-0013 requires a versioned internal Protobuf envelope, owner-only
  storage, atomic publication before queue publication, fixed byte ceilings, and
  deletion before terminal state.
- Inherited: ADR-0014 fixes post-admission identity, signature, job-ID, metadata, and
  timestamp rules.
- Inherited: Existing workspace-pinned dependencies can be selected. No new package
  or version is approved.
- Made: Keep the internal envelope as private Rust-only Prost message types under
  `services/razield/src/private_envelope/`. Do not add it to the public Buf contract
  or generated TypeScript and Go packages.
- Made: Use the repository's existing no-clobber publication pattern: create and sync
  a private temporary file, atomically hard-link it to the final name, remove the
  temporary name, and sync the store directory. This preserves an existing envelope
  instead of using replacement rename semantics.

## Open Questions and Blockers

- None block completion of the private-envelope boundary.
- RPC ingress ceilings, queue publication, worker-time expiry validation, and public
  error mapping remain later boundaries.
- The ADR-0014 TypeScript thumbprint vector remains pending for the agent SDK slice.
- ADR-0002 and O-007 remain open before protected-request gateway integration.

## Work Completed

- B-0017 implemented and tested pure capability issuance admission.
- Inspected the public Protobuf generation path and existing atomic registry writer.
  The internal envelope stays service-private and reuses approved `prost` and standard
  library filesystem facilities.
- Added private Rust-only Prost wire types with custom redacted debug output. No
  private schema enters Buf, TypeScript, Go, or the public RPC package.
- Added strict encoding and decoding with a 16 KiB ceiling. Decode re-encodes the
  message to reject unknown, duplicate, reordered, and non-canonical fields.
- Decode rebuilds the ADR-0011 request, repeats semantic validation, recalculates its
  commitment, and matches the commitment to both stored bytes and the opaque
  reference.
- Added a dedicated `0700` store with `0600` temporary and final files, file and root
  identity checks, data sync, atomic hard-link publication, no replacement, and
  directory sync.
- Added bounded reads, idempotent deletion, and grace-based cleanup driven by a later
  publisher's live-reference set.
- Added 13 focused tests for round trips, maximum valid size, concurrent publication,
  permissions, links, malformed bytes, wrong versions, file swaps, commitment
  mutations, deletion, orphan cleanup, root replacement, and redaction.
- Updated README, architecture, and the technical specification to distinguish the
  completed store boundary from pending RPC and queue wiring.

## Validation Plan

- Run focused codec, filesystem mode, race, malformed-input, and cleanup tests.
- Confirm queue, network, witness, and proof APIs are absent from the boundary.
- Run affected `razield` and shared-contract tests.
- Run strict Clippy and repository formatting in the pinned toolchain.
- Run `git diff --check` and inspect private-data handling.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0017 archive | PASS | Capability issuance admission | 2026-08-12T09:27:20Z |
| `cargo test -p razield` | PASS, 25 active tests | Service, admission, queue policy, and private envelope. Four Redis tests ignored | 2026-08-21T04:05:32Z |
| Affected shared-crate tests | PASS, 59 active tests | Authority, authorization core, PolicyIR, and generated messages | 2026-08-21T04:05:32Z |
| Pinned-container non-guest workspace | PASS, 163 active tests | Full Rust workspace. Nine SP1 and four Redis tests ignored | 2026-08-21T04:05:32Z |
| Pinned-container workspace Clippy | PASS | All non-guest targets with warnings denied | 2026-08-21T04:05:32Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-21T04:05:32Z |
| Side-effect import and logging search | PASS, no matches | Private-envelope production modules | 2026-08-21T04:05:32Z |
| Cargo lock inspection | PASS, no new package or version | Existing workspace Prost `0.14.4` selected directly | 2026-08-21T04:05:32Z |
| Private-material diff inspection | PASS | Synthetic private-looking values remain test-only | 2026-08-21T04:05:32Z |
| `git diff --check` and file-scope review | PASS | B-0018 patch hygiene | 2026-08-21T04:05:32Z |

## Privacy Checks

- PASS: Agent-facing outputs from this boundary contain no PolicyIR, relationship,
  path, Merkle opening, or stored caveat value.
- PASS: Domain, wire, reference, store, and error debug output redacts private fields,
  tokens, and filesystem paths.
- PASS: The only queue-facing value is a bounded opaque token derived from the public
  request commitment. It contains no private context or path.
- PASS: Production modules import no queue, network, witness, guest, or proof API and
  emit no log output.
- NOT APPLICABLE: No real transaction data, witness, proof, or Redis state is used.

## Risks and Private-Data Notes

- The later async RPC path must run synchronous filesystem and durability work away
  from Tokio request executors.
- The queue publisher must supply an authoritative live-reference set and a bounded
  grace cutoff to orphan cleanup.
- Cleanup failure cannot be reported as completed cancellation or terminal state.
- The worker must repeat expiry checks against trusted worker start time.
- The store targets the approved Unix host and Linux-container runtime. It does not
  provide a Windows storage implementation.

## Exact Next Action

Archive B-0018, reset the active baton to B-0019 for atomic BullMQ publication, and
commit the completed private-envelope slice.
