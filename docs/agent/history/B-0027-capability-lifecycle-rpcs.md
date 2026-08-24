# Engineering Baton Archive

- Baton ID: B-0027
- Final state: completed and archived
- Milestone: M4, capability lifecycle RPCs
- Owner: Codex
- Started UTC: 2026-08-24T08:42:13Z
- Completed UTC: 2026-08-24T11:33:29Z
- Branch: `main`
- Base commit: `5154da2`

## Objective

Implement authoritative `GetCapability` and `CancelCapability` behavior over retained
BullMQ state. Return only validated public lifecycle data and propagate active
cancellation through the existing proof supervisor.

## Acceptance Criteria

- [x] `GetCapability` validates the job identifier and returns queued, executing,
      proving, failed, canceled, or completed public state.
- [x] A completed response exposes only the retained bound `ProofCapability` and
      approved aggregate metrics.
- [x] `CancelCapability` handles waiting and active jobs through BullMQ and the proof
      supervisor.
- [x] Duplicate cancellation is deterministic. Terminal jobs cannot reopen or change.
- [x] Missing, malformed, stale, and queue-failure paths return stable generic RPC
      errors with typed private causes.
- [x] No lifecycle response or Redis mutation exposes private envelopes, requests,
      relationships, caveat values, salt, witness data, proof input, or paths.
- [x] Focused service, live Redis, worker, privacy, formatting, lint, and diff gates
      pass without running SP1.

## Architecture Constraints

- ADR-0013 fixes the API-job topology, queue lifecycle, retry, timeout, cleanup, and
  public retention rules.
- ADR-0014 fixes request identity and job-ID derivation.
- ADR-0015 fixes one-shot cancellation acknowledgment and cleanup order.
- ADR-0016 fixes snapshot publication and remains unchanged.
- ADR-0017 fixes durable cancellation control and its PoC consistency limits.
- The API never reads a private envelope to answer Get or Cancel.
- A completed capability must already pass terminal proof and claim validation.
- A real SP1 proof run remains separately approval-gated.

## Decisions

- Accepted: ADR-0017 adds one durable BullMQ `cancellation-commands` queue inside the
  proof supervisor.
- Accepted: one Redis instance, one supervisor process, and one active proof form the
  PoC lifecycle boundary.
- Accepted: `CancelCapability` acknowledges command acceptance. Clients poll
  `GetCapability` for the terminal result.
- Accepted: bounded transition reads can return generic unavailable. Persistent
  inconsistency fails closed.
- Accepted: recovery provides eventual cancellation and private-envelope deletion.
  It does not promise that proof execution never started.

## Work Completed

- Added strict public projection for every supported proof-job state, progress value,
  terminal capability, metric, failure class, and timestamp.
- Implemented `GetCapability` and idempotent `CancelCapability` over authoritative
  BullMQ records with stable public RPC errors.
- Added the durable cancellation queue. The supervisor removes waiting jobs, signals
  active jobs, waits for private cleanup, and retains one closed outcome.
- Added restart recovery for commands retained before or after proof-job removal.
- Added bounded reads for BullMQ field and set-membership transition snapshots.
- Added focused unit, service, live Redis, cancellation, restart, and privacy tests.
- Updated ADR-0013, README, architecture, and the technical specification.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| `cargo test -p razield --lib` | PASS, 97 active and 4 ignored | Lifecycle, service, supervisor, and cancellation units |
| `./scripts/test-bullmq-conformance` | PASS | Live publication, lifecycle, cancellation, recovery, and Redis restart |
| `./scripts/dev pnpm format:check` | PASS | Biome formatting, Rustfmt, and Gofmt |
| `./scripts/dev pnpm lint` | PASS | Biome lint, workspace Clippy, Go vet, and Buf lint |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust workspace check, and Go compile check |
| `./scripts/dev pnpm test` | PASS | Default TypeScript, Rust workspace, Go, and doc tests |
| `sh -n scripts/test-bullmq-conformance` | PASS | Shell syntax |
| Production log and private-field source scan | PASS | New lifecycle, queue, and worker paths |
| Current-state stale-claim scan | PASS | Lifecycle status in README, architecture, and specification |
| `git diff --check` | PASS | Patch hygiene |
| Real SP1 proof | NOT RUN | Outside B-0027 |

The final lint run followed one Clippy-only syntax correction in terminal hex decoding.
The correction used the standard integer divisibility method and changed no behavior.

## Privacy Checks

- PASS: lifecycle responses contain only generated public Protobuf fields.
- PASS: cancellation payloads contain only the job ID, submission time, version, and
  closed outcome.
- PASS: strict decoding rejects unknown cancellation and terminal-result fields.
- PASS: production source contains no lifecycle logging of job IDs, queue payloads,
  private envelopes, witness data, or proof input.
- PASS: live Redis tests confirm private envelope and witness fields do not enter
  lifecycle records.
- NOT APPLICABLE: independent verifier behavior belongs to a later M4 baton.

## Remaining Risks and Follow-Up

- BullMQ fields and state membership need separate reads. Bounded retries absorb fast
  transitions. A longer mismatch returns generic unavailable.
- A cancellation command that exhausts its retry policy needs operator recovery. It
  never becomes a false canceled result.
- Restart recovery cannot prove that the one-shot executor never started.
- Independent retained-proof verification remains pending in M4.
- The maintainer approved an eight-CPU proof profile after reviewing the later VM
  benchmark. B-0028 owns that separate resource-profile amendment.

## Exact Next Action

Claim B-0028 and reconcile the approved eight-CPU SP1 Plonk resource profile across
Compose, checks, ADR-0012, ADR-0015, current documentation, and benchmark evidence.
