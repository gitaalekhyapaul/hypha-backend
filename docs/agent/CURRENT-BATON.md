# Current Engineering Baton

- Baton ID: B-0019
- State: READY
- Milestone: M4, atomic capability job publication
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-21T04:05:32Z
- Branch: `main`
- Worktree: B-0018 is archived at
  `docs/agent/history/B-0018-private-capability-request-envelope.md`. B-0019 has not
  started. Verify Git status and HEAD before claiming it.

## Objective

Implement the bounded coordinator between admitted capability requests, the private
envelope store, and BullMQ publication. Stage the private envelope first, publish one
deterministic single-attempt job containing only its opaque reference and safe public
metadata, and return the retained queued job for exact duplicates.

## Acceptance Criteria

- [ ] The coordinator accepts only a B-0017 admitted request and a configured B-0018
      private store.
- [ ] Private-envelope durability completes before BullMQ publication begins.
- [ ] The BullMQ payload contains a versioned opaque envelope reference and safe
      public metadata. It contains no request context, key, signature, nonce, path,
      PolicyIR, relationship, witness, or proof input.
- [ ] The exact lowercase request commitment remains the custom job ID. Attempts,
      started attempts, stalled count, concurrency, and terminal retention match
      ADR-0013.
- [ ] Exact duplicate publication returns one retained job and never replaces the
      existing private envelope.
- [ ] Queue publication failure removes a newly staged envelope. A failed cleanup is
      surfaced and cannot be reported as queued.
- [ ] Publication returns a queued public job without starting witness or proof work.
- [ ] Focused unit tests, live Redis publication tests, affected workspace tests,
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
- New capability-publication modules under `services/razield/src/`
- `services/razield/src/lib.rs`
- Focused publication tests under `services/razield/tests/` or the new module
- `Cargo.lock` only for already approved workspace selections
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
- Made: None.

## Open Questions and Blockers

- No blocker is recorded before inspection.
- Inspect the official binding's add and duplicate-return APIs before fixing the
  coordinator interface.
- The later RPC slice still owns the 2 MiB ingress ceiling, trusted clock, public
  error mapping, and Tonic status mapping.
- The later worker slice owns expiry rechecks, claims, progress, cancellation, and
  proof isolation.
- ADR-0002 and O-007 remain open before protected-request gateway integration.

## Work Completed

- B-0017 completed pure signed request admission.
- B-0018 completed the versioned owner-only private envelope and store.
- No B-0019 implementation has started.

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
| B-0019 implementation | NOT AVAILABLE | Not started | 2026-08-21T04:05:32Z |

## Privacy Checks

- NOT STARTED: Redis payload and debug output must expose no private envelope value or
  filesystem path.
- NOT STARTED: Publication failures must preserve redacted errors and cleanup state.
- NOT APPLICABLE: No real transaction data, witness, or proof is planned for this
  baton.

## Risks and Private-Data Notes

- Duplicate handling spans two durable systems. The coordinator must distinguish a
  retained job from a failed first publication without replacing or orphaning private
  data.
- A queue success followed by an unreadable envelope must fail later worker claims.
  The publication path must not report success before both durable transitions hold.
- Queue or cleanup errors cannot include Redis credentials, opaque tokens, private
  paths, request fields, or stored bytes.
- Live Redis tests use the accepted digest-pinned service and do not start proof work.

## Exact Next Action

Inspect `services/razield/src/queue.rs`, the BullMQ conformance tests, and the pinned
crate's publication API. Then claim B-0019 with the exact coordinator and test file
map.
