# Current Engineering Baton

- Baton ID: B-0032
- State: ACTIVE
- Milestone: M4, complete capability service proof flow
- Owner: Codex
- Started UTC: 2026-08-25T19:34:44Z
- Last updated UTC: 2026-08-26T08:59:08Z
- Branch: `main`
- Worktree: Clean after the scoped B-0032 retry fix. The change is pushed to
  `origin/main`.

## Objective

Run one signed capability request through the live API, BullMQ supervisor, private
input assembly, one-shot Plonk prover, terminal capability publication, and independent
verifier, then record a reproducible M4 acceptance result.

## Acceptance Criteria

- [x] One valid signed `CreateCapability` request returns a queued job without waiting
      for proof generation.
- [x] The BullMQ supervisor claims the job once and the isolated prover publishes one
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

- The Mac cannot run the 56 GiB proof sandbox. The real B-0032 proof completed on the
  prepared NUS VM.
- The first NUS B-0032 run stopped before proof generation. Its unprivileged Rust
  acceptance client inherited Cargo's default `/workspace/target`, which was not
  writable after earlier root-owned VM builds. The runner now owns a private
  per-run Cargo target.
- The second NUS attempt confirmed that target fix, then stopped before proof because
  a root-created public Cargo registry file lacked read permission for the host UID.
  Cluster preparation now normalizes the public cache after fetching, and the runner
  repairs an existing cache before starting its first client.
- The third NUS attempt passed the signed request and exact-duplicate gate, then
  exposed SP1's nested Gnark Go download inside the networkless prover-runtime build.
  A separate public dependency-fetch container now fills a dedicated verified module
  cache.
- The fourth NUS attempt reached that fetch container, then stopped because Go tried
  to extend the pinned crate's read-only `go.sum`. The fetch now works from a private
  manifest copy, verifies it, and publishes the finalized pair for the offline build.
  The exact fetch and networkless runtime build now pass in local Apple Silicon
  Docker.
- The fifth NUS attempt completed the signed request, queue, one-shot Plonk proof, and
  terminal capability. The verifier API did not start because its Compose command
  omitted `--bin verifierd` after the package gained three binaries. The terminal
  public job remains in persistent Redis for recovery without another proof.
- No architecture decision or dependency approval remains.

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
- The runner gives every unprivileged acceptance-client invocation one private,
  host-user-owned Cargo target. It reuses that target within the run and deletes it
  during cleanup, independent of the shared workspace target's ownership.
- One reusable cache helper restores read and directory-traverse permissions after a
  root Cargo fetch. The M4 runner applies it before private inputs or services exist.
- The runner now fetches SP1's pinned Gnark Go modules in a build-only container with
  no repository or private-runtime mounts. It builds the prover runtime before
  creating private state. The runtime build remains networkless with a read-only Go
  module cache and `GOPROXY=off`.
- The dependency fetch copies SP1's immutable `go.mod` and `go.sum` into its public
  cache. Go can add required transitive checksums there. The offline build receives
  the finalized manifest and cache read-only.
- Local Docker completed that fetch and the full networkless runtime build. The build
  compiled the native Gnark bridge, SP1 prover SDK, Raziel prover services, and
  authorization guest without creating private state or generating a proof.
- The NUS VM completed the B-0032 Plonk proof in 932,007 ms. The proof container
  peaked at 46,172,573,696 bytes, used no swap, and recorded no OOM event.
- The verifier API Compose command now selects the `verifierd` binary explicitly.
  The rendered boundary check fixes that command shape. Failed runs now preserve API
  and verifier-worker logs before Compose cleanup removes their containers.

## Validation Plan

