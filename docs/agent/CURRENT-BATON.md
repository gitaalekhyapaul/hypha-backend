# Current Engineering Baton

- Baton ID: B-0026
- State: READY
- Milestone: M4, private snapshot publication
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-23T05:35:06Z
- Branch: `main`
- Worktree: B-0025 is complete and ready to commit. Its archive is
  `docs/agent/history/B-0025-isolated-prover-launcher-and-terminal-validation.md`.

## Objective

Implement the trusted administrative publication path that reads one exact SpiceDB
revision, creates the blinded relationship snapshot, stores the immutable private
record, and publishes the signed public snapshot record used by capability requests
and proof generation.

## Acceptance Criteria

- [ ] The build path reads relationships at one exact SpiceDB revision and records
      the returned consistency token without exposing it to agents.
- [ ] One fresh 32-byte secret salt creates the ADR-0009 relationship root and never
      enters public RPC output, Redis, logs, or the signed descriptor.
- [ ] The immutable ADR-0015 private snapshot is published before the public registry
      record and stays bound to the exact snapshot ID, root, and policy.
- [ ] Public publication signs the approved snapshot descriptor with the configured
      authority key and performs an atomic trusted-registry update.
- [ ] Duplicate, stale epoch, policy mismatch, SpiceDB error, private-store error,
      signing error, and public-registry error paths fail closed with typed private
      errors and generic RPC errors.
- [ ] Focused adapter, publication, service, registry, privacy, formatting, lint, and
      diff gates pass without running SP1.

## Architecture Constraints

- ADR-0009 fixes blinded relationship leaves, tree ordering, salt size, and public
  descriptor fields.
- ADR-0010 fixes signed snapshot authority and local registry trust.
- ADR-0015 fixes the immutable private snapshot record, root-derived lookup, owner
  modes, and publication order.
- The agent never receives relationships, caveat values, salt, tree openings,
  PolicyIR, or the SpiceDB consistency token.
- Raw Protobuf bytes never become commitment inputs.
- `razield` cannot approve its own PolicyIR. Snapshot publication must resolve an
  already approved policy record.
- No new dependency, package version, toolchain, proof run, or architecture change is
  approved by this baton.

## File Scope

- Existing `SnapshotService` handlers and service composition under
  `services/razield/src/`
- Existing private snapshot, authority, registry, snapshot, and SpiceDB adapter APIs
- Focused snapshot publication tests and current-state documentation
- `docker-compose.yml` only for the private snapshot volume needed by `razield`
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- `GetCapability` and `CancelCapability`
- Independent `verifierd` service behavior
- A proof run, proof-protocol change, new public proof value, or capability change
- M5 frontend and gateway work, M6 real-data acceptance, M7, or another scenario
- At-rest encryption, a database-backed private snapshot store, or remote proving

## Decisions

- Inherited: ADR-0009, ADR-0010, ADR-0013, and ADR-0015.
- Inherited: B-0024 provides the strict private snapshot codec and immutable store.
- Inherited: B-0025 provides the fixed proof sandbox and terminal capability path.
- Made: None.

## Open Questions and Blockers

- None recorded. Reconcile the existing SnapshotService shell and SpiceDB adapter
  before claiming this baton.

## Work Completed

- B-0025 completed the isolated launcher, sandbox, terminal verification, and public
  BullMQ result boundary.
- The user requested exact VM Docker and API E2E instructions after snapshot,
  lifecycle, worker, and verifier endpoints form a complete backend flow.

## Validation Plan

- Inspect the generated SnapshotService contract, current handlers, SpiceDB clients,
  authority APIs, registry publisher, and private snapshot store.
- Add the smallest production publication path that composes those approved parts.
- Run focused snapshot and service tests, then affected authority, registry, Compose,
  privacy, formatting, lint, and diff gates.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0025 archive | PASS | Launcher, sandbox, worker, terminal verification, and public result | 2026-08-23T05:35:06Z |
| B-0026 implementation | NOT AVAILABLE | Not started | 2026-08-23T05:35:06Z |

## Privacy Checks

- NOT APPLICABLE: B-0026 has not started.

## Risks and Private-Data Notes

- Publication ordering matters. A public accepted root without its private snapshot
  leaves queued proofs unable to resolve the witness source.
- A private snapshot published without a public record is safe but orphan cleanup
  must not delete data referenced by an active publication attempt.
- SpiceDB pagination and consistency must not mix revisions.
- The final VM E2E runbook must use only public admin and capability APIs. It must not
  require manual private-volume edits.

## Exact Next Action

Commit B-0025, then claim B-0026 and inspect the existing snapshot service and
SpiceDB publication APIs before editing production code.
