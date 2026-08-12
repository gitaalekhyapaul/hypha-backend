# Current Engineering Baton

- Baton ID: B-0012
- State: REVIEW
- Milestone: M4, service execution contract
- Owner: Codex
- Started UTC: 2026-08-11T11:55:07Z
- Last updated UTC: 2026-08-12T05:59:49Z
- Branch: `main`
- Worktree: Started from clean `main` at `c403620`. Current changes are limited to
  B-0012 documentation scope. M0 through M3 are complete. B-0011 is archived at
  `docs/agent/history/B-0011-local-sp1-plonk-proof.md`. The validated public Plonk
  result is recorded at
  `docs/benchmarks/sp1-plonk/cluster-20260810T174732Z-76324.md`.

## Objective

Reconcile M3 closure across every current-state document and draft the M4 execution
contract. The contract covers the asynchronous capability API, the local proof-job
worker, independent verifier isolation, framework ownership, and proof-lifetime
constraints. Stop at a reviewable ADR. Do not add dependencies or service behavior
in this baton.

## Acceptance Criteria

- [x] The baton protocol distinguishes current-state documents from historical
      evidence and requires a stale-claim scan at milestone close.
- [x] The README, architecture, technical specification, and active baton agree that
      M0 through M3 are complete and M4 design is active.
- [x] The architecture shows a responsive API process, an asynchronous proof worker,
      job polling, and a verifier that never calls the prover.
- [x] One accepted decision record defines M4 process roles, a maintained BullMQ
      queue, bounded private-blob storage, cancellation, isolation, trusted-key
      selection, timeouts, errors, and SP1 feature separation.
- [x] The decision record exposes the conflict between the measured 29-minute proof
      and the current 15-minute capability lifetime.
- [x] The documents distinguish a possible synthetic development walkthrough from
      the real-data M6 acceptance run without changing the current synthetic-data
      restriction.
- [x] Documentation checks and `git diff --check` pass.

## Architecture Constraints

- `verifierd` never calls `razield`.
- The verifier resolves `policy_hash` through its trusted local policy registry.
- The agent never receives PolicyIR, relationships, paths, openings, salts, stored
  caveat values, or witness traces.
- Request data cannot select the authorization guest ELF or verification key.
- Raw Protobuf bytes are never commitment inputs.
- The baseline guest proves `ALLOW` only.
- No hosted prover or SP1 network client receives private witness data.
- Policy authorization and dataset relevance remain separate systems.
- A second scenario cannot start until the reconciliation acceptance gate passes.

## File Scope

- `docs/agent/BATON-PROTOCOL.md`
- `docs/agent/CURRENT-BATON.md`
- `AGENTS.md` for the maintainer-clarified M5 synthetic-data boundary
- `README.md`
- `ARCHITECTURE.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/decisions/0013-m4-service-and-verifier-execution.md`

## Out of Scope

- Rust, TypeScript, Go, Protobuf, generated code, or dependency changes
- `razield`, proof-worker, or `verifierd` implementation
- Protected-request signatures and gateway cache behavior
- A second scenario or M5 runtime implementation
- Real transaction data and reconciliation acceptance
- Another SP1 proof run or retained proof fixture
- Hosted proving, SP1 network proving, CUDA, or a new proof mode
- Hypha dataset relevance attestation

## Decisions

- Inherited: D-001 through D-019 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0012.
- Inherited: FR-066 already requires an asynchronous proof-job API.
- Inherited: D-003 assigns the console and gateway to Next.js. D-004 assigns proving,
  verification, `razield`, and `verifierd` to Rust.
- Inherited: ADR-0012 permits Plonk for public capabilities and limits Core to
  non-sensitive internal measurements.
- Maintainer direction: retain the Rust API-job server architecture and defer NestJS
  for the PoC.
- Maintainer direction: use a maintained job-queue dependency and reject the custom
  filesystem spool as queue machinery.
