# Engineering Baton History

- Baton ID: B-0019
- State: REVIEW
- Milestone: M4, atomic capability job publication
- Owner: Codex
- Started UTC: 2026-08-21T10:19:57Z
- Last updated UTC: 2026-08-22T05:24:38Z
- Branch: `main`
- Worktree: B-0019 implementation, current-status updates, archive, and B-0020 reset
  are ready to commit on `main` after `1bf85d4`. B-0018 is archived at
  `docs/agent/history/B-0018-private-capability-request-envelope.md`.

## Objective

Implement the bounded coordinator between admitted capability requests, the private
envelope store, and BullMQ publication. Stage the private envelope first, publish one
deterministic single-attempt job containing only its opaque reference and safe public
metadata, and return the retained queued job for exact duplicates.

## Acceptance Criteria

- [x] The coordinator accepts only a B-0017 admitted request and a configured B-0018
      private store.
- [x] Private-envelope durability completes before BullMQ publication begins.
- [x] The BullMQ payload contains a versioned opaque envelope reference and safe
      public metadata. It contains no request context, key, signature, nonce, path,
      PolicyIR, relationship, witness, or proof input.
- [x] The exact lowercase request commitment remains the custom job ID. Attempts,
      started attempts, stalled count, concurrency, and terminal retention match
      ADR-0013.
- [x] Exact duplicate publication returns one retained job and never replaces the
      existing private envelope.
- [x] Confirmed queue rejection or confirmed absence removes a newly staged envelope.
      An ambiguous queue outcome preserves it for reconciliation. A failed cleanup is
      surfaced and cannot be reported as queued.
- [x] Publication returns a queued public job without starting witness or proof work.
- [x] Focused unit tests, live Redis publication tests, affected workspace tests,
      strict Clippy, formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- Admission remains pure and runs before this coordinator.
- The filesystem stores private request bytes. Redis never stores those bytes or a
  private filesystem path.
- BullMQ remains authoritative for waiting, active, completed, and failed job state.
- Queue semantics come from `bullmq-official`. Do not add raw Redis queue logic or a
  filesystem spool.
- The queue job uses one proof attempt and cannot start a second proof after a stall.
- No Tonic listener, witness construction, guest execution, proof generation, proof
  verification, or capability packaging occurs in this boundary.

## File Scope

- `services/razield/src/queue.rs`
- `services/razield/src/private_envelope/` only for a narrow publisher handoff
- `services/razield/src/publication/`
- `services/razield/src/lib.rs`
- `services/razield/tests/capability_publication.rs`
- `services/razield/Cargo.toml`
- `Cargo.lock` only for already approved workspace selections
- `scripts/test-bullmq-conformance`
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Tonic handlers, listeners, interceptors, TLS, or public transport mapping
- Get or Cancel RPC behavior and active-worker cancellation
- BullMQ worker claims, lock renewal, progress, or terminal transitions
- Trusted policy, snapshot, delegation, key, or program registry resolution
- Witness construction, guest execution, proof generation, proof verification, or
  capability artifact issuance
- ADR-0002, O-007, M5, M6, M7, or another scenario
- New packages, versions, toolchains, or proof runs without explicit approval

## Decisions

- Inherited: ADR-0013 selects `bullmq-official` `1.2.3`, Redis `8.2.8`, deterministic
  custom IDs, one proof attempt, zero stalled retries, and bounded terminal retention.
- Inherited: ADR-0014 assigns the lowercase request commitment as the BullMQ custom
  job ID.
- Inherited: B-0017 authenticates and validates requests before publication.
- Inherited: B-0018 atomically stores a strict private envelope and returns an opaque
  queue token.
- Inherited: No new package or version is approved.
- Made: The coordinator will fetch the authoritative retained Redis record after
  `Queue::add`. The binding returns the existing ID for a duplicate but leaves the
  caller's candidate payload in the returned in-memory `Job`.
- Made: The typed queue payload contains only its format version and opaque private
  envelope reference. The BullMQ custom ID remains the separate exact request
  commitment.
- Made: A publisher that created the private envelope can add the Redis job. A retry
  that finds an existing envelope never adds or deletes it. It returns the matching
  retained record or a typed uncertain result during a cross-process race.
- Made: Confirmed rejection or confirmed post-add absence rolls back a newly created
  envelope. An unreadable or mismatched retained queue state preserves the envelope
  for audit and reconciliation.
- Made: Terminal records retain for 24 hours with a 100-record count ceiling and a
  100-record eviction-pass ceiling. The BullMQ wire field is 86,400 seconds. The
  pinned Rust type documents milliseconds, but its embedded finish script multiplies
  the field by 1,000 against millisecond timestamps.
- Made: `serde` `1.0.229` and `serde_json` `1.0.151` are direct selections of the
  already approved workspace pins. No package or version was added.

## Open Questions and Blockers

- No blocker remains for B-0019.
- `bullmq-official` supplies the required add, custom-ID, retained-job lookup, size
  limit, retention, and global-concurrency APIs under the approved pin.
- The service startup path must run orphan reconciliation with an authoritative live
  queue-reference set before accepting traffic.
- The later RPC slice still owns the 2 MiB ingress ceiling, trusted clock, public
  error mapping, and Tonic status mapping.
- The later worker slice owns expiry rechecks, claims, progress, cancellation, and
  proof isolation.
- ADR-0002 and O-007 remain open before protected-request gateway integration.

## Work Completed

