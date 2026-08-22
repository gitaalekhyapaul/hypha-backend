# Current Engineering Baton

- Baton ID: B-0021
- State: READY
- Milestone: M4, BullMQ proof-supervisor claim and private-envelope handoff
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-22T07:33:26Z
- Branch: `main`
- Worktree: B-0020 service ingress and its archive are ready to commit after
  `0a011c0`. The branch was two commits ahead of `origin/main` before that commit.

## Objective

Implement the bounded proof-supervisor claim boundary. Use the official BullMQ worker
to claim one job, validate its public payload, read its private envelope only after
the claim, and pass a validated private request to an injectable one-shot executor
contract. Do not run a real proof or publish a terminal capability in this baton.

## Acceptance Criteria

- [ ] Supervisor configuration validates Redis, queue prefix, private-envelope root,
      and worker timing values without exposing credentials or private paths.
- [ ] The official BullMQ worker owns claims, locks, lock renewal, one-job
      concurrency, stalled failure, and terminal retention.
- [ ] The supervisor validates the strict public payload before private storage access.
- [ ] The supervisor reads and commitment-checks the private envelope only after a
      successful BullMQ claim.
- [ ] An injectable one-shot executor contract receives only validated private input
      and bounded owner configuration.
- [ ] Invalid payloads, missing or corrupt envelopes, cancellation, and executor
      failures produce generic public state and typed private failure classes.
- [ ] Progress contains safe phase values only. It contains no request context,
      policy branch, relationship, witness, path, opening, or proof bytes.
- [ ] Focused unit tests, live Redis worker tests, affected workspace tests, strict
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
- `services/razield/src/bin/` for the proof-supervisor process
- `services/razield/src/queue.rs` only for shared worker policy
- `services/razield/src/private_envelope/` only for the post-claim handoff
- `services/razield/src/publication/` only for shared payload and public state types
- `services/razield/src/lib.rs`
- `services/razield/Cargo.toml`
- `Cargo.lock` only for approved workspace selections
- `services/razield/tests/` for focused and live worker checks
- `scripts/test-bullmq-conformance`
- `docker-compose.yml`
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Real SP1 execution or Plonk proof generation
- Witness construction and registry-backed authorization input assembly
- Final capability packaging or terminal success publication
- `GetCapability` and `CancelCapability` RPC behavior
- Active child-process cancellation and proof cgroup implementation
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
- Made: None.

## Open Questions and Blockers

- No blocker prevents inspection and claim.
- Source inspection must fix the exact BullMQ processor, cancellation-token, progress,
  and completion APIs before edits begin.
- Source inspection must fix the narrow executor handoff type before the supervisor
  reads a private envelope.
- No new dependency or proof run is approved.

## Work Completed

- B-0020 completed and archived the bounded Create ingress at
  `docs/agent/history/B-0020-create-capability-service-ingress.md`.
- No B-0021 implementation has started.

## Validation Plan

- Test payload rejection before private-store reads.
- Test private-envelope access only inside an official BullMQ processor claim.
- Test one-job concurrency, progress redaction, cancellation delivery, and zero proof
  retry against the pinned Redis service.
- Test executor success, typed failure, timeout, and malformed-output boundaries with
  a deterministic fake executor. Do not run SP1.
- Run affected service and workspace tests in the pinned toolchain.
- Run strict Clippy, repository formatting, dependency inspection, privacy scans, and
  `git diff --check`.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0020 archive | PASS | Bounded CreateCapability service ingress | 2026-08-22T07:33:26Z |
| Not run | B-0021 implementation not claimed | Proof supervisor | 2026-08-22T07:33:26Z |

## Privacy Checks

- NOT STARTED: Worker progress and failures must expose no private request or witness
  value.
- NOT STARTED: The executor handoff and its debug output must redact the complete
  private envelope.
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

Reconcile the B-0020 commit, claim B-0021, then inspect the pinned BullMQ worker APIs
and existing prover input boundary before selecting the implementation file map.