- Approved for review testing: `bullmq-official = 1.2.0` and the pinned Redis 8.2.8
  image. The image pull and isolated tests are complete.
- Approved: use `bullmq-official = 1.2.3` for M4. Its worker fixes affect
  prompt pickup, bounded concurrency, and graceful shutdown. The filesystem stores
  private envelopes only and performs no queue operations.
- Approved: ADR-0013 defines the M4 service and isolation topology, verifier worker,
  75-minute Plonk capability window, and Ed25519 issuance authentication.
- Approved: Tonic `0.14.6`, `tonic-prost` `0.14.6`, Tokio `1.53.1` with the bounded
  service features, and the `raziel-prover` verification-only feature split.
- Made: ADR-0013 accepted on 2026-08-12.

## Open Questions and Blockers

- The accepted 75-minute window replaces the unusable 15-minute proof-bound default
  for the local Plonk profile.
- Maintainer clarification on 2026-08-12 permits portal-created synthetic transaction
  records for the labeled M5 development walkthrough. This does not count toward M6.
- None of the approved M4 architecture, dependency, runtime, or feature changes block
  the first implementation baton.
- A later real-proof verifier gate needs one retained public synthetic proof artifact
  or one approved proof rerun. No rerun is approved in this baton.
- ADR-0002 and O-007 remain open. M4 cannot close the stolen-capability exit gate
  until its protected-request envelope is approved and tested.
- O-005 remains open prior to gateway integration.
- Deferred follow-up: after the M7 exit gate passes and the PoC meets its definition
  of done, remind the maintainer about Hypha relevance attestation, the Obsidian
  `Current Implementation Discussion Post`, and the TEE-backed local-authorization
  experiment.

## Work Completed

- Reconciled the current branch and M3 closeout commit with ADR-0012, B-0011, the
  public benchmark, and the technical specification.
- Inspected the capability and verifier Protobuf contracts. The asynchronous job
  states and Create/Get/Cancel API already exist.
- Inspected the empty `razield` and `verifierd` process shells, the verifier-owned
  registries, the SP1 artifact codec, `CpuProver`, and `LightProver` boundaries.
- Replaced the synchronous capability sequence with the existing asynchronous
  Create/Get/Cancel job contract and proposed local proof-worker flow.
- Corrected the README and milestone status records to mark M3 complete and M4 at
  active design.
- Confirmed that generated Rust service bindings exist but are not compiled by
  `raziel-proto`; no Rust service runtime dependency is approved.
- Drafted ADR-0013 with the API/worker topology, verifier isolation, Core-versus-Plonk
  boundary, NestJS decision, job lifecycle, and proof-lifetime choice.
- Folded the maintainer's portal-created synthetic transaction workflow into M5 and
  removed O-009 as an unnecessary separate decision.
- Replaced the proposed filesystem queue with official BullMQ on Redis. Retained the
  filesystem only for bounded private envelopes and one-shot proof scratch data.
- Compared official BullMQ Rust 1.2.0 with Apalis 0.7.4 and raw Redis Streams. BullMQ
  supplies the required queue lifecycle without custom Redis or filesystem queue
  logic. Its July 2026 Rust release remains subject to focused conformance tests.
- Compiled BullMQ 1.2.0 on Rust 1.97.1 and ran its 49 bundled unit tests. All passed.
- Ran an isolated authenticated Redis 8.2.8 probe for custom-ID deduplication,
  one-job global concurrency, lock renewal, stalled failure without a second
  processor invocation, queued removal, cancellation-token delivery, progress
  events, bounded terminal retention, and restart recovery. All passed.
- Audited BullMQ 1.2.3. It pins direct dependencies and contains worker fixes relevant
  to job pickup, concurrency, and graceful close. No repository dependency changed.
- Recorded maintainer acceptance of ADR-0013, BullMQ 1.2.3, the pinned Redis runtime,
  Tonic and Tokio dependencies, and the `raziel-prover` feature split.

## Validation Plan

