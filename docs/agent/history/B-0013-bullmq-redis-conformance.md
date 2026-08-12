# Current Engineering Baton

- Baton ID: B-0013
- State: REVIEW
- Milestone: M4, BullMQ and Redis conformance gate
- Owner: Codex
- Started UTC: 2026-08-12T06:03:08Z
- Last updated UTC: 2026-08-12T06:49:12Z
- Branch: `main`
- Worktree: B-0013 implementation and documentation changes are ready to commit on
  `main` after `fe77005`. B-0012 is archived at
  `docs/agent/history/B-0012-m4-service-execution-contract.md`.

## Objective

Add a reproducible BullMQ `1.2.3` and Redis `8.2.8` conformance gate for the exact M4
queue behaviors accepted in ADR-0013. Stop before implementing capability admission,
private envelopes, proof execution, RPC endpoints, or verifier behavior.

## Acceptance Criteria

- [x] BullMQ `1.2.3` and the required Tokio runtime are workspace-pinned and locked.
- [x] Docker Compose defines the digest-pinned Redis `8.2.8` service with owner-only
      networking, authentication, bounded memory, `noeviction`, and persistent state.
- [x] The live gate proves custom-ID deduplication and queued-job removal.
- [x] The live gate proves worker and queue-global concurrency both limit processor
      execution to one job.
- [x] The live gate proves lock renewal and terminal failure without a second
      processor invocation after lock loss.
- [x] The live gate proves active cancellation-token delivery and progress events.
- [x] The live gate proves age-and-count terminal retention and Redis restart
      recovery.
- [x] The gate uses no raw Redis commands for queue lifecycle behavior.
- [x] Focused tests, affected workspace checks, formatting, and diff checks pass.

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
- Made: Serde `1.0.229` and Serde JSON `1.0.151` are mandatory transitive pins of the
  approved BullMQ `1.2.3` dependency. The BullMQ approval covers their workspace
  alignment.
- Made: Redis writes AOF state under `/data` with `appendfsync always`. The default
  Redis user has a random boot-local password and full internal command rights so AOF
  `MULTI/EXEC` replay succeeds. Network clients do not receive that password and use
  the named `raziel` account.

## Open Questions and Blockers

- ADR-0002 and O-007 remain open before the stolen-capability M4 exit gate can close.
- A later positive isolated-verifier gate needs approval for one synthetic Plonk rerun
  and retention of its public proof artifact.
- No extra crate or system facility for Linux process limits is approved.

## Work Completed

- B-0012 accepted the M4 service topology and dependency boundaries.
- Pinned BullMQ `1.2.3`, Tokio `1.53.1`, Serde `1.0.229`, and Serde JSON `1.0.151` in
  the Rust workspace and resolved `Cargo.lock`.
- Added reusable queue configuration with one worker slot, one global slot, one job
  attempt, one started attempt, and zero stalled recoveries.
- Added live BullMQ tests for custom-ID deduplication, queued removal, worker and
  global concurrency, lock renewal, cancellation, progress, terminal retention,
  stalled lock loss, and Redis restart recovery.
- Added digest-pinned Redis with authentication, no host port, internal networking,
  AOF persistence, bounded memory, `noeviction`, a persistent volume, and a test
  runner with no Linux capabilities.
- Added `./scripts/test-bullmq-conformance` and `pnpm test:queue`. The command creates
  an isolated project, random credential, network, containers, and volumes, then
  removes every resource on success or failure.
- Diagnosed Redis AOF replay failures caused by a disabled default ACL user. A
  boot-local unknown password now permits internal replay without unauthenticated
  network access.
- Updated current-state documentation to record a passing queue gate without marking
  M4 complete.

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
| `git status --short`, branch, and recent log | PASS, clean `main` at `fe77005` | B-0013 start loop | 2026-08-12T06:03:08Z |
| `./scripts/compose run --rm toolchain cargo check -p razield --all-targets` | BLOCKED | Exact Serde pins conflict with BullMQ 1.2.3 | 2026-08-12T06:16:44Z |
| Maintainer clarification | APPROVED | BullMQ approval covers mandatory Serde `1.0.229` and Serde JSON `1.0.151` alignment | 2026-08-12T06:25:06Z |
| `./scripts/compose run --rm toolchain cargo check -p razield --all-targets` | PASS | Resolved BullMQ, Tokio, and Serde graph | 2026-08-12T06:26:00Z |
| Initial `./scripts/test-bullmq-conformance` restart checks | FAIL | AOF transaction replay returned Redis `NOPERM` with the default user disabled | 2026-08-12T06:37:54Z |
| Authenticated Redis AOF script-transaction probe | PASS | Values survived restart, named user authenticated, unauthenticated `PING` returned `NOAUTH`, and logs contained no replay error | 2026-08-12T06:40:06Z |
| Final `./scripts/test-bullmq-conformance` | PASS | All BullMQ `1.2.3` behaviors, authenticated AOF restart, and runner with no Linux capabilities | 2026-08-12T06:49:12Z |
| `./scripts/dev cargo test --locked -p razield` | PASS | Four unit tests, four live tests skipped outside their Redis gate | 2026-08-12T06:49:12Z |
| `./scripts/dev cargo test --locked --offline --workspace --exclude raziel-authorization-guest` | PASS | Full non-guest Rust workspace, with documented SP1 execution and proof tests ignored | 2026-08-12T06:49:12Z |
| `./scripts/dev cargo clippy --locked --offline -p razield --all-targets -- -D warnings` | PASS | Focused Rust static checks | 2026-08-12T06:49:12Z |
| `./scripts/dev pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt checks | 2026-08-12T06:49:12Z |
| Compose config, shell syntax, `git diff --check`, stale-state scan, and Docker cleanup scan | PASS | Final B-0013 review | 2026-08-12T06:49:12Z |

## Privacy Checks

- PASS: Test payloads contain synthetic identifiers only.
- PASS: Redis values contain no PolicyIR, relationship, path, caveat, opening,
  witness, or proof bytes.
- PASS: Test and service logs contain no private queue payload or Redis credential.
- NOT APPLICABLE: This baton does not implement verifier registry access.
- NOT APPLICABLE: This baton does not use real transaction data.

## Risks and Private-Data Notes

- The official Rust binding is new. The exact pinned crate and Redis image now have a
  live repository conformance gate.
- BullMQ made two internal claims after lock loss. `max_started_attempts = 1` blocked
  the second processor invocation, and the test recorded one invocation.
- Redis persistence can retain unsafe payloads. The test fixture must model only the
  accepted public queue record and opaque reference boundary.
- Redis needs an enabled default principal for AOF transaction replay. Its random
  boot-local password remains inside an owner-only tmpfs ACL file.
- The test entry point removes its containers, networks, volumes, and credential on
  success and failure.

## Exact Next Action

Archive B-0013, commit the queue conformance slice, and open B-0014 for the approved
`raziel-prover` verification-only feature split. Do not run another proof.
