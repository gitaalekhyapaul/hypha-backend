# Current Engineering Baton

- Baton ID: B-0020
- State: READY
- Milestone: M4, `CreateCapability` RPC and service ingress
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-22T05:24:38Z
- Branch: `main`
- Worktree: B-0019 is archived at
  `docs/agent/history/B-0019-atomic-capability-job-publication.md`. Its implementation,
  current-status updates, archive, and this B-0020 reset are ready to commit after
  `1bf85d4`. Verify Git status and HEAD before claiming B-0020.

## Objective

Implement the bounded Tonic `CreateCapability` service boundary and runnable
`razield` ingress path. Apply the accepted request ceiling, use a trusted server
clock, run B-0017 admission, publish through B-0019, and return an authoritative
queued `ProofJob` without starting witness or proof work.

## Acceptance Criteria

- [ ] Service configuration validates the owner-controlled bind address, Redis
      connection, queue prefix, and private-envelope root without logging credentials
      or private paths.
- [ ] Startup reconciles stale private envelopes against an authoritative BullMQ live
      reference set before accepting requests.
- [ ] The generated Tonic service applies a 2 MiB decoding ceiling before handler
      work and rejects malformed requests without queue or filesystem side effects.
- [ ] `CreateCapability` derives whole-second UTC time from an injectable trusted
      server clock, then calls B-0017 admission before B-0019 publication.
- [ ] A valid request returns an authoritative `QUEUED` `ProofJob` immediately with
      its deterministic ID and public timestamps. It does not wait for worker or
      proof execution.
- [ ] Exact duplicates return the retained public job. Authorization-bound mutations
      fail before publication, and `agent_id` alone remains non-authoritative.
- [ ] Admission, queue, storage, and cleanup failures map to stable generic Tonic
      statuses and public error codes without private values or internal paths.
- [ ] `GetCapability` and `CancelCapability` fail closed as unimplemented until their
      own baton supplies lifecycle behavior.
- [ ] Focused handler tests, a live loopback and Redis test, affected workspace tests,
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
- New service configuration and RPC modules under `services/razield/src/`
- `services/razield/src/publication/` only for a narrow public-job read model
- `services/razield/src/private_envelope/` only for startup reconciliation wiring
- `services/razield/src/queue.rs` only for official BullMQ read helpers
- `services/razield/src/lib.rs`
- `services/razield/Cargo.toml`
- `Cargo.lock` only for already approved workspace selections
- Focused `razield` service tests
- Service Compose configuration only if required by the inspected listener path
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
  workspace pins are approved. No new package or version is approved.
- Made: None.

## Open Questions and Blockers

- No blocker is recorded before inspection.
- Inspect the generated `CapabilityService` trait and official BullMQ job-query APIs
  before fixing the handler and startup-reconciliation interfaces.
- Confirm the existing Compose service network and bind-address pattern before
  changing listener configuration.
- The later lifecycle baton owns `GetCapability`, queued cancellation, active
  cancellation, progress, and terminal transitions.
- ADR-0002 and O-007 remain open before protected-request gateway integration.

## Work Completed

- B-0017 completed pure signed request admission.
- B-0018 completed private-envelope encoding, storage, deletion, and orphan cleanup.
- B-0019 completed atomic publication coordination and live BullMQ conformance.
- No B-0020 implementation has started.

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
| B-0019 archive | PASS | Atomic capability job publication | 2026-08-22T05:24:38Z |
| B-0020 implementation | NOT AVAILABLE | Not started | 2026-08-22T05:24:38Z |

## Privacy Checks

- NOT STARTED: Tonic responses and status details must expose no private request field,
  opaque envelope reference, Redis credential, or filesystem path.
- NOT STARTED: Startup reconciliation must not log envelope tokens or stored values.
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

## Exact Next Action

Inspect the generated `CapabilityService` server API, `razield` process shell, Compose
service configuration, BullMQ public-job reads, and B-0018 cleanup interface. Then
claim B-0020 with the exact RPC, runtime, and test file map.