- Inspect the diff for agreement across current-state documents.
- Search non-historical documentation for stale M3 and synchronous-proof claims.
- Check every new local Markdown link target.
- Run `git diff --check`.
- Confirm that no source, dependency, lockfile, generated file, proof, or private
  artifact changed.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `git status --short` | PASS, clean | Start state at `c403620` | 2026-08-11T11:55:07Z |
| M3 source reconciliation | PASS | ADR-0012, B-0011, public benchmark, specification | 2026-08-11T11:55:07Z |
| M4 contract inspection | PASS | Protobuf, service shells, registries, SP1 prove and verify paths | 2026-08-11T11:55:07Z |
| Current-state stale-claim scan | PASS | No obsolete M3 state outside dated or archived evidence | 2026-08-11T12:10:33Z |
| New local-link target check | PASS | ADR-0013, B-0011 archive, and M3 benchmark targets exist | 2026-08-11T12:10:33Z |
| `./scripts/dev pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt checks | 2026-08-11T12:10:33Z |
| `git diff --check` | PASS | B-0012 documentation diff | 2026-08-11T12:10:33Z |
| Maintainer review amendments | PASS | Rust API-job direction recorded; O-009 removed; M5 synthetic editor contract added | 2026-08-12T02:17:56Z |
| `git diff --check` and `./scripts/dev pnpm format:check` | PASS | Amended B-0012 documentation | 2026-08-12T02:17:56Z |
| Queue dependency review | PASS | Official BullMQ Rust 1.2.0, Apalis 0.7.4, Redis runtime, isolation gaps | 2026-08-12T03:26:42Z |
| `git diff --check` and `./scripts/dev pnpm format:check` | PASS | BullMQ and Redis ADR amendment | 2026-08-12T03:26:42Z |
| `cargo check --locked` | PASS | Isolated BullMQ 1.2.0 compile probe on Rust 1.97.1 | 2026-08-12T05:43:57Z |
| `cargo test --lib` | PASS, 49 tests | Isolated BullMQ 1.2.0 bundled unit suite | 2026-08-12T05:43:57Z |
| Authenticated Redis 8.2.8 live probe | PASS, nine behaviors | Disposable BullMQ 1.2.0 queue and worker | 2026-08-12T05:43:57Z |
| `git diff --check` and `./scripts/dev pnpm format:check` | PASS | Queue probe review amendment | 2026-08-12T05:45:11Z |
| Maintainer approval reconciliation and `./scripts/dev pnpm format:check` | PASS | Accepted ADR-0013 and current-state documents | 2026-08-12T05:59:49Z |

## Privacy Checks

- PASS: This baton exposes no PolicyIR, relationship tuple, path, Merkle opening,
  salt, caveat value, witness, proof byte, or private journal.
- PASS: The planned worker keeps witness-bearing job data inside the data-owner trust
  boundary.
- PENDING: M4 verifier ingress and child-process logs need implementation review.
- PENDING: M4 must prove verifier independence with `razield` stopped.

## Risks and Private-Data Notes

- BullMQ and Redis store an opaque private-blob reference, safe progress, and public
  terminal data. They do not store private request context or witness material.
- The private envelope store must be owner-only, bounded, atomically written, and
  erased before terminal state publication.
- A proof worker sharing the API process can exhaust the API's cgroup. The M4 design
  needs separate process and resource boundaries.
- A remote artifact can trigger large allocations or verifier aborts prior to
  cryptographic rejection. Bound bytes prior to decode and isolate verification.
- A caller-selected key or policy record can turn valid cryptography into invalid
  authorization. Resolve trusted material from verifier-owned state.
- Generic client failures must not expose policy structure, registry state, or proof
  internals. Detailed codes belong only in access-controlled audit records.

## Exact Next Action

Archive B-0012, reset the current baton to B-0013 for the BullMQ and Redis conformance
gate, validate the documentation checkpoint, and commit it before changing Cargo or
service source. ADR-0002 and a future proof rerun remain separate.