- B-0017 completed pure signed request admission.
- B-0018 completed the versioned owner-only private envelope and store.
- Reconciled a clean worktree and inspected the official `1.2.3` publication API,
  duplicate Lua path, existing conformance tests, admission type, and private store.
- Claimed the coordinator, typed payload, focused test, and live Redis script file map.
- Added a strict two-field public payload with format version and opaque envelope
  reference. Debug output redacts the reference and exact job ID.
- Added the publication coordinator with stage-before-add ordering, authoritative
  retained-record validation, exact duplicate handling, typed failure classes,
  confirmed-failure cleanup, and conservative uncertain-outcome handling.
- Added one-attempt publication options, a 256-byte payload ceiling, global
  concurrency one, zero stalled retries, and 24-hour or 100-record terminal
  retention for both job and worker paths.
- Extended the live conformance suite with exact waiting-state publication,
  duplicate retention, payload privacy, and confirmed Redis rejection cleanup.
- Added nine focused publication tests for ordering, strict decoding, redaction,
  duplicate behavior, rollback, cleanup failure, missing or mismatched records, and
  ambiguous queue outcomes.
- Updated current-state documentation to separate the completed publication boundary
  from pending RPC, startup-reconciliation, worker, proof, and verifier work.

## Validation Plan

- Run focused coordinator tests with a test queue abstraction only where the official
  binding cannot provide a deterministic local unit seam.
- Run live Redis tests for publish, duplicate, failure cleanup, payload privacy, and
  retained queued state.
- Confirm no worker, witness, guest, or proof API enters the coordinator.
- Run affected service and workspace tests in the pinned toolchain.
- Run strict Clippy, repository formatting, `git diff --check`, dependency inspection,
  and private-data scans.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0018 archive | PASS | Private capability request envelope | 2026-08-21T04:05:32Z |
| Git and baton reconciliation | PASS | Clean `main` at `1bf85d4`, ahead one | 2026-08-21T10:19:57Z |
| BullMQ `1.2.3` source/API inspection | PASS | Add, duplicate, lookup, options, errors | 2026-08-21T10:19:57Z |
| `cargo test -p razield --no-fail-fast` | PASS, 34 active tests | Admission, envelope, queue policy, and publication. Six live tests ignored | 2026-08-22T05:24:38Z |
| `./scripts/test-bullmq-conformance` | PASS, 6 live cases | Lifecycle, stalled failure, publication, cleanup, and restart | 2026-08-22T05:24:38Z |
| Pinned-container non-guest workspace | PASS, 172 active tests | Full Rust workspace. Nine SP1 and six live Redis tests ignored | 2026-08-22T05:24:38Z |
| Pinned-container workspace Clippy | PASS | All non-guest targets with warnings denied | 2026-08-22T05:24:38Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-22T05:24:38Z |
| Shell syntax check | PASS | `scripts/test-bullmq-conformance` | 2026-08-22T05:24:38Z |
| Production dependency scan | PASS, no matches | No prover, witness, SDK, executor, or Gnark dependency in `razield` | 2026-08-22T05:24:38Z |
| Public-payload and debug inspection | PASS | Private fields remain absent or test-only | 2026-08-22T05:24:38Z |
| Cargo lock inspection | PASS, no new package or version | Existing workspace Serde pins selected directly | 2026-08-22T05:24:38Z |
| `git diff --check` and file-scope review | PASS | B-0019 patch hygiene | 2026-08-22T05:24:38Z |
| Secret-pattern inspection | PASS | Only documented policy-key path and synthetic test credential matched | 2026-08-22T05:24:38Z |
| Host workspace Clippy diagnostic | FAIL, missing host Go executable | Superseded by passing pinned-container Clippy | 2026-08-22T05:24:38Z |

## Privacy Checks

- PASS: Agent-facing results contain no PolicyIR, relationship tuple, satisfying path,
  Merkle opening, caveat value, witness field, or proof input.
- PASS: Redis stores only the exact public job ID, payload format version, opaque
  envelope reference, queue options, and queue state. It receives no private request
  field or filesystem path.
- PASS: Payload, publisher, result, store, and error debug output redact opaque
  references, job IDs, credentials, paths, and private request values.
- PASS: Confirmed cleanup and uncertain-publication paths return typed errors without
  private values. Cleanup failure cannot be reported as queued.
- NOT APPLICABLE: No real transaction data, witness, proof, capability, verifier
  registry, or prover connection enters this baton.

## Risks and Private-Data Notes

- Duplicate handling spans two durable systems. The coordinator must distinguish a
  retained job from a failed first publication without replacing or orphaning private
  data.
- A queue success followed by an unreadable envelope must fail later worker claims.
  The publication path must not report success before both durable transitions hold.
- Queue or cleanup errors cannot include Redis credentials, opaque tokens, private
  paths, request fields, or stored bytes.
- Ambiguous publication preserves a private envelope and requires startup
  reconciliation. Deleting it before queue state is authoritative could orphan an
  accepted job.
- `bullmq-official` emits a debug-level custom job ID internally after add. The later
  service logging slice must filter that target before enabling debug logs.
- Terminal age uses the BullMQ wire unit proven by its embedded Lua script, not the
  contradictory unit text on the Rust `KeepJobs` type.
- Live Redis tests use the accepted digest-pinned service and do not start proof work.

## Exact Next Action

Archive B-0019, reset the active baton to B-0020 for the bounded `CreateCapability`
RPC and service-ingress boundary, and commit the completed publication slice.
