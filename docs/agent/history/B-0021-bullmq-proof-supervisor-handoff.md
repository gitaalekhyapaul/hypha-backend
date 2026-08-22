# Engineering Baton History

- Baton ID: B-0021
- State: REVIEW
- Milestone: M4, BullMQ proof-supervisor claim and private-envelope handoff
- Owner: Codex
- Started UTC: 2026-08-22T07:38:08Z
- Last updated UTC: 2026-08-22T08:56:28Z
- Branch: `main`
- Worktree: B-0021 implementation, current-status updates, archive, and B-0022 reset
  are ready to commit on `main` after `598ba02`. The branch remains three commits
  ahead of `origin/main` before this commit. B-0020 is archived at
  `docs/agent/history/B-0020-create-capability-service-ingress.md`.

## Objective

Implement the bounded proof-supervisor claim boundary. Use the official BullMQ worker
to claim one job, validate its public payload, read its private envelope only after
the claim, and pass a validated private request to an injectable one-shot executor
contract. Do not run a real proof or publish a terminal capability in this baton.

## Acceptance Criteria

- [x] Supervisor configuration validates Redis, queue prefix, and private-envelope
      root without exposing credentials or private paths. Worker timing remains a
      fixed code policy rather than an environment-selected proof parameter.
- [x] The official BullMQ worker owns claims, locks, lock renewal, one-job
      concurrency, stalled failure, and terminal retention.
- [x] The supervisor validates the strict public payload before private storage access.
- [x] The supervisor reads and commitment-checks the private envelope only after a
      successful BullMQ claim.
- [x] An injectable one-shot executor contract receives only validated private input
      and bounded owner configuration.
- [x] Invalid payloads, missing or corrupt envelopes, cancellation, and executor
      failures produce generic public state and typed private failure classes.
- [x] Progress contains safe phase values only. It contains no request context,
      policy branch, relationship, witness, path, opening, or proof bytes.
- [x] Focused unit tests, live Redis worker tests, affected workspace tests, strict
      Clippy, formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- The supervisor remains a process separate from the responsive `razield` API.
- BullMQ remains the sole owner of claims, locks, lock renewal, stalled jobs, and
  terminal retention.
- The private filesystem remains a bounded data handoff. It is not a queue.
- The worker cannot use request data to select an ELF, setup archive, proof mode,
  resource profile, registry generation, or external endpoint.
- A worker claim cannot expose the private envelope through Redis, Tonic, progress,
  standard logs, or debug output.
- No hosted prover or SP1 network client receives private input.
- No architecture invariant, public proof value, capability format, or PolicyIR
  semantic changes occur inside this baton.

## File Scope

- `services/razield/src/worker/`
- `services/razield/src/queue.rs` only for shared worker policy
- `services/razield/src/private_envelope/` only for the post-claim handoff
- `services/razield/src/publication/` only for shared payload and public state types
- `services/razield/src/service/config.rs` and `service/mod.rs` only to share the
  existing owner-configuration validators
- `services/razield/src/lib.rs`
- `services/razield/Cargo.toml`
- `Cargo.lock` only for approved workspace selections
- `services/razield/tests/` for focused and live worker checks
- `scripts/test-bullmq-conformance`
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Real SP1 execution or Plonk proof generation
- Witness construction and registry-backed authorization input assembly
- Final capability packaging or terminal success publication
- Production proof-supervisor binary or Compose service before terminal result
  validation exists
- `GetCapability` and `CancelCapability` RPC behavior
- Concrete child-process cancellation and proof cgroup implementation
- `verifierd`, verifier worker, ADR-0002, O-007, M5, M6, or M7
- New packages, versions, toolchains, or proof runs without explicit approval

## Decisions

- Inherited: ADR-0013 fixes the separate Rust API and proof-supervisor processes,
  official BullMQ ownership, one attempt, one-job concurrency, zero stalled retries,
  and generic public errors.
- Inherited: B-0018 supplies the strict owner-only private envelope.
- Inherited: B-0019 supplies the strict public payload and atomic publication.
- Inherited: B-0020 supplies runnable Create ingress and startup reconciliation.
- Inherited: BullMQ `1.2.3`, Redis `8.2.8`, Tokio `1.53.1`, Tonic `0.14.6`, and current
  workspace pins are approved.
- Made: Keep B-0021 library-only. BullMQ maps every successful processor return to a
  completed job, so no production worker is wired before a strict proof-capability
  result exists.
- Made: Require an unconstructable crate-owned activation token at supervisor startup.
  Tests can exercise the library plumbing, but external binaries cannot activate the
  placeholder path.
- Made: Validate the exact persisted BullMQ job policy after claim. BullMQ omits
  publisher-only `job_id` and `size_limit` fields from stored options, so identity and
  payload size use their authoritative top-level values.
- Made: Treat BullMQ's `Stalled` event as the lock-loss cancellation signal. The
  pinned Lua path moves the job to waiting and records deferred failure before it
  emits the event. One-job concurrency prevents the next terminal claim until the
  current processor handles cancellation and cleanup.
- Made: Recheck the full 75-minute admission window at trusted worker start. Bound
  execution to one hour, then allow 30 seconds for executor termination.
- Made: Publish `EXECUTING` only. The real isolated executor will own the later
  transition to `PROVING` once proof execution exists.
- Made: Use BullMQ global concurrency as the cross-process coordination boundary. Do
  not add a custom Redis or filesystem lease.
- Made: Keep B-0020 startup reconciliation responsible for valid terminal orphan
  cleanup after its grace period. Invalid public job metadata cannot authorize a
  private-file read or deletion.

## Open Questions and Blockers