- Recover the retained public capability from the successful NUS proof job and run
  the independent verifier without another proof.
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
| B-0032 implementation review and push | PASS, commit `1fa6983` | VM-ready runner, client, fixture, docs, and B-0031 archive | 2026-08-26T03:04:41Z |
| B-0032 VM result availability | NOT AVAILABLE, no public result on `origin/main` | Final proof-backed service flow | 2026-08-26T03:08:18Z |
| First B-0032 NUS VM attempt | FAIL before proof, Cargo could not create `/workspace/target/debug` | Acceptance-client build ownership | 2026-08-26T06:55:14Z |
| `sh -n scripts/run-m4-service-flow` | PASS | Corrected runner syntax | 2026-08-26T07:10:28Z |
| Compose rendered configuration | PASS | Admin, service, and proof profiles | 2026-08-26T07:10:28Z |
| `cargo test --locked --offline -p razield --test m4_service_flow` | PASS, four live tests compiled and intentionally ignored | B-0032 acceptance client | 2026-08-26T07:10:28Z |
| Pinned-container Cargo target smoke | SKIP, local Docker daemon unavailable | Exact container ownership path | 2026-08-26T07:10:28Z |
| `git diff --check` | PASS | Scoped retry fix | 2026-08-26T07:10:28Z |
| Second B-0032 NUS VM attempt | FAIL before proof, cached `fnv` source was not readable by the host UID | Public Cargo cache permissions | 2026-08-26T07:13:07Z |
| Public Cargo cache permission fixture | PASS, directory `0755` and source file `0644` | Read and traverse repair without added write access | 2026-08-26T07:30:34Z |
| Shell syntax for cache helper, bootstrap, cluster preparation, and M4 runner | PASS | Corrected cache lifecycle | 2026-08-26T07:30:34Z |
| Compose rendered configuration | PASS | Corrected admin, service, and proof profiles | 2026-08-26T07:30:34Z |
| Cluster plumbing runtime check | SKIP, local Docker daemon unavailable | Docker-backed cluster harness | 2026-08-26T07:30:34Z |
| `git diff --check` | PASS | Public-cache repair | 2026-08-26T07:30:34Z |
| Third B-0032 NUS VM attempt | FAIL before proof after `b0032_submit` passed, Gnark Go modules were absent from the networkless build | Prover-runtime dependency cache | 2026-08-26T07:33:05Z |
| Proof sandbox rendered boundary check | PASS | Public-only fetch mounts, read-only module cache, networkless runtime build and prover | 2026-08-26T07:51:25Z |
| Shell syntax for dependency fetch, M4 runner, and Compose boundary check | PASS | Corrected proof-build preflight | 2026-08-26T07:51:25Z |
| `cargo test --locked --offline -p razield --test m4_service_flow` | PASS, four live tests compiled and intentionally ignored | B-0032 acceptance client | 2026-08-26T07:51:25Z |
| Pinned Gnark module download and networkless runtime build | SKIP, local Docker daemon unavailable | Exact build-only fetch and offline consumer | 2026-08-26T07:51:25Z |
| `git diff --check` | PASS | Gnark build-cache fix | 2026-08-26T07:51:25Z |
| Fourth B-0032 NUS VM attempt | FAIL before private state or proof, Go could not extend the immutable SP1 `go.sum` | Public dependency-manifest staging | 2026-08-26T08:00:17Z |
| Shell syntax for dependency staging and M4 runner | PASS | Cached Go manifest lifecycle | 2026-08-26T08:07:01Z |
| Proof sandbox rendered boundary check | PASS | Cached manifest is read-only and restricted to the networkless build | 2026-08-26T08:07:01Z |
| Cached Gnark module download and networkless runtime build | SKIP, local Docker daemon unavailable | Exact Go command against pinned SP1 source | 2026-08-26T08:07:01Z |
| `git diff --check` | PASS | Cached-manifest fix | 2026-08-26T08:07:01Z |
| `razield-proof-dependency-fetch` | PASS, `all modules verified` | Pinned SP1 Go modules and finalized cached manifest | 2026-08-26T08:24:54Z |
| Networkless `razield-proof-build` | PASS in about five minutes | Native Gnark bridge, SP1 SDK, Raziel binaries, and authorization guest | 2026-08-26T08:24:54Z |
| Fifth B-0032 NUS VM attempt | PASS through terminal capability, proof took 932,007 ms and peaked at 46,172,573,696 bytes with zero swap and no OOM | Signed request, BullMQ, one-shot Plonk, terminal publication, and envelope deletion | 2026-08-26T08:43:57Z |
| Fifth B-0032 independent-verifier startup | FAIL before service code, ambiguous Cargo binary selection | Verifier API Compose command | 2026-08-26T08:43:57Z |
| `./scripts/check-verifier-sandbox-compose` | PASS | Explicit verifier API binary and existing isolation controls | 2026-08-26T08:59:08Z |
| Local verifier API command smoke | PASS through binary selection and expected configuration rejection | Exact Compose `cargo run` command | 2026-08-26T08:59:08Z |

## Privacy Checks

- PASS: no client response contains PolicyIR, relationships, satisfying paths, Merkle
  openings, salts, or caveat values.
- PASS: B-0031 retained only public zero-knowledge proof material.
- PASS: the independent verifier reads trusted state from its own registry and has no
  prover connection.
- PASS: the local B-0032 request marker did not enter service logs or Redis.
- PASS: the networked dependency-fetch service has no repository, registry-state,
  key, envelope, snapshot, proof IPC, or Plonk setup mount.
- NOT APPLICABLE: real transaction data is not part of this synthetic M4 run.

## Risks And Private-Data Notes

- The complete B-0032 proof already succeeded. Recovery work must reuse its retained
  public BullMQ result and must not start another proof.
- The acceptance harness must not introduce a mock success mode into production code.
- The private envelope, witness, and prover traces must be deleted before a terminal
  public capability is retained. The approved private snapshot remains owner-side.
- The cached-manifest fetch and networkless runtime build pass on both architectures.
  Module versions originate in the locked SP1 `6.3.1` crate.

## Exact Next Action

Pull the verifier command fix on the NUS VM. Restart `verifierd` against the retained
B-0032 registry, recover the terminal capability from persistent Redis, and complete
the independent verification and privacy gates without another proof.
