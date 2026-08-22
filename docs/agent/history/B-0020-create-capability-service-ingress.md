# Engineering Baton History

- Baton ID: B-0020
- State: REVIEW
- Milestone: M4, `CreateCapability` RPC and service ingress
- Owner: Codex
- Started UTC: 2026-08-22T05:46:21Z
- Last updated UTC: 2026-08-22T07:33:26Z
- Branch: `main`
- Worktree: B-0020 implementation, current-status updates, archive, and B-0021 reset
  are ready to commit on `main` after `0a011c0`. The branch remains two commits ahead
  of `origin/main` before this commit. B-0019 is archived at
  `docs/agent/history/B-0019-atomic-capability-job-publication.md`.

## Objective

Implement the bounded Tonic `CreateCapability` service boundary and runnable
`razield` ingress path. Apply the accepted request ceiling, use a trusted server
clock, run B-0017 admission, publish through B-0019, and return an authoritative
queued `ProofJob` without starting witness or proof work.

## Acceptance Criteria

- [x] Service configuration validates the owner-controlled bind address, Redis
      connection, queue prefix, and private-envelope root without logging credentials
      or private paths.
- [x] Startup reconciles stale private envelopes against an authoritative BullMQ live
      reference set before accepting requests.
- [x] The generated Tonic service applies a 2 MiB decoding ceiling before handler
      work and rejects malformed requests without queue or filesystem side effects.
- [x] `CreateCapability` derives whole-second UTC time from an injectable trusted
      server clock, then calls B-0017 admission before B-0019 publication.
- [x] A valid request returns an authoritative `QUEUED` `ProofJob` immediately with
      its deterministic ID and public timestamps. It does not wait for worker or
      proof execution.
- [x] Exact queued duplicates return the retained public job. Authorization-bound
      mutations fail before publication, and `agent_id` alone remains
      non-authoritative.
- [x] Admission, queue, storage, and cleanup failures map to stable generic Tonic
      statuses without private values or internal paths. Queued jobs use
      `PUBLIC_ERROR_CODE_UNSPECIFIED`. A later worker slice owns terminal public codes.
- [x] `GetCapability` and `CancelCapability` fail closed as unimplemented until their
      own baton supplies lifecycle behavior.
- [x] Focused handler tests, a live loopback and Redis test, affected workspace tests,
      strict Clippy, formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- `CreateCapability` returns after validation and durable queue publication. It never
  runs witness construction, guest execution, proving, verification, or capability
  packaging.
- Admission remains the only request-to-canonical-binding conversion boundary.
- The handler cannot derive authorization identity from `agent_id`.
- Redis stores only public queue state and opaque private-envelope references.
- The filesystem remains the sole owner of private request bytes.
- BullMQ remains authoritative for public job state and timestamps.
- Public errors stay generic. Detailed diagnostics require a later access-controlled
  audit boundary.
- No architecture invariant, public proof value, capability format, or PolicyIR
  semantic changes inside this baton.

## File Scope

- `services/razield/src/main.rs`
- `services/razield/src/service/`
- `services/razield/src/publication/` only for public-job reads and reconciliation
- `services/razield/src/private_envelope/store.rs` only for validated reference scans
- `services/razield/src/queue.rs` only for stricter configuration validation
- `services/razield/src/lib.rs`
- `services/razield/Cargo.toml`
- `Cargo.lock` only for already approved workspace selections
- `services/razield/tests/capability_service.rs`
- `scripts/test-bullmq-conformance`
- `scripts/bootstrap` only for unprivileged offline service access to a root-populated
  public Cargo cache
- `docker-compose.yml`
- `.env.example`
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Implemented `GetCapability` or `CancelCapability` lifecycle behavior
- Active-worker cancellation, progress transitions, or terminal result packaging
- Witness construction, guest execution, proof generation, or proof verification
- Snapshot service handlers or registry resolution
- TLS termination, public internet exposure, browser routes, or gateway behavior
- `verifierd`, ADR-0002, O-007, M5, M6, M7, or another scenario
- New packages, versions, toolchains, or proof runs without explicit approval

## Decisions

- Inherited: ADR-0013 fixes the Rust API and job-server split, asynchronous create
  behavior, private storage, BullMQ ownership, and generic public failures.
- Inherited: ADR-0014 fixes request identity, signature, timestamp, and deterministic
  job-ID rules.
- Inherited: B-0017 supplies pure signed request admission.
- Inherited: B-0018 supplies the strict owner-only private envelope and cleanup API.
- Inherited: B-0019 supplies stage-before-add publication, exact duplicate handling,
  bounded queue metadata, and conservative uncertain-outcome handling.
