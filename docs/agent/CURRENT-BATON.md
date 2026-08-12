# Current Engineering Baton

- Baton ID: B-0018
- State: READY
- Milestone: M4, private capability request envelope
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-12T09:27:20Z
- Branch: `main`
- Worktree: B-0017 implementation, archive, and current-status changes are ready to
  commit on `main` after `3673b9c`. B-0017 is archived at
  `docs/agent/history/B-0017-capability-issuance-admission.md`.

## Objective

Implement the bounded private-envelope boundary used after successful capability
admission and before BullMQ publication. Encode the validated canonical request in a
versioned internal Protobuf envelope, persist it through an owner-only atomic file
transition, and return an opaque reference for the later queue publisher.

## Acceptance Criteria

- [ ] The envelope contains only validated post-admission fields needed by the proof
      worker and never uses its Protobuf bytes as a commitment input.
- [ ] Serialization and storage apply fixed byte ceilings before durable publication.
- [ ] Directories use owner-only access and published envelope files use mode `0600`.
- [ ] Creation uses a private temporary file, data sync, atomic rename, and directory
      sync without replacing an existing job envelope.
- [ ] Reads reject symbolic links, non-regular files, wrong modes, malformed bytes,
      wrong versions, and oversized files.
- [ ] Deletion supports later queued-job cancellation and orphan cleanup without
      exposing private paths or values.
- [ ] Private structs, errors, and debug output redact request context, nonces, keys,
      signatures, commitments, and filesystem details.
- [ ] Focused tests, affected workspace tests, strict Clippy, formatting, and diff
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
- Made: None.

## Open Questions and Blockers

- No blocker is recorded before inspection.
- The exact internal Protobuf placement and existing owner-only file helpers need
  inspection before claiming this baton.
- RPC ingress ceilings, queue publication, worker-time expiry validation, and public
  error mapping remain later boundaries.
- The ADR-0014 TypeScript thumbprint vector remains pending for the agent SDK slice.
- ADR-0002 and O-007 remain open before protected-request gateway integration.

## Work Completed

- B-0017 implemented and tested pure capability issuance admission.
- No B-0018 implementation has started.

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
| B-0018 implementation | NOT AVAILABLE | Not started | 2026-08-12T09:27:20Z |

## Privacy Checks

- NOT STARTED: Envelope and store types must redact all private request material.
- NOT STARTED: Queue-visible data must contain no private context or filesystem path.
- NOT APPLICABLE: No real transaction data is planned for this baton.

## Risks and Private-Data Notes

- Atomic rename alone does not make file creation safe. Mode, link, replacement,
  durability, and directory ownership checks must hold together.
- Cleanup failure cannot be reported as a completed cancellation or terminal job.
- An internal envelope schema can become an accidental public contract if generated
  exports are not scoped carefully.

## Exact Next Action

After the B-0017 commit, inspect existing private-file and generated-Protobuf patterns.
Then claim B-0018 with the exact module and schema file map before editing.
