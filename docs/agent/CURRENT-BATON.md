# Current Engineering Baton

- Baton ID: B-0013
- State: READY
- Milestone: M4, BullMQ and Redis conformance gate
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-12T06:01:56Z
- Branch: `main`
- Worktree: B-0012 is archived at
  `docs/agent/history/B-0012-m4-service-execution-contract.md`. ADR-0013 and its
  dependency and feature decisions are accepted. The documentation checkpoint still
  needs validation and commit before this baton is claimed.

## Objective

Add a reproducible BullMQ `1.2.3` and Redis `8.2.8` conformance gate for the exact M4
queue behaviors accepted in ADR-0013. Stop before implementing capability admission,
private envelopes, proof execution, RPC endpoints, or verifier behavior.

## Acceptance Criteria

- [ ] BullMQ `1.2.3` and the required Tokio runtime are workspace-pinned and locked.
- [ ] Docker Compose defines the digest-pinned Redis `8.2.8` service with owner-only
      networking, authentication, bounded memory, `noeviction`, and persistent state.
- [ ] The live gate proves custom-ID deduplication and queued-job removal.
- [ ] The live gate proves worker and queue-global concurrency both limit processor
      execution to one job.
- [ ] The live gate proves lock renewal and terminal failure without a second
      processor invocation after lock loss.
- [ ] The live gate proves active cancellation-token delivery and progress events.
- [ ] The live gate proves age-and-count terminal retention and Redis restart
      recovery.
- [ ] The gate uses no raw Redis commands for queue lifecycle behavior.
- [ ] Focused tests, affected workspace checks, formatting, and diff checks pass.

## Architecture Constraints

- `verifierd` never calls `razield`.
- The agent never receives PolicyIR, relationships, satisfying paths, Merkle openings,
  salts, stored caveat values, or witness traces.
- Redis stores queue state, opaque private-blob references, safe progress, and public
  terminal results only.
- The filesystem is not a queue.
- Worker concurrency and queue-global concurrency are one.
- A crash, lost lock, OOM, or deadline cannot invoke proof processor code twice.
- No hosted prover or SP1 network path receives private witness data.
- Raw Protobuf bytes are never commitment inputs.
- Policy authorization and dataset relevance remain separate systems.
- A second scenario cannot start until the reconciliation acceptance gate passes.

## File Scope

- `Cargo.toml`
- `Cargo.lock`
- `services/razield/Cargo.toml`
- `services/razield/src/` for queue configuration and reusable conformance helpers
- `services/razield/tests/` for the live queue gate
- `docker-compose.yml`
- `scripts/` only for a reproducible queue-test entry point
- `package.json` only if the new gate joins the planned command surface
- `README.md`, `ARCHITECTURE.md`, and `docs/spec/POC-TECHNICAL-SPEC.md` for behavior
  that the gate proves
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Capability admission, job-status RPCs, private-envelope storage, or proof execution
- Tonic service implementation
- `raziel-prover` feature separation
- `verifierd` ingress or worker implementation
- ADR-0002 protected-request signatures
- Next.js gateway, SDK, or console behavior
- Another SP1 proof run or retained proof fixture
- Real transaction data or M6 acceptance
- A second scenario

## Decisions

- Inherited: ADR-0001 and ADR-0003 through ADR-0013 are accepted.
- Inherited: ADR-0013 selects BullMQ `1.2.3`, digest-pinned Redis `8.2.8`, one proof
  job, no proof retries, bounded terminal retention, and owner-only Redis access.
- Inherited: Tonic `0.14.6`, `tonic-prost` `0.14.6`, Tokio `1.53.1`, and the
  `raziel-prover` feature split are approved for their later M4 slices.
- Made: None.

## Open Questions and Blockers

- None for the conformance gate.
- ADR-0002 and O-007 remain open before the stolen-capability M4 exit gate can close.
- A later positive isolated-verifier gate needs approval for one synthetic Plonk rerun
  and retention of its public proof artifact.
- No extra crate or system facility for Linux process limits is approved.

## Work Completed

- B-0012 accepted the M4 service topology and dependency boundaries.
- An isolated BullMQ `1.2.0` baseline passed 49 bundled unit tests and nine live Redis
  behaviors. The in-repository gate must rerun those behaviors against `1.2.3`.
- The pinned Redis image is present locally and passed the isolated authenticated
  restart probe.

## Validation Plan

- Run the focused BullMQ and Redis live gate from one documented command.
- Run `cargo test -p razield` through the pinned toolchain.
- Run affected workspace tests after dependency resolution.
- Run `./scripts/dev pnpm format:check`.
- Run `git diff --check`.
- Inspect logs and Redis payload fields for private-data violations.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0012 archive | PASS | Accepted M4 contract and dependency approvals | 2026-08-12T06:00:38Z |
| `./scripts/dev pnpm format:check`, stale-state scan, link targets, and `git diff --check` | PASS | Accepted documentation checkpoint | 2026-08-12T06:01:56Z |

## Privacy Checks

- PENDING: Test payloads contain synthetic identifiers only.
- PENDING: Redis values contain no PolicyIR, relationship, path, caveat, opening,
  witness, or proof bytes.
- PENDING: Test and service logs contain no private queue payload.
- NOT APPLICABLE: This baton does not implement verifier registry access.
- NOT APPLICABLE: This baton does not use real transaction data.

## Risks and Private-Data Notes

- The official Rust binding is new. Passing its bundled tests does not replace live
  conformance against the exact pinned Redis image.
- BullMQ may perform a second internal claim after a stalled job. The gate must prove
  that `max_started_attempts = 1` blocks a second processor invocation.
- Redis persistence can retain unsafe payloads. The test fixture must model only the
  accepted public queue record and opaque reference boundary.
- The test entry point must remove its container, network, volume, and credentials on
  success and failure.

## Exact Next Action

Commit the accepted B-0012 documentation checkpoint. Then claim B-0013, inspect the
current Compose and workspace command patterns, and add the smallest reusable BullMQ
`1.2.3` conformance surface before implementing any service endpoint.