- Inherited: Tonic `0.14.6`, Tokio `1.53.1`, BullMQ `1.2.3`, Redis `8.2.8`, and current
  workspace pins are approved.
- Made: Set the 2 MiB request ceiling on generated
  `CapabilityServiceServer::max_decoding_message_size`, before handler execution.
- Made: Build startup liveness from a validated private-store reference scan and one
  exact official BullMQ lookup per reference. Do not load every queue job or use raw
  Redis. Queue errors, corrupt records, and mismatches fail startup without deletion.
- Made: Use a ten-minute orphan grace period. A normal publication takes seconds, and
  live jobs preserve their envelopes for any proof duration.
- Made: Require explicit bind address, Redis URL, queue prefix, and absolute private
  store root. Compose binds on the internal network and publishes no host port.
- Made: Select Tonic's approved `router` feature for `Server::add_service`. Cargo locks
  its transitive Axum `0.8.9`, Axum Core `0.5.6`, and Matchit `0.8.4` packages. Raziel
  does not import those crates directly.
- Made: Fail closed on completed or failed BullMQ records until the worker slice stores
  and validates the bound public capability or typed terminal result.

## Open Questions and Blockers

- No blocker remains.
- The generated service trait requires Create, Get, and Cancel methods. Get and Cancel
  will return generic `UNIMPLEMENTED` statuses in this baton.
- Compose now supplies the listener address, queue prefix, persistent owner-only
  private-envelope volume, and an internal TCP health check. It publishes no host port.
- Root-run container bootstrap grants read and directory-search access to the shared
  public Cargo cache. This lets the unprivileged offline service compile from the
  cache without making the private envelope volume public.
- The later lifecycle baton owns `GetCapability`, queued cancellation, active
  cancellation, progress, and terminal transitions.
- ADR-0002 and O-007 remain open before protected-request gateway integration.

## Work Completed

- B-0017 completed pure signed request admission.
- B-0018 completed private-envelope encoding, storage, deletion, and orphan cleanup.
- B-0019 completed atomic publication coordination and live BullMQ conformance.
- Reconciled a clean worktree at `0a011c0` and claimed B-0020.
- Inspected the generated Tonic server trait and its pre-handler decoding ceiling.
- Inspected the service shell, Compose networks, private cleanup contract, BullMQ exact
  job and state APIs, public Protobuf states, and timestamp accessors.
- Chose a store-driven startup scan so memory is bounded by the private-store footprint
  rather than the total Redis queue.
- Added strict service configuration with redacted Redis and private-path diagnostics.
- Added injectable whole-second trusted time conversion with protocol-range checks.
- Added a sorted, fail-closed private-store reference scan shared with orphan cleanup.
- Added startup reconciliation through exact BullMQ job and state lookups. It validates
  every live envelope before removing old missing or terminal records.
- Added the bounded Tonic handler and runnable listener. Create uses trusted time,
  admission, atomic publication, and authoritative BullMQ timestamps.
- Added generic Tonic failure mapping, closed Get and Cancel handlers, a real loopback
  request-ceiling test, and a live Redis service test.
- Added the persistent Compose private volume, root-only initializer, internal health
  check, explicit service settings, and an unprivileged offline cache fix.
- Confirmed the live seven-case Redis conformance gate and isolated Compose service
  health check.
- Rejected embedded Redis URL whitespace and out-of-range BullMQ timestamps.
- Kept terminal records closed until a later worker result reader can return a bound
  capability or typed public failure.
- Updated the README, architecture, and technical specification to match the runnable
  Create ingress and its pending lifecycle work.

## Validation Plan

- Test strict configuration and redacted debug output.
- Test the 2 MiB Tonic ceiling and malformed-request rejection before side effects.
- Test trusted-clock admission, valid create, exact duplicate, changed fields, and
  generic failure mapping.
- Run a loopback Tonic client against the digest-pinned Redis service and prove that
  create returns `QUEUED` without a worker.
- Confirm startup cleanup preserves every live queue reference and removes only stale
  unreferenced envelopes after the accepted grace period.
- Run affected service and workspace tests in the pinned toolchain.
- Run strict Clippy, repository formatting, `git diff --check`, dependency inspection,
  and private-data scans.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0019 archive and commit `0a011c0` | PASS | Atomic capability job publication | 2026-08-22T05:46:21Z |