- No blocker prevents this bounded implementation.
- Real proof execution remains blocked outside B-0021 by missing witness construction,
  one-shot process isolation, and strict terminal capability validation.
- Immediate renewal-loop notification is not public in BullMQ `1.2.3`. The approved
  stalled event supplies a later signal before the next claim can terminally fail the
  job under one-job concurrency.
- Real witness construction is the next feature slice. It needs no new external
  dependency and does not run SP1.

## Work Completed

- B-0020 completed and archived the bounded Create ingress at
  `docs/agent/history/B-0020-create-capability-service-ingress.md`.
- Reconciled a clean worktree at `598ba02` and claimed B-0021.
- Inspected the pinned BullMQ processor, cancellation, progress, completion, stalled,
  and lock-renewal paths. No dependency addition is needed.
- Fixed the handoff boundary: validated private envelope, trusted whole-second worker
  start time, fixed proof deadline, and cancellation control. It carries no queue job,
  JSON, path, reference, raw Protobuf, PolicyIR, witness, ELF, or Redis configuration.
- Implemented validated supervisor configuration with redacted debug output and fixed
  BullMQ worker timing.
- Implemented exact claimed-job validation before private access. The checks cover
  name, identity, payload ceiling, payload schema, persisted options, attempts,
  parentage, delay, repeat state, and deferred failure.
- Implemented the post-claim private handoff, trusted-time window recheck, closed
  progress, one-hour deadline, cancellation, termination grace, and ordered envelope
  deletion.
- Implemented the official BullMQ worker wrapper, stalled-event cancellation bridge,
  close lifecycle, active-work confirmation, and fail-closed placeholder result.
- Added a crate-owned activation token. No production binary can start the placeholder
  supervisor.
- Added one live Redis worker case to the existing conformance runner. No second test
  harness or new dependency was added.
- Completed an independent production review. Fixed placeholder activation, terminal
  cleanup ownership, shutdown confirmation, singleton coordination, exact job-policy
  validation, validity-window recheck, deadline enforcement, and phase timing before
  the final gates.
- Updated README, architecture, and technical-spec status with the implemented
  behavior and the remaining production gaps.

## Validation Plan

- Test payload rejection before private-store reads.
- Test private-envelope access only inside an official BullMQ processor claim.
- Test one-job concurrency, progress redaction, cancellation delivery, and zero proof
  retry against the pinned Redis service.
- Test executor handoff, typed failure, trusted start time, fixed deadline, and active
  cancellation with a deterministic fake executor. Do not run SP1.
- Run affected service and workspace tests in the pinned toolchain.
- Run strict Clippy, repository formatting, dependency inspection, privacy scans, and
  `git diff --check`.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0020 archive | PASS | Bounded CreateCapability service ingress | 2026-08-22T07:33:26Z |
| Git and baton reconciliation | PASS | Clean `main` at `598ba02`, ahead three | 2026-08-22T07:38:08Z |
| Pinned BullMQ source audit | PASS | Exact worker API and stalled ordering fixed | 2026-08-22T08:10:06Z |
| `cargo test -p razield --lib worker::tests --no-fail-fast` | RED AS EXPECTED | Missing B-0021 worker API after fixture corrections | 2026-08-22T08:10:06Z |
| `cargo test -p razield --lib worker::tests --no-fail-fast` | PASS | 12 focused cases passed, one live case ignored | 2026-08-22T08:41:00Z |
| `cargo clippy -p razield --all-targets -- -D warnings` | PASS | Strict service lint after review fixes | 2026-08-22T08:42:00Z |
| `./scripts/test-bullmq-conformance` | PASS | Eight pinned live Redis cases, including supervisor claim and cleanup | 2026-08-22T08:51:00Z |
| `cargo fmt --all` | PASS | Rust sources formatted | 2026-08-22T08:53:00Z |
| `cargo test -p razield --all-targets --no-fail-fast` | PASS | 60 active package tests, eight live cases ignored in this run | 2026-08-22T08:54:00Z |
| `cargo clippy -p razield --all-targets -- -D warnings` | PASS | Fresh strict service lint | 2026-08-22T08:54:00Z |
| Pinned-container non-guest workspace | PASS, 198 active tests | Full Rust workspace. Seventeen proof and live-service tests ignored | 2026-08-22T08:55:00Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-22T08:56:00Z |
| Production logging and private-term scan | PASS | No logging macro. Only validated and redacted Redis configuration matched | 2026-08-22T08:56:28Z |
| `git diff --check` and file-scope review | PASS | B-0021 patch hygiene and bounded scope | 2026-08-22T08:56:28Z |

## Privacy Checks

- PASS: Progress uses a closed format-version and phase object. Failure
  state uses three fixed strings.
- PASS: Supervisor, configuration, request, and control debug output omit
  Redis credentials, private paths, and envelope fields.
- PASS: Production supervisor modules contain no logging macro. Public Redis state
  receives no request context, relationship, witness, opening, or proof bytes.
- NOT APPLICABLE: This baton does not run a real proof or use transaction data.

## Risks and Private-Data Notes

- BullMQ starts the processor only after it grants the active-job lock. Private reads
  outside that processor could bypass queue ownership.
- A private envelope can be corrupt, swapped, linked, missing, or commitment-mismatched
  after publication. The supervisor must use the strict B-0018 read path.
- Progress and terminal errors are public Redis values. They need closed typed schemas,
  fixed ceilings, and redacted debug output.
- A task spawn does not isolate a proof crash or OOM. This baton stops at an executor
  contract and leaves real process isolation to the next proof-execution slice.

## Exact Next Action

Archive B-0021, reset the current baton, commit the scoped feature, and claim the
witness-construction slice.
