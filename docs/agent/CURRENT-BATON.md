# Current Engineering Baton

- Baton ID: B-0032
- State: ACTIVE
- Milestone: M4, complete capability service proof flow
- Owner: Codex
- Started UTC: 2026-08-25T19:34:44Z
- Last updated UTC: 2026-08-25T20:04:00Z
- Branch: `main`
- Worktree: Clean at `960e1be` before B-0031 archive and current-state document edits.

## Objective

Run one signed capability request through the live API, BullMQ supervisor, private
input assembly, one-shot Plonk prover, terminal capability publication, and independent
verifier, then record a reproducible M4 acceptance result.

## Acceptance Criteria

- [x] One valid signed `CreateCapability` request returns a queued job without waiting
      for proof generation.
- [ ] The BullMQ supervisor claims the job once and the isolated prover publishes one
      bound public `ProofCapability`.
- [ ] `GetCapability` reaches `SUCCEEDED` with no job envelope, witness, or prover
      trace in Redis, service logs, retained job files, or client responses. The
      approved private snapshot remains in its owner-only store.
- [ ] The independent verifier accepts the capability with the proof issuer and proof
      worker stopped.
- [ ] An altered capability and stolen agent key binding fail closed. Replay storage
      remains the M5 gateway's responsibility.
- [ ] One operator command set and one public result record reproduce the complete
      flow on the NUS VM.
- [ ] Focused service, queue, proof, verifier, privacy, formatting, lint, and Compose
      checks pass.

## Architecture Constraints

- `verifierd` never calls `razield`.
- BullMQ stays authoritative for asynchronous job state. One proof job runs at once.
- Only the approved local Plonk prover receives private witness material.
- The prover runs with eight CPUs, a 56 GiB memory cap, zero swap, and no network.
- The verifier runs with a 2 GiB memory cap, zero swap, no network, and a read-only
  root.
- Public proof values, canonical encodings, policy semantics, and capability formats
  do not change in this task.

## File Scope

- M4 service-flow integration tests and fixtures under `services/razield/` and
  `services/verifierd/`
- Existing Compose profiles and bounded orchestration scripts
- One M4 operator runbook and public acceptance result
- `README.md`, `ARCHITECTURE.md`, and `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/agent/CURRENT-BATON.md`

## Out Of Scope

- M5 gateway, nonce cache, payment API, and frontend
- M6 real transaction acceptance
- Public proof-value, PolicyIR, snapshot, caveat, or canonical-hash changes
- Hosted or network proving

## Decisions

- Inherited: ADR-0013 through ADR-0017 define the service, identity, private proving,
  snapshot publication, and cancellation boundaries.
- Inherited: ADR-0012 fixes local Plonk proof mode and the approved eight-CPU profile.
- Inherited: ADR-0002 fixes independent-verifier request binding and replay checks.
- Made: None.

## Open Questions And Blockers

- The Mac cannot run the 56 GiB proof sandbox. The final real-proof portion must run
  on the prepared NUS VM after this plumbing is committed and pushed.
- No architecture decision, dependency approval, or implementation blocker remains.

## Work Completed

- B-0017 through B-0030 implemented and tested every service boundary separately.
- B-0031 retained one public Plonk fixture and proved isolated verification with all
  `razield` roles stopped.
- The B-0031 proof used 424,312 instructions, took 932,309 ms, and peaked at
  46,271,315,968 bytes under the approved eight-CPU profile.
- The isolated verifier accepted that proof in 447 ms and rejected eight public
  mutations. It peaked at 8,146,944 bytes with zero swap and no OOM event.
- B-0032 adds one synthetic SpiceDB policy and one Rust acceptance client that uses
  the generated production RPC clients. It contains no mock proof or success path.
- The operator runner creates temporary authorities, compiles and approves PolicyIR,
  seeds SpiceDB, submits the signed request before starting the proof worker, waits
  for one real Plonk result, stops the issuer and prover, and verifies independently.
- The local pre-proof run built a real snapshot and queued job. The exact duplicate
  returned the same job ID with the proof worker absent.
- The local privacy scan found no private request marker in service logs or Redis.
- The B-0032 VM runbook documents the one approval flag, resource profile, outputs,
  retention rules, and failure evidence.

## Validation Plan

- Complete the final source review, commit the VM-ready plumbing, and push it.
- Run the real Plonk service flow once on the NUS VM and commit only public evidence.
- Reconcile M4 status across the spec, architecture, README, and baton archive.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0031 archive | PASS | Retained proof and isolated verifier | 2026-08-25T19:34:44Z |
| `cargo test -p verifierd -p razield --all-targets` | PASS, 122 tests with 11 controlled or live tests skipped | M4 service and verifier regression suite | 2026-08-25T19:34:44Z |
| Proof sandbox, verifier sandbox, and cluster plumbing checks | PASS | Rendered isolation controls and retained-proof tooling | 2026-08-25T19:34:44Z |
| B-0032 policy compilation | PASS | Synthetic SpiceDB schema to approved PolicyIR input | 2026-08-25T20:04:00Z |
| B-0032 live SpiceDB seed | PASS | Schema, key-derived relationship, and exact revision | 2026-08-25T20:04:00Z |
| B-0032 pre-proof service path | PASS | Snapshot, signed request, queued job, and exact duplicate | 2026-08-25T20:04:00Z |
| B-0032 local privacy scan | PASS | Private marker absent from service logs and Redis | 2026-08-25T20:04:00Z |
| `cargo test -p verifierd -p razield --all-targets` | PASS, 131 active tests with 19 controlled or live tests skipped | Current B-0032 service and verifier regression suite | 2026-08-25T20:04:00Z |
| `cargo clippy -p razield --test m4_service_flow -- -D warnings` | PASS | B-0032 acceptance client | 2026-08-25T20:04:00Z |
| Proof sandbox, verifier sandbox, and cluster plumbing checks | PASS | Current isolation and SP1 plumbing | 2026-08-25T20:04:00Z |

## Privacy Checks

- PASS: no client response contains PolicyIR, relationships, satisfying paths, Merkle
  openings, salts, or caveat values.
- PASS: B-0031 retained only public zero-knowledge proof material.
- PASS: the independent verifier reads trusted state from its own registry and has no
  prover connection.
- PASS: the local B-0032 request marker did not enter service logs or Redis.
- NOT APPLICABLE: real transaction data is not part of this synthetic M4 run.

## Risks And Private-Data Notes

- A complete run starts a costly Plonk proof. Local development must stop before that
  step or use the retained public fixture. Only the NUS VM performs the final proof.
- The acceptance harness must not introduce a mock success mode into production code.
- The private envelope, witness, and prover traces must be deleted before a terminal
  public capability is retained. The approved private snapshot remains owner-side.

## Exact Next Action

Review, commit, and push the B-0032 acceptance plumbing. Then run its one approved
real Plonk service flow on the NUS VM and return the public result record.
