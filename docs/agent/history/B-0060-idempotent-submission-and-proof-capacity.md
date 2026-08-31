# Current Engineering Baton

- Baton ID: B-0060
- State: REVIEW
- Milestone: M5, idempotent submission and proof capacity
- Owner: Codex
- Started UTC: 2026-08-31T13:58:12Z
- Last updated UTC: 2026-08-31T14:50:44Z
- Branch: `main`
- Worktree: B-0060 is implemented and validated on top of `bb58440`. This archive
  accompanies the scoped implementation commit.

## Objective

Make an uncertain M5 capability submission safe to retry, and prevent a second
distinct proof request from waiting behind the single live Plonk worker.

## Acceptance Criteria

- [x] One browser action has one stable submission ID across uncertain retries.
- [x] A retry reuses the exact signed request, issuance nonce, expiry, and job ID.
- [x] The console reconciles an uncertain create result through `GetCapability`
      before it sends the exact create command again.
- [x] `razield` accepts an exact duplicate but rejects a different request while the
      PoC proof-capacity slot is occupied.
- [x] Busy rejection retains no new private envelope or BullMQ job.
- [x] The portal reports proof capacity separately from generic service failure.
- [x] Focused TypeScript and Rust checks pass without SP1 proof generation.
- [x] Docs match the approved PoC behavior and no private request data enters logs.

## Architecture Constraints

- BullMQ remains authoritative for proof lifecycle state.
- The PoC has one Redis instance, one `razield` publisher process, one proof
  supervisor, and global proof concurrency one.
- Stable submission IDs are API orchestration metadata. They are not proof inputs or
  capability claims.
- The exact signed request remains commitment-bound. Retries cannot change its
  nonce, expiry, context, scope, or signature.
- The worker keeps the approved 75-minute validity check at trusted execution start.
- The agent receives no PolicyIR, relationship tuple, caveat value, witness, Merkle
  opening, or private-envelope reference.

## File Scope

- `apps/demo-console/components/agent-capability-workspace.tsx`
- `apps/demo-console/lib/agent-capability/`
- `apps/demo-console/test/agent-capability-request.test.ts`
- `apps/demo-console/test/agent-capability-lifecycle.test.ts`
- `services/razield/src/publication/`
- `services/razield/src/queue.rs`
- `services/razield/src/service/`
- `services/razield/src/worker/tests.rs`
- `services/razield/tests/bullmq_conformance.rs`
- `services/razield/tests/capability_publication.rs`
- `tools/raziel-admin/src/capability_client.rs`
- `tools/raziel-admin/src/bin/raziel-capability-client.rs`
- `docs/decisions/0017-durable-capability-cancellation-control.md`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0059-dataset-capability-scope-regression.md`
- `docs/agent/history/B-0060-idempotent-submission-and-proof-capacity.md`

## Out Of Scope

- Changing capability wire fields, public proof values, guest code, or witness data
- Changing the approved 75-minute worker-start validity rule
- Running another Plonk proof during implementation
- Multi-publisher or multi-worker production scheduling
- M6 source selection and ingestion

## Decisions

- Inherited: exact signed request duplicates resolve to one deterministic BullMQ job.
- Approved amendment: one distinct nonterminal proof job can exist in the PoC queue.
- Approved amendment: uncertain retries reuse the exact signed request and reconcile
  its expected job ID before another create call.
- Made: one fixed BullMQ simple-deduplication ID is the atomic proof-capacity slot.
- Made: gRPC `RESOURCE_EXHAUSTED` maps to `CAPABILITY_BUSY` and HTTP `429`.
- Made: gRPC `NOT_FOUND` remains distinct so an uncertain retry can safely resend the
  exact create command.

## Open Questions And Blockers

None for B-0060.

## Work Completed

- Confirmed the first live request entered BullMQ and completed successfully despite
  its initial HTTP `503`.
- Confirmed the click retry created a different commitment, waited 952,993 ms, and
  failed the existing worker-start validity rule before proof generation.
- Added a browser submission ID that stays stable only for one unresolved action.
- Retained the exact signed request in a bounded operator-scoped server record.
- Added create-to-get-to-create reconciliation for unavailable and missing results.
- Added one atomic BullMQ capacity slot and retained exact-duplicate behavior.
- Removed a losing private envelope after an atomic capacity race.
- Added typed busy and missing-job results across gRPC, the Rust client process, the
  Next.js API, and the portal message.
- Updated cancellation checks to stage distinct proof jobs sequentially under the
  approved one-job contract.
- Amended ADR-0017 and the technical specification without changing proof semantics.

## Validation Plan

- Run focused console retry and changed-input tests.
- Run the full demo-console tests, lint, typecheck, format check, and production build.
- Run `razield` and internal client unit tests.
- Run live Redis publication, cancellation, and restart-recovery cases.
- Run `git diff --check` and inspect the final diff for private-data exposure.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| NUS VM queue and job inspection | PASS, both failures separated | Two live M5 submissions | 2026-08-31T13:50:00Z |
| Demo-console Vitest suite | PASS, 75 tests | Submission reuse, reconciliation, and portal regressions | 2026-08-31T14:49:12Z |
| Demo-console format and lint | PASS, 104 formatted and 103 linted files | Console source | 2026-08-31T14:49:12Z |
| Demo-console typecheck and production build | PASS | Next.js routes and components | 2026-08-31T14:49:12Z |
| `cargo test --locked -p razield --lib` | PASS, 99 tests and 4 controlled ignores | Publication, queue, service, and worker units | 2026-08-31T14:44:25Z |
| `cargo test --locked -p raziel-admin` | PASS, 34 tests | Typed internal client results | 2026-08-31T14:50:44Z |
| Live Redis capability publication cases | PASS | Exact duplicate, distinct busy result, and envelope cleanup | 2026-08-31T14:30:00Z |
| Live queued and active cancellation cases | PASS | Sequential one-slot cancellation lifecycle | 2026-08-31T14:42:00Z |
| Live retained cancellation recovery | PASS | Present and removed proof targets | 2026-08-31T14:43:00Z |
| Live Redis restart recovery | PASS | Durable BullMQ state | 2026-08-31T14:44:00Z |
| SP1 Plonk proof generation | SKIP, no proof semantics changed | Out of B-0060 scope | 2026-08-31T14:50:44Z |

## Privacy Checks

- PASS: Agent responses contain no PolicyIR.
- PASS: Agent responses contain no relationship tuple or satisfying path.
- PASS: Agent responses contain no Merkle opening.
- PASS: Logs contain no caveat secrets or witness material.
- PASS: Capability claims expose only approved public fields.
- PASS: The verifier still reads policy from its own trusted registry.
- NOT APPLICABLE: No verifier or proof execution changed in B-0060.
- NOT APPLICABLE: No real transaction data entered Git or generated output.

## Risks And Private-Data Notes

- A stable retry record contains the signed transport request in server process
  memory for at most two hours. The store is bounded to 64 submissions.
- Server restart loses that retry record. The atomic queue slot still prevents a
  second distinct proof from waiting behind the first job.
- A concurrent publisher can stage an envelope before losing the atomic slot. It
  deletes that envelope before returning busy.
- A busy response reveals only occupied proof capacity. It reveals no existing job
  ID, policy, dataset, subject, relationship, or context.

## Exact Next Action

Pull the B-0060 commit and restart the M5 stack on the NUS VM before the next live
capability request.
