# Current Engineering Baton

- Baton ID: B-0027
- State: READY
- Milestone: M4, capability lifecycle RPCs
- Owner: unassigned
- Started UTC: None
- Last updated UTC: 2026-08-24T05:48:03Z
- Branch: `main`
- Worktree: B-0026 is archived. Inspect Git before claiming B-0027.

## Objective

Implement authoritative `GetCapability` and `CancelCapability` behavior over the
retained BullMQ job state. Return only validated public lifecycle data and propagate
active cancellation through the existing proof supervisor.

## Acceptance Criteria

- [ ] `GetCapability` validates the job identifier and returns the authoritative
      queued, executing, failed, cancelled, or completed public state.
- [ ] A completed response exposes only the retained bound `ProofCapability` and
      approved aggregate metrics.
- [ ] `CancelCapability` handles waiting and active jobs through the existing BullMQ
      and supervisor cancellation contracts.
- [ ] Duplicate cancellation is deterministic. Terminal jobs cannot be reopened or
      altered.
- [ ] Missing, malformed, stale, and queue-failure paths return stable generic RPC
      errors with typed private causes.
- [ ] No lifecycle response or Redis mutation exposes private envelopes, requests,
      relationships, caveat values, salt, witness data, proof input, or paths.
- [ ] Focused service, live Redis, worker, privacy, formatting, lint, and diff gates
      pass without running SP1.

## Architecture Constraints

- ADR-0013 fixes the API-job topology, queue lifecycle, cancellation, retry, timeout,
  cleanup, and public retention rules.
- ADR-0014 fixes request identity and job-ID derivation.
- ADR-0015 fixes the one-shot cancellation acknowledgment and cleanup order.
- ADR-0016 fixes snapshot publication and remains unchanged.
- The API never reads a private envelope to answer Get or Cancel.
- A completed capability must already have passed terminal proof and claim validation.
- A real SP1 proof run remains separately approval-gated.

## File Scope

- Existing capability service handlers and lifecycle types under
  `services/razield/src/service/`
- Existing queue, publication, and worker cancellation APIs under
  `services/razield/src/`
- Focused capability lifecycle and live Redis tests
- Current-state README, architecture, technical specification, and baton documents

## Out of Scope

- Independent `verifierd` service behavior
- Snapshot publication or registry-writer changes
- Proof protocol, proof generation, guest, canonical hash, or public capability changes
- M5 frontend and gateway work, M6 real-data acceptance, M7, or another scenario

## Decisions

- Inherited: ADR-0013 through ADR-0016.
- Inherited: B-0025 stores only validated public terminal capability data in BullMQ.
- Inherited: B-0026 completes the runtime SnapshotService lifecycle.
- None made for B-0027.

## Open Questions and Blockers

- None. Reconcile the exact ADR-0013 cancellation and terminal-state contract before
  claiming the baton.

## Work Completed

- B-0026 completed exact SpiceDB snapshot publication, private storage, signing,
  atomic root registration, read, and revocation.
- The current CapabilityService keeps Get and Cancel closed as unimplemented.

## Validation Plan

- Inspect the generated CapabilityService contract and existing queue state mapping.
- Implement the smallest lifecycle boundary over authoritative BullMQ records.
- Test every public state, active and waiting cancellation, terminal idempotency,
  malformed identifiers, queue failures, and private-data exclusion.
- Run focused tests, live Redis conformance, affected service tests, formatting, lint,
  privacy scans, and `git diff --check`.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0026 archive | PASS | Runtime snapshot publication and revocation | 2026-08-24T05:48:03Z |
| B-0027 implementation | NOT AVAILABLE | Baton not claimed | 2026-08-24T05:48:03Z |

## Privacy Checks

- PASS: B-0026 public snapshot RPCs expose no private snapshot inputs.
- NOT AVAILABLE: B-0027 lifecycle responses are not implemented.
- NOT APPLICABLE: Independent verifier behavior belongs to a later baton.

## Risks and Private-Data Notes

- Queue records are the authoritative lifecycle source. The API must not reconstruct
  state from private envelopes.
- Active cancellation must wait for the existing correlated child acknowledgment and
  cleanup sequence before reporting terminal cancellation.
- A queue transport failure must not be presented as a missing or cancelled job.
- The final VM API E2E runbook follows lifecycle and verifier completion.

## Exact Next Action

Read ADR-0013 and the current queue and handler implementations, claim B-0027, and map
each retained BullMQ state to the generated public RPC response.