| Git and baton reconciliation | PASS | Clean `main` at `0a011c0`, ahead two | 2026-08-22T05:46:21Z |
| Tonic, BullMQ, store, and Compose inspection | PASS | Exact B-0020 interfaces and file map | 2026-08-22T05:46:21Z |
| `cargo test -p razield --lib service::tests --no-fail-fast` | PASS, 3 tests | Configuration redaction and trusted clock | 2026-08-22T06:03:51Z |
| `cargo test -p razield --lib private_envelope::tests --no-fail-fast` | PASS, 14 tests | Validated scans and prior store behavior | 2026-08-22T06:03:51Z |
| `cargo test -p razield --lib publication::tests --no-fail-fast` | PASS, 11 tests | Publication and startup reconciliation | 2026-08-22T06:03:51Z |
| `cargo test -p razield --lib service:: --no-fail-fast` | PASS, 10 tests | Configuration, clock, handler, edge cases, and loopback ceiling | 2026-08-22T07:13:54Z |
| `cargo test -p razield --lib publication:: --no-fail-fast` | PASS, 12 tests | Publication, reconciliation, and terminal fail-closed behavior | 2026-08-22T07:13:54Z |
| Isolated `raziel-b0020-smoke` Compose run | PASS | Redis healthy, private init complete, `razield` healthy | 2026-08-22T07:13:54Z |
| Approved `cargo fetch --locked` | PASS | Populated Docker cache with locked Tonic router graph, with no version change | 2026-08-22T07:33:26Z |
| `cargo test -p razield --all-targets --no-fail-fast` | PASS, 48 active tests | All service, library, binary, and admission targets. Seven live tests ignored here | 2026-08-22T07:33:26Z |
| Pinned-container non-guest workspace | PASS, 186 active tests | Full Rust workspace. Sixteen proof and live-service tests ignored | 2026-08-22T07:33:26Z |
| Pinned-container workspace Clippy | PASS | All non-guest targets with warnings denied | 2026-08-22T07:33:26Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-22T07:33:26Z |
| `sh -n scripts/bootstrap scripts/test-bullmq-conformance` | PASS | Changed shell scripts | 2026-08-22T07:33:26Z |
| `./scripts/compose --profile services config --quiet` | PASS | Service composition | 2026-08-22T07:33:26Z |
| `./scripts/test-bullmq-conformance` | PASS, 7 live cases | Final Redis lifecycle, publication, service ingress, and restart | 2026-08-22T07:33:26Z |
| Dependency-tree inspection | PASS | Tonic router stays in `razield`. Guest stays transport-free. No prover or witness enters the service | 2026-08-22T07:33:26Z |
| Production logging and private-term scan | PASS | No service logging call or private value in a public response | 2026-08-22T07:33:26Z |
| Current-state stale-claim scan | PASS | README, architecture, and specification match B-0020 | 2026-08-22T07:33:26Z |
| `git diff --check` and file-scope review | PASS | B-0020 patch hygiene and bounded scope | 2026-08-22T07:33:26Z |

## Privacy Checks

- PASS: Tonic responses and status details expose no private request field, opaque
  envelope reference, Redis credential, or filesystem path.
- PASS: Startup reconciliation emits no envelope token, stored value, or path.
- PASS: Production service, publication, and private-store code contains no logging
  call. Debug implementations redact the Redis URL, private root, job ID, and opaque
  envelope reference.
- PASS: The authorization guest dependency graph contains no Tonic, Axum, Tokio,
  HTTP, Hyper, Tower, BullMQ, or Redis package.
- NOT APPLICABLE: No real transaction data, witness, or proof is planned for this
  baton.

## Risks and Private-Data Notes

- Tonic decodes a request before invoking a service method. The message-size cap must
  be set on the generated server wrapper, not only checked inside the handler.
- Filesystem durability and startup cleanup are blocking operations. The async service
  must keep them away from Tokio request executors.
- The BullMQ binding emits custom job IDs at debug level. Service logging must filter
  that target before debug output is active.
- Startup cleanup needs an authoritative complete live-reference set. A partial queue
  scan could delete private input for a retained job.
- An uncertain B-0019 publication must map to retryable service unavailability. It
  cannot be reported as queued or as a confirmed rejection.
- Cargo reports an existing future-compatibility warning for
  `proc-macro-error2 2.0.1`. Strict Clippy passes, and B-0020 does not select that
  package.

## Exact Next Action

Archive B-0020, reset the current baton to B-0021 for the BullMQ proof-supervisor claim
and private-envelope handoff, then commit the completed service-ingress slice.
