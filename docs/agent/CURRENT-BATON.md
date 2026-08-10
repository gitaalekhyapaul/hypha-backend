# Current Engineering Baton

- Baton ID: B-0011
- State: BLOCKED
- Milestone: M3, SP1 execution and proof
- Owner: Codex
- Started UTC: 2026-07-24T09:01:33Z
- Resumed UTC: 2026-07-27T10:16:28Z
- Resumed after Plonk approval UTC: 2026-07-27T19:45:18Z
- Resumed after maintainer pause UTC: 2026-07-28T11:32:51Z
- Last updated UTC: 2026-08-10T18:30:38Z
- Branch: `main`
- Worktree: This B-0011 implementation commit is based on `10fbc88`. It contains
  accepted ADR-0012, the local Plonk proof path, and secured Slurm plumbing. The
  approved 7 GiB gate failed three times, and the approved 30 GiB no-swap diagnostic
  failed once. The maintainer approved one fixed 36 GiB attempt on the prepared
  independent VM. That attempt reached the exact 36 GiB cgroup limit and recorded
  one OOM kill. The future x86 SIF provenance tuple remains unapproved. The single
  approved 56 GiB attempt at commit `44d58c7` created and verified a real Plonk
  proof, then passed all mutation checks. Its public exporter rejected one duplicated
  profile field written by the wrapper and container. Its first recovery pass found
  the verification metric after an exact Rust test-harness prefix on the same line.
  The proof remains valid and must not run again. The local worktree adds narrow
  handling for both saved-log formats and removes the duplicate from future runs.

## Objective

Define and implement the second M3 slice: local SP1 proof generation, verification-key
identity, proof artifact encoding, local verification, and proof benchmarks. Stop
before `verifierd`, capability job APIs, or gateway wiring.

## Acceptance Criteria

- [x] One accepted decision record fixes the local proof mode, verification-key ID,
      proof artifact encoding, public-journal handling, and failure rules.
- [x] A concrete local `CpuProver` creates a proof without selecting an SP1 network
      prover.
- [x] The pinned local verifier accepts the proof, exact verification key, and exact
      public journal.
- [x] Proof, verification-key, and public-journal mutations fail closed.
- [x] At least one representative allowed case records proof time, proof size,
      verification time, instruction count, hardware, and tool versions.
- [x] Focused and affected workspace gates pass.

## Architecture Constraints

- The baseline guest proves `ALLOW` only.
- The agent never receives PolicyIR, relationship tuples, selected paths, Merkle
  openings, salts, or stored caveat values.
- The capability carries `policy_hash`, not PolicyIR.
- Proofs bind the exact ADR-0011 public journal and an approved verification-key ID.
- Raw Protobuf bytes are never commitment inputs.
- `verifierd` never calls `razield`.
- Local proof creation cannot route private witness data to a hosted prover.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `docs/decisions/0012-*.md` for the approved proof and key contract
- `ARCHITECTURE.md` and `docs/spec/POC-TECHNICAL-SPEC.md`
- `crates/prover/`
- Proof fixtures, mutation tests, and benchmark records
- `docker-compose.yml` for the isolated capped benchmark service
- Plonk setup, benchmark, feature-gate, and in-container measurement scripts
- Cluster preparation, Slurm template, proof-only runners, public result exporter,
  and runbook
- `tools/docker/toolchain.Dockerfile` and `tools/toolchain-versions.env`
- Cargo metadata for the approved exact dependencies
- Baton records

## Out of Scope

- `verifierd` service implementation
- Capability job APIs and request signatures
- Registry distribution or deployment key rotation
- Next.js gateway, SDK, and console work
- Real transaction data
- A denial proof or private-policy model
- Hosted proving or an SP1 network prover
- Hypha dataset relevance attestation

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0011.
- ADR-0011 fixes the guest input, public journal, local execute boundary, and the use
  of local `CpuProver` for this proof-generation slice.
- Accepted ADR-0012 defines a benchmark-gated Core proof profile, a local
  `CpuProver`/`LightProver` split, verification-key identity, strict proof artifact
  bytes, unsafe SP1 environment rejection, and a verifier-owned trusted program key.
  The maintainer approved the record and its two exact dependencies on 2026-07-27.
- The approved amendment excludes the SDK version from `verification_key_id`. The
  proof artifact still binds the SDK version for serialization compatibility.
- A post-approval security review found no witness masking in the pinned Core prover.
  Core commits generated traces directly and serializes witness-derived multilinear
  openings. ADR-0012 now suspends Core capability use until a reviewed
  zero-knowledge path exists.
- A follow-up source audit favors Plonk over Groth16 for the first outer-proof
  experiment. Both paths use the same SP1 native Gnark bridge. Plonk uses the Aztec
  Ignition universal setup and its release artifact is 2,305,113,029 bytes. Groth16
  uses a circuit-specific setup and its release artifact is 6,211,807,514 bytes.
  Groth16's smaller proof and lower on-chain gas do not benefit this local PoC.
- The proposed Plonk claim is computational zero knowledge. The pinned CPU BN254
  prover generates random blinding polynomials for the L, R, O, and permutation
  witness polynomials. SP1 exposes only the program-key hash, committed-values
  digest, exit code, recursion-key root, and proof nonce as outer public inputs.
  SP1 does not request Gnark's optional statistical-zero-knowledge mode, so the
  stronger statistical claim is out of scope.
- SP1 replaces official Gnark `v0.14.0` with `p4u/gnark` commit
  `cd7874155e266e08ca8dc247dbc66efb1030bd3b`. Its Go module content is pinned by
  `go.sum`. The CPU BN254 Plonk prover differs from official `v0.14.0` only by two
  lint-suppression comments. No cryptographic code differs in that file.
- The maintainer approved the ADR-0012 Plonk amendment, SP1 `native-gnark`, pinned
  `libclang-14-dev=1:14.0.6-12`, and the one-time 2,305,113,029-byte setup artifact
  download with SHA-256 pinning on 2026-07-27.
- The maintainer allocated 33,600,331,776 bytes to Docker and approved one bounded
  diagnostic. The container uses a 30 GiB hard cap and a 24 GiB Go soft heap target.
  A passing run accepts its measured peak. A failed run does not authorize another
  memory increase.
- A pre-proof live audit found Docker's default extra 30 GiB swap allowance. The
  first 30 GiB attempt was stopped before peak memory and produced no artifact.
  Compose now sets equal memory and memory-plus-swap limits. The runner requires
  zero cgroup swap, records cgroup events, hashes the exact test binary, and uses an
  atomic host lock to prevent concurrent benchmark runs.
- A proposed cluster profile requests one node, four CPUs, 64 GiB scheduler memory,
  two hours, and at least 60 GiB free disk. The proof process receives an exact
  36 GiB no-swap cap. This profile is plumbing, not an approved ADR amendment.
- Cluster preparation may use network access without witness construction. The
  scheduled proof phase uses the prepared source tree and pinned image, then rejects
  non-loopback routes, swap, cgroup drift, source drift, and setup drift.
- A shared NUS cluster may receive only the checked-in synthetic M3 fixture. Real
  transactions, production relationships, stored caveat values, Merkle openings,
  and snapshot salts need a separate trust and data-governance decision.
- The maintainer confirmed NUS uses Slurm and has no Docker support on its nodes.
  NUS runtime selection rejects Docker. Docker remains supported on a separate
  independent Linux machine.
- The approval flag fixes the complete `cluster-36g` profile. Callers cannot
  override its memory, disk, Go heap, worker count, or benchmark identity.
- The maintainer approved one `cluster-36g` proof-creation attempt on the prepared
  independent VM on 2026-08-10. The approval permits no retry or cap increase.
- ADR-0012 records the approved 36 GiB profile. This resource amendment changes no
  proof semantics, public values, artifact encoding, key identity, or privacy rule.
- The approved 36 GiB attempt passed direct guest execution at 422,138 instructions
  in 434 milliseconds. Proof creation reached the exact 38,654,705,664-byte cgroup
  cap after the native solver reported 27,576,375 constraints. The cgroup recorded
  one OOM kill and no swap use. No proof artifact was produced.
- The pinned Gnark prover starts three L, R, and O KZG commitments after constraint
  solving. Its default multi-exponentiation task count uses
  `runtime.NumCPU() * 2`, which is separate from `GOMAXPROCS`. The failed 36 GiB
  profile set `GOMAXPROCS=4` but did not pin process CPU affinity. The approved
  56 GiB profile makes the intended four-CPU boundary visible to both controls.
- The maintainer approved one `cluster-56g` attempt on the dedicated independent VM
  on 2026-08-11. It fixes a 60,129,542,144-byte hard cap, a 66,571,993,088-byte
  minimum host, CPUs `0-3`, four Go and Rust workers, a 24 GiB Go soft heap target,
  `GODEBUG=gctrace=1`, zero swap, no network, and the existing stage deadlines.
- The GC trace is memory-only diagnostic evidence in the owner-only ignored raw log.
  The public exporter records only that `gctrace=1` was active.
- The successful `44d58c7` log contains two identical
  `benchmark_profile=cluster-56g` fields. Default export still rejects duplicates.
  An explicit recovery mode accepts exactly two identical profile fields for this
  saved log and applies every other result check unchanged. Future runs write the
  field only inside the proof container.
- Merged standard output and standard error placed the verification metric directly
  after its Rust test-harness prefix. The exporter strips only the exact expected
  test name before applying the complete ADR-0012 gate pattern.
- NUS preparation and proving require Slurm. Ambient container-runtime controls fail
  closed. The Docker standby path accepts only a local Unix-socket daemon.
- The maintainer approved baking the pinned Rust `1.97.1` Rustfmt and Clippy
  components into the toolchain image on 2026-07-28. The build receives the existing
  `RUST_VERSION` pin and installs no unversioned toolchain.
- The witness-bearing process mounts an owner-only allowlisted run bundle rather than
  the repository. Preparation and proving share one operation lock, and the public
  result binds the bundle-manifest digest.
- Cluster plumbing runs its JavaScript syntax check through the digest-pinned Node
  image. Docker and Git remain the only software prerequisites on an independent
  Linux proof host.
- The syntax-check container rejects ambient Docker controls, requires a local
  Unix-socket daemon, drops capabilities, disables networking, uses a read-only root,
  and mounts only the checked JavaScript file.
- M4 must decode and verify remote artifacts in a memory-capped, time-capped child
  process.
- B-0010 measured the combined accepted ceiling at 94,608,186 instructions and the
  depth-8 case at 93,284,394 under the provisional 100,000,000 operating maximum.

## Open Questions and Blockers

- The approved 36 GiB independent VM attempt failed at the exact cgroup memory cap.
  It produced no proof artifact.
- The approved 56 GiB attempt succeeded. It must not run again. The saved pass log
  still needs export through the narrow duplicate-profile and test-prefix recovery
  before the public benchmark result can be committed.
- The approved 7 GiB Plonk gate ended with exit 137 on three runs. Each
  cgroup peak reached exactly 7,516,192,768 bytes. No proof artifact was produced.
- The fixed `2^25` Gnark Plonk domain has a source-derived live-memory floor near
  15.75 GiB before the circuit, witness, FFT, MSM, runtime, and proof buffers.
- The earlier Docker Desktop VM exposed 33,600,331,776 bytes. Its 30 GiB no-swap
  diagnostic reached exactly 32,212,254,720 bytes and recorded one cgroup OOM kill.
  No proof artifact was produced.
- An independent NUS SoC VM exposes 67,419,770,880 bytes of RAM. It passed the Docker
  preflight and preparation gates. Its approved 36 GiB attempt recorded one OOM kill
  and produced no proof.
- The maintainer confirmed that NUS uses Slurm and does not provide Docker on its
  nodes. The exact NUS SIF runtime remains unknown. Pinned Apptainer and Singularity
  paths exist. Docker remains available for a separate independent Linux machine.
- A caller-supplied SIF SHA-256 proves handoff consistency but not approved-image
  provenance. This applies only if the experiment returns to the NUS Slurm path.
  SIF signing remains a separate dependency and key-management decision.
- The toolchain definition now includes Rustfmt and Clippy for pinned Rust `1.97.1`.
  The independent VM built the pinned x86_64 image during successful preparation.
- NUS compute-node outbound HTTPS remains unknown. The witness-free Slurm
  preparation job needs HTTPS for Cargo dependencies and the pinned setup archive.
  This no longer blocks the independent Docker path.
- Hosted or SP1 network proving remains disallowed. It would move the private witness
  outside Raziel's local trust boundary.
- SP1's transitive `proc-macro-error2 v2.0.1` emits a Rust future-compatibility
  warning under the pinned toolchain.
- The host feature-tree rerun is skipped. One unrelated locked package is absent from
  the host Cargo cache, and the gate requires offline mode. The pinned Docker
  benchmark runs the same gate before proof generation.
- ADR-0011 establishes a feature and code-path no-network boundary. M4 must add
  process-level network isolation before a service handles private witness data.
- Deferred follow-up: carry these items through each baton reset. After the M7 exit
  gate passes and the PoC meets its definition of done, remind the maintainer about
  the separate Hypha relevance-attestation scope, the Obsidian
  `Current Implementation Discussion Post`, and the TEE-backed local-authorization
  experiment in `docs/spec/POC-TECHNICAL-SPEC.md`.

## Work Completed

- M0 established the repository, toolchain, contracts, and service shells.
- M1 delivered the strict SpiceDB compiler adapter, PolicyIR semantics, and live
  equivalence harness.
- M2 delivered canonical commitments, blinded relationship snapshots, authority
  signatures, strict local registries, and deterministic cross-language vectors.
- B-0010 delivered the bounded authorization guest, exact public journal, host input
  builder, local execute-mode equality, exact invalid-status checks, and measured
  boundary fixtures.
- B-0010 is archived at
  `docs/agent/history/B-0010-sp1-guest-execute-equivalence.md`.
- Completed a post-restart sanity audit after the earlier `CpuProver` terminal
  freeze. No Cargo, SP1 runner, or project container remained. The pinned toolchain,
  guest build, result tests, serial execute suite, and both boundary fixtures passed.
- Audited the pinned SP1 `6.3.1` proof APIs. `CpuProver` is the concrete local proof
  path. `LightProver` performs real proof verification without creating proofs.
- Confirmed SP1 Core proofs grow with shard count, Compressed proofs add recursive
  aggregation, and Plonk/Groth16 target on-chain verification.
- Confirmed the SDK proof file format uses bincode `1.3.3` with a
  trailing-byte-permitting decoder.
- Confirmed `SP1_DUMP` can write the full private stdin to `stdin.bin` and exit with a
  success status.
- Drafted proposed ADR-0012 for maintainer and dependency approval.
- The maintainer approved ADR-0012 and its exact `bincode 1.3.3` and
  `sp1-core-executor 6.3.1` dependencies on 2026-07-27.
- Amended the accepted key-ID formula to bind the circuit version and SP1 program-key
  digest without binding the SDK release.
- Implemented the fixed local `CpuProver` profile, circuit-bound key identifier,
  strict proof artifact codec, unsafe SP1 environment guard, local Core prover, and
  independent `LightProver` verifier.
- Bound `LightProver` to a verifier-owned trusted guest key. Request data cannot
  select the guest ELF or verification key.
- Added canonical inner-payload re-encoding and panic-to-error containment. Recorded
  the 64 MiB limit as a wire-byte ceiling rather than a heap-allocation ceiling.
- Moved the direct authorization case into a shared execute/prove fixture and added
  proof, key-ID, journal, version, trailing-byte, TEE, malicious-length, and dump
  rejection coverage.
- Added a reproducible benchmark wrapper that prebuilds outside the measurement
  container, applies the 7 GiB cgroup cap, and runs execute, prove, verify, and
  mutation checks under separate deadlines. It records public timing, size, hardware,
  version, idle-memory, and peak-memory data.
- Completed the post-approval privacy review. The pinned Core prover commits main
  traces without an identified masking step and exposes main-trace multilinear
  openings. Recorded the security hold in ADR-0012 before any capability emission.
- Audited both SP1 outer-proof build paths. Native Plonk or Groth16 needs the
  `native-gnark` SDK feature, the bundled Go/Gnark bridge, bindgen, and libclang.
  Release proving needs preinstalled circuit artifacts. The SDK otherwise downloads
  those artifacts during the witness-bearing proof request.
- Measured the immutable release object headers. The SP1 `v6.1.0` Plonk archive is
  2,305,113,029 bytes with S3 CRC64NVME `qtkhMmLw/yQ=`. The Groth16 archive is
  6,211,807,514 bytes with S3 CRC64NVME `YwP5cL7BIIg=`. CRC64 detects corruption but
  is not the cryptographic supply-chain pin Raziel requires.
- Audited the exact SP1 Plonk dependency path. The outer circuit has five declared
  public fields. Its private recursion witness remains private input. The pinned
  Gnark prover blinds the witness polynomials with fresh field elements before
  commitment and opening.
- Compared the fork's CPU BN254 Plonk prover against official Gnark `v0.14.0`. The
  only differences are two `nolint:staticcheck` comments. The published Gnark
  private-commitment advisory states that Plonk is not affected, and `v0.14.0` is
  newer than its first patched release. OSV returned no known advisory for the
  queried Gnark, Gnark Crypto, Gnark Ignition, or Icicle Gnark versions.
- Confirmed SP1 calls `plonk.Prove` without
  `backend.WithStatisticalZeroKnowledge()`. The proposed privacy contract must claim
  computational zero knowledge only.
- The maintainer approved the Plonk privacy amendment, native build graph, pinned
  libclang package, and one-time setup artifact download on 2026-07-27.
- Downloaded the approved 2,305,113,029-byte Plonk archive and pinned SHA-256
  `b3e2f5b5dd5ca89675c73975d2d1cc4a0f9e7cde048ec5354dabc7b0694aa39b`.
- Validated archive paths, entry types, duplicate paths, entry count, and expanded
  size in the pinned Linux image. Pinned SHA-256 values for the constraints, circuit,
  proving key, and verification key.
- Built the pinned ARM64 toolchain with `native-gnark` and
  `libclang-14-dev=1:14.0.6-12`. The locked feature graph contains `blocking` and
  `native-gnark`, with no network, CUDA, or experimental SP1 feature.
- Implemented local Plonk generation, strict canonical proof bytes, wrapper-key hash
  validation, setup-file streaming hashes, a verifier-owned key, `LightProver`
  verification, nonce field reduction, and fail-closed mutation tests.
- The isolated benchmark uses a read-only root and workspace, a private `/tmp`
  tmpfs, no non-loopback route, a read-only setup mount, one proof job, and exact
  cgroup limits.
- The direct execute gate passed at 422,138 instructions in 330 to 366 ms across the
  recorded attempts.
- Three Plonk attempts hit the exact 7 GiB limit. Lower trace concurrency, a smaller
  shard preallocation, four Rust and Go workers, a 5 GiB Go soft heap target, and a
  2 GiB native executor child limit did not make the outer proof fit.
- The 30 GiB no-swap diagnostic passed the 422,138-instruction execute gate in 306
  milliseconds, then reached the exact cgroup cap. The kernel recorded one OOM kill.
  No proof artifact was produced.
- Added a clean-clone cluster workflow with separate networked preparation,
  network-isolated proof execution, and allowlisted public result export.
- Added a Slurm job template for one node, four CPUs, 64 GiB scheduler memory, and
  a two-hour wall limit.
- Added Docker, Apptainer, and Singularity runtime preflight checks. The SIF path
  requires a caller-supplied SHA-256 pin. The proof runner resolves its nested
  cgroup-v2 path through `/proc/self/cgroup`.
- Moved SP1 binaries and the guest Rust toolchain from root-owned home state to
  world-readable `/opt/raziel/sp1` paths for unprivileged HPC execution.
- Added a commit-safe Markdown exporter. It rejects dirty source, swap use, OOM
  events, missing proof gates, and missing mutation checks. It omits hostnames,
  users, scheduler accounts, scheduler job IDs, internal paths, and raw logs.
- Consolidated cluster runtime selection, positive-integer validation, and portable
  SHA-256 handling in one sourced POSIX helper.
- Removed the hostname and scheduler job ID from the benchmark log itself. Added a
  repeatable plumbing self-test for exact Compose limits and result redaction.
- Added a Slurm preparation job for the witness-free networked build stage. The
  proof job remains a separate offline Slurm job.
- Bound the preparation handoff to exact SHA-256 values for the compiled proof-test
  executable, authorization guest ELF, and tracked Plonk setup manifest. The proof
  job recomputes them and rechecks every setup-file hash and size before execution.
- NUS now has one scheduler path: Slurm plus a pinned Apptainer or Singularity SIF.
  An explicit independent-machine profile keeps Docker available.
- Added a small Slurm probe for runtime discovery, compute-node HTTPS, cgroup-v2
  memory delegation, zero-swap enforcement, network isolation, and private `/tmp`.
  Its setup URL check sends only an HTTPS HEAD request and constructs no witness.
- NUS preparation and proving now require a real Slurm job identifier. An
  unscheduled login-node invocation fails before container startup.
- Tightened the public result exporter to one exact cluster profile. It rejects
  duplicate fields, execution-path drift, malformed identities, unsafe Markdown,
  inconsistent proof gates, and values beyond the approved ceilings.
- Fixed the independent review findings for resource-profile overrides, ambient
  Apptainer and Docker controls, remote Docker contexts, permissive logs, incomplete
  probe checks, run-bundle privacy, preparation races, and report parsing.
- Added an owner-only allowlisted proof run bundle. Its manifest covers the proof
  test, guest ELF, exact source files, runner, and pinned toolchain and setup records.
- Changed the real verification gate to derive expected public values from the fixed
  authorization fixture rather than a journal file emitted by the prover.
- Rejected every `SP1_WORKER_*` setting before proving and verification. The pinned
  host toolchain no longer attempts Rust component installation during SIF use.
- Added `--no-eval` and disabled administrator-configured host, home, and current
  directory mounts for every SIF execution. Only explicit build or proof mounts
  remain.
- Rebuilt the release proof test after hardening and restaged its complete run-bundle
  manifest.
- Prepared the independent `amd64` Docker VM at source commit `810f4de`. That commit
  differs from the M3 implementation commit only by one progress-report document.
- The VM built image
  `sha256:7adfa8b55c31d686f133e94cad6a68a1a03950ef5389a6493d8673a631a952f7`,
  verified the pinned Plonk setup, compiled the guest and proof test, and staged the
  allowlisted run bundle. No witness or proof was created.
- Rebuilt the pinned local ARM64 toolchain after Docker resumed. Exact Rustfmt and
  Clippy components are present, and the full format and lint gates pass.
- Re-ran bootstrap, deterministic generation, typecheck, and full non-guest
  workspace tests in the rebuilt image. Every active gate passed. The four real
  proof tests remain isolated to the approved VM run.
- Re-ran all five prepared-guest execute cases in the rebuilt image. Host and guest
  results match, invalid private inputs fail, cycle limits hold, and both accepted
  boundary fixtures remain below the operating ceiling.
- Audited ADR-0012 and the M3 specification against the baton. The remaining exit
  evidence is the real Plonk proof, `LightProver` verification, mutation rejection,
  and the public benchmark record.
- Updated the cluster runbook to scope the approval to one independent Docker
  attempt. Slurm still needs its SIF provenance tuple and a separate execution
  approval.
- Recorded the fixed benchmark workload dimensions in ADR-0012. The direct fixture
  has four object definitions, eight relations, seven permissions, one caveat, eight
  snapshot relationships, one witness relationship, and path depth one.
- Documented the proof runner's buffered output and bounded quiet period. The runbook
  gives one read-only Docker status command and prohibits interruption or rerun.
- Reviewed the complete independent VM result. Direct guest execution passed at
  422,138 instructions in 434 milliseconds. The Plonk proof process then reached the
  exact 36 GiB cgroup cap and was OOM-killed. No proof, verification result, mutation
  result, or public benchmark report was produced.
- Implemented the approved `cluster-56g` profile with equal 56 GiB memory and
  memory-plus-swap limits, CPUs `0-3`, four affinity-visible CPUs, four Go and Rust
  workers, a 24 GiB Go soft heap target, and `GODEBUG=gctrace=1`.
- Added pre-witness runtime rejection for memory, swap, affinity-visible CPU count,
  worker count, Go heap target, and Go diagnostic drift.
- Updated the public result exporter and plumbing fixtures to require the 56 GiB
  profile, four visible CPUs, and active memory diagnostics.
- Prepared the independent `amd64` Docker VM at source commit `44d58c7`. The host
  exposed 67,419,770,880 bytes of RAM and passed the 60,129,542,144-byte proof-limit
  check. The VM built image
  `sha256:9083e064208e7e024177b6728e66171605cd012dc209c705e0ef270c6c2d23d0`,
  verified the pinned setup, compiled the guest and proof test, and staged run-bundle
  manifest
  `4ad456c05da5befdaeb17c7173f4ba5c88ddd1b5e96fffef693652cc24238026`.
- Created a real SP1 Plonk proof on the independent VM. The direct fixture executed
  422,138 instructions in 424 milliseconds. Proof generation took 1,742,106
  milliseconds and produced a 2,537-byte artifact. `LightProver` verification took
  479 milliseconds. Proof, key, and journal mutation checks failed closed.
- The proof container peaked at 46,301,016,064 bytes with zero swap, zero OOM events,
  and four affinity-visible CPUs. The proof and temporary verification files were
  deleted inside the isolated process after the gates completed.
- Traced the failed public export to one duplicate profile field. The wrapper and
  in-container benchmark each wrote the same value. Removed the wrapper copy and
  added an explicit recovery mode for the saved successful log.
- The first recovery attempt exposed the saved verification metric after the exact
  Rust test-harness prefix on the same line. Added exact test-name extraction and
  rejection coverage for unknown prefixes.

## Validation Plan

- Deterministic verification-key ID and proof artifact vectors
- Positive local proof generation and verification
- Mutated proof, key, journal, and trailing-byte rejection
- Proof size, time, verification time, instruction, CPU, and memory measurements
- Guest dependency-tree and local-only prover-path checks
- Rust focused tests and affected workspace gates
- Format, lint, typecheck, build, `git diff --check`, and private-data scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0010 archive | PASS | Bounded guest, public values, execute equality, and cycle measurements | 2026-07-24T07:36:34Z |
| B-0011 claim reconciliation | PASS | Clean `main` at `10fbc88`; scope matches the remaining M3 proof work | 2026-07-24T09:01:33Z |
| Post-restart process and Compose check | PASS | No exact Cargo process, SP1 executor runner, or running project container | 2026-07-24T09:05:33Z |
| `pnpm bootstrap:check` and pinned guest build | PASS | ARM64 toolchain pins and authorization guest ELF rebuilt | 2026-07-24T09:05:33Z |
| `cargo test -p raziel-prover --lib` | PASS, 2 tests | Exit-status and public-journal mismatch checks | 2026-07-24T09:05:33Z |
| Serial ignored SP1 suite | PASS, 5 tests | Execute cases in 7.60 seconds; boundary cases in 63.25 seconds | 2026-07-24T09:05:33Z |
| Pinned SP1 `6.3.1` source audit | PASS | Local prover roles, modes, key digest, serialization, memory defaults, and dump behavior identified | 2026-07-24T09:20:01Z |
| Docker resource query | PASS | 11 CPUs and 8,216,719,360 bytes available to the Linux VM | 2026-07-24T09:20:01Z |
| `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt checks after proposed ADR-0012 | 2026-07-24T09:20:01Z |
| Maintainer approval | PASS | ADR-0012, conditional Core gate, strict artifact, key identity, and two exact dependencies | 2026-07-27T10:16:28Z |
| Rust `1.97.1` host compile | PASS | `raziel-prover` library and all integration-test targets compile against the locked SP1 `6.3.1` APIs | 2026-07-27T10:38:03Z |
| `cargo test -p raziel-prover --tests` | PASS, 13 active tests | Artifact, key, profile, nonce, execute-result, and active `SP1_DUMP` checks; six SP1 tests remain ignored | 2026-07-27T10:38:03Z |
| SP1 SDK feature tree | PASS | `sp1-sdk 6.3.1` has only `blocking`; network, CUDA, and experimental features are absent | 2026-07-27T10:38:03Z |
| Benchmark script syntax | PASS | Both POSIX shell runners parse with `sh -n` | 2026-07-27T10:38:03Z |
| Docker availability | BLOCKED | Daemon socket is unavailable; no proof benchmark was started | 2026-07-27T10:38:03Z |
| `cargo check --locked -p raziel-prover --tests` | PASS | Trusted verifier key ownership, strict artifact codec, guard, prover, verifier, and all test targets compile | 2026-07-27T15:07:59Z |
| `cargo test --locked -p raziel-prover --lib` | PASS, 13 tests | Artifact, key, profile, nonce, guard, execution-result, and panic-containment tests | 2026-07-27T15:07:59Z |
| `cargo test --locked -p raziel-prover --tests` | PASS, 14 active and 9 ignored | All active prover tests pass; execute and real-proof tests remain gated on the pinned guest build and capped container | 2026-07-27T15:07:59Z |
| `cargo clippy --locked -p raziel-prover --tests -- -D warnings` | PASS | Edited Rust code and affected local crates | 2026-07-27T15:07:59Z |
| `sh -n` benchmark scripts | PASS | Benchmark wrapper, in-container runner, and feature gate | 2026-07-27T15:07:59Z |
| `docker compose config --quiet` | PASS | Benchmark profile and 7 GiB service configuration parse | 2026-07-27T15:07:59Z |
| Host feature-tree rerun | SKIPPED | Offline Cargo cache lacks unrelated locked `base64 0.13.1`; Docker gate remains required | 2026-07-27T15:07:59Z |
| Docker availability recheck | BLOCKED | Daemon socket remains unavailable; no proof benchmark was started | 2026-07-27T15:07:59Z |
| Pinned Core privacy source review | BLOCKED | Core commits unmasked main traces and serializes witness-derived multilinear openings; no pinned zero-knowledge assurance was found | 2026-07-27T15:14:53Z |
| SP1 outer-proof source audit | PASS | Native feature, Go and libclang build requirements, release artifact download path, and setup trust identified | 2026-07-27T15:21:29Z |
| SP1 release artifact headers | PASS | Plonk is 2,305,113,029 bytes; Groth16 is 6,211,807,514 bytes; neither path has SDK cryptographic checksum verification | 2026-07-27T15:21:29Z |
| Pinned Plonk privacy source review | PASS | Fresh L/R/O/Z blinding, five public inputs, exact fork diff, Go content hashes, and non-statistical mode identified | 2026-07-27T15:26:41Z |
| Pinned Go advisory query | PASS | OSV returned no known advisory for the four queried module versions; the result does not replace source review of the fork | 2026-07-27T15:26:41Z |
| Plonk archive download and pin | PASS | 2,305,113,029 bytes; SHA-256 `b3e2f5b5dd5ca89675c73975d2d1cc4a0f9e7cde048ec5354dabc7b0694aa39b` | 2026-07-27T20:14:00Z |
| Pinned Linux setup install | PASS | Safe archive checks and four per-file SHA-256 pins; installed read-only at `.sp1/circuits/plonk/v6.1.0` | 2026-07-27T20:21:00Z |
| Docker toolchain bootstrap | PASS | ARM64 Node, Rust, Go, Buf, Protobuf, SP1, native Gnark, and exact libclang package | 2026-07-27T20:25:00Z |
| `cargo test --locked --offline -p raziel-prover --lib` | PASS, 18 tests | Artifact, setup, key, profile, nonce, guard, execute-result, and panic-containment tests | 2026-07-27T20:32:00Z |
| Serial ignored SP1 execute suite | PASS, 5 tests | Three execute cases and two boundary cases against the rebuilt guest ELF | 2026-07-27T20:36:00Z |
| First isolated Plonk attempt | FAIL, exit 137 | 7 GiB cgroup peak; no proof artifact | 2026-07-27T20:48:00Z |
| Reduced-concurrency Plonk attempt | FAIL, exit 137 | One trace slot, `2^20` shard, four Rust and Go workers; 7 GiB cgroup peak | 2026-07-27T20:56:00Z |
| 2 GiB executor Plonk attempt | FAIL, exit 137 | Conservative native executor child limit; 7 GiB cgroup peak; no proof artifact | 2026-07-27T21:01:00Z |
| `cargo test --locked --offline -p raziel-prover --tests` | PASS, 19 active tests | Final artifact, setup, key, profile, nonce, guard, execute-result, panic-containment, and unsafe-environment coverage | 2026-07-27T21:08:02Z |
| `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt in the pinned toolchain container | 2026-07-27T21:08:02Z |
| 30 GiB no-swap Plonk diagnostic | FAIL, exit 137 | Execute passed at 422,138 instructions in 306 ms; proof reached 32,212,254,720 bytes; one cgroup OOM kill; no proof artifact | 2026-07-27T21:43:33Z |
| Cluster shell syntax and diff check | PASS | Local, Docker, Apptainer, Slurm, setup, and result scripts parse; `git diff --check` passes | 2026-07-28T06:27:26Z |
| 36 GiB Compose rendering | PASS | Exact 38,654,705,664-byte memory and memory-plus-swap limits; four workers; 24 GiB Go target; no network | 2026-07-28T06:27:26Z |
| Public result exporter self-test | PASS | Fixed public field allowlist renders; raw proof, journal, witness, hostname, user, account, and job ID remain absent | 2026-07-28T06:27:26Z |
| Pinned ARM64 toolchain rebuild | PASS | Toolchain archive size and SHA-256 pins pass; image ID `sha256:8139ef8cd17b83b420a9b3fcab6708ef0b205e6716923d8c20c1fa39c77d8122` | 2026-07-28T06:27:26Z |
| Unprivileged toolchain access | PASS | UID 65534 executes `cargo-prove 6.3.1` and `rustc 1.94.0-dev` from world-readable `/opt/raziel/sp1` | 2026-07-28T06:27:26Z |
| `cargo test --locked --offline -p raziel-prover --lib` | PASS, 21 tests | Rebuilt pinned image; artifact, setup, key, profile, guard, prover, and verifier units | 2026-07-28T06:27:26Z |
| SP1 feature gate | PASS | Rebuilt image has only `blocking` and `native-gnark`; network, CUDA, and experimental features remain absent | 2026-07-28T06:27:26Z |
| `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt checks after cluster plumbing | 2026-07-28T06:29:50Z |
| `check-sp1-plonk-plumbing` | PASS | Shared helpers parse; 36 GiB Compose limits render; public report excludes raw hostname and scheduler job ID | 2026-07-28T06:36:30Z |
| Prepared-artifact handoff check | PASS | Actual proof-test binary, guest ELF, and all four Plonk setup files pass path, type, size, and SHA-256 checks in pinned Linux | 2026-07-28T06:45:34Z |
| Slurm and runtime policy self-test | PASS | NUS rejects Docker; independent Linux accepts Docker; only Slurm templates remain | 2026-07-28T06:45:34Z |
| Cluster plumbing and exact-option self-test | PASS | Slurm-only templates, exact runtime option tokens, independent Docker standby, Compose limits, and public-result redaction | 2026-07-28T06:52:05Z |
| Cluster and benchmark shell syntax | PASS | All current POSIX shell entry points parse with `sh -n`; `git diff --check` passes | 2026-07-28T06:52:05Z |
| `cargo test --locked --offline -p raziel-prover --tests` in the pinned toolchain | PASS, 22 active and 9 ignored | All active proof codec, setup, key, profile, guard, prover, verifier, and unsafe-environment tests; real proof and guest execution remain gated | 2026-07-28T07:04:54Z |
| Strict Slurm and result-export self-test | PASS | NUS rejects unscheduled execution; Slurm and independent Docker paths render; duplicate required fields fail closed | 2026-07-28T07:04:54Z |
| `pnpm format:check` in the pinned toolchain | PASS | Biome, Rustfmt, and Gofmt after scheduler and exporter hardening | 2026-07-28T07:04:54Z |
| Independent Rust and cluster review | PASS with one external provenance question | Two read-only reviews covered proof semantics, strict decoding, resource controls, container isolation, handoff integrity, and result safety | 2026-07-28T07:23:01Z |
| Allowlisted run-bundle staging in pinned Linux | PASS | Exact approved file set, per-file SHA-256 checks, no links or special entries, owner-only modes | 2026-07-28T07:23:01Z |
| Expanded 256 MiB isolation probe in Docker-equivalent cgroup v2 | PASS | Exact memory, zero swap, complete cgroup evidence, read-only mounts, tmpfs, and no network | 2026-07-28T07:23:01Z |
| Strict profile and runtime-control self-tests | PASS | Memory override, Apptainer bind injection, remote Docker endpoint, and unscheduled NUS path fail closed | 2026-07-28T07:23:01Z |
| Release proof-test rebuild and run-bundle restage | PASS | Hardened Rust source compiled in release mode; exact binary, guest, runner, source, and tool manifests rehashed | 2026-07-28T07:30:23Z |
| Final cluster plumbing self-test | PASS | Both execution paths render; wrong profile, duplicate field, and mismatched verification key fail closed | 2026-07-28T07:30:23Z |
| `cargo fmt --all -- --check` and `git diff --check` | PASS | Hardened Rust code and complete worktree diff | 2026-07-28T07:30:23Z |
| Latest pinned-container `pnpm format:check` | BLOCKED after Biome pass | Runtime component installation is now disabled; image lacks approved `rustfmt` and `clippy` components | 2026-07-28T07:30:23Z |
| Pinned-container `pnpm typecheck` split gate | PASS for TypeScript and Rust; Go blocked before compilation | Four TypeScript packages and the Cargo workspace check passed. The offline container has no cached `github.com/authzed/authzed-go@v1.9.0`, so `GOPROXY=off` rejected module lookup without fetching it | 2026-07-28T07:36:30Z |
| `pnpm -r --if-present test` in the pinned toolchain | PASS, 20 tests | Console, gateway, generated protocol package, and TypeScript SDK | 2026-07-28T07:36:30Z |
| `cargo test --locked --offline --workspace --exclude raziel-authorization-guest` | PASS | Full non-guest Rust workspace. Five prepared-guest execution tests and four real Plonk proof gates remain ignored by design | 2026-07-28T07:36:30Z |
| Resume reconciliation and local static checks | PASS | Baton returned to `ACTIVE`; exact Rust `1.97.1` Rustfmt, shell syntax, Node syntax, Compose rendering, cluster plumbing, `git diff --check`, private-string scan, and private-artifact ignore rules | 2026-07-28T11:38:56Z |
| Focused host Clippy rerun | NOT AVAILABLE | The native Gnark build reached `sp1-recursion-gnark-ffi` and stopped because the host has no Go binary. No Clippy diagnostic was emitted | 2026-07-28T11:38:56Z |
| Pinned-container Biome, Buf, Gofmt, and Clippy rerun | NOT AVAILABLE | Docker daemon socket is absent after the maintainer pause. No container validation process started | 2026-07-28T11:38:56Z |
| Pinned Rustfmt and Clippy toolchain amendment | PASS | Maintainer approval recorded; Compose passes `RUST_VERSION` into the image build; Dockerfile installs only Rustfmt and Clippy for exact Rust `1.97.1`; Compose, plumbing, formatting, syntax, and diff checks pass | 2026-07-28T11:41:08Z |
| Independent-host prerequisite review | PASS | README and ADR-0003 require a container-first toolchain. Removed the accidental host Node call from cluster plumbing and documented the Docker-host requirements | 2026-08-10T08:28:08Z |
| Revised plumbing static checks | PASS | Shell syntax, scoped diff review, and `git diff --check` pass without a host Node invocation | 2026-08-10T08:28:08Z |
| Revised plumbing live Docker check | PASS | Docker `29.6.2` pulled the digest-pinned Node `24.18.0` image and completed `check-sp1-plonk-plumbing` without host Node | 2026-08-10T08:33:01Z |
| Independent VM preparation | PASS | `amd64` Docker `29.7.2`, source `810f4de`, pinned image, setup, guest ELF, proof test, and run-bundle hashes recorded. No proof created | 2026-08-10T08:26:40Z |
| Maintainer proof-run approval | PASS | One synthetic `cluster-36g` attempt with a 36 GiB hard cap, zero swap, four workers, no network, and no automatic retry or cap increase | 2026-08-10T08:42:29Z |
| Pinned ARM64 toolchain rebuild | PASS | Image `sha256:be87a5a2364b2bdd3f6841653e48de4a832cdb3bcbcfac919e8fa248e8b59624` contains Node `24.18.0`, Rustfmt `1.9.0`, and Clippy `0.1.97` | 2026-08-10T08:55:09Z |
| `pnpm format:check` in rebuilt image | PASS | Biome, Rustfmt, and Gofmt pass | 2026-08-10T08:55:09Z |
| `pnpm lint` in rebuilt image | PASS | Biome, full non-guest workspace Clippy with warnings denied, Go vet, and Buf lint pass | 2026-08-10T08:55:09Z |
| `pnpm bootstrap:check` in rebuilt image | PASS | Exact Node, pnpm, Rust, Cargo, Go, Buf, Protobuf, SP1, well-known type, and ARM64 pins | 2026-08-10T08:59:28Z |
| `pnpm generate:check` in rebuilt image | PASS | Two Buf generations match byte for byte | 2026-08-10T08:59:28Z |
| `pnpm typecheck` in rebuilt image | PASS | Four TypeScript packages, all non-guest Rust targets, and every Go package compile | 2026-08-10T08:59:28Z |
| `pnpm test` in rebuilt image | PASS | 20 TypeScript tests, 130 active Rust tests, and all Go tests pass. Nine Rust tests are ignored by design, including four VM proof gates | 2026-08-10T08:59:28Z |
| Hardened Docker-only plumbing check | PASS | Local Docker path completes with a pinned unprivileged Node container. Injected `DOCKER_CONTEXT` fails closed with exit 1 | 2026-08-10T09:01:00Z |
| Prepared-guest execute suite | PASS, 3 tests | Host and guest equality, invalid private input, and cycle-limit behavior | 2026-08-10T09:05:14Z |
| Prepared-guest boundary suite | PASS, 2 tests | Accepted input ceiling and maximum snapshot depth remain below the execution-cycle limit | 2026-08-10T09:05:14Z |
| M3 closure and ADR consistency audit | PASS | ADR-0012 records the approved fixed 36 GiB attempt. The specification and baton agree on the four remaining VM proof gates | 2026-08-10T09:07:34Z |
| Cluster approval-boundary review | PASS | Runbook permits one prepared independent Docker attempt and does not reuse that approval for Slurm, retries, cap increases, or profile changes | 2026-08-10T09:10:07Z |
| M3 benchmark-context audit | PASS | ADR-0012 states the fixed policy structure, relationship counts, witness count, path depth, proof mode, and instruction count | 2026-08-10T09:11:54Z |
| Final scoped diff and privacy review | PASS | Plumbing self-test passes, injected Docker controls exit 1, `git diff --check` passes, and no private or generated proof artifact is tracked | 2026-08-10T09:13:13Z |
| Proof timeout and cleanup review | PASS | Execute, prove, verify, and mutation stages have 2, 60, 1, and 5 minute limits. Output is buffered, and success or failure renames the in-progress log | 2026-08-10T09:14:25Z |
| External-result blocked audit | BLOCKED | Local M3 gates are exhausted. Review requires the complete output from the one approved independent VM run | 2026-08-10T09:15:27Z |
| Approved 36 GiB independent VM proof attempt | FAIL, cgroup OOM kill | Execute passed at 422,138 instructions in 434 ms; solver reported 27,576,375 constraints; memory peaked at 38,654,705,664 bytes with zero swap; no proof artifact | 2026-08-10T09:09:15Z |
| Pinned post-solver memory source audit | PASS | Gnark retains solved L/R/O polynomials and starts three KZG commitments; default multi-exponentiation sizing uses `runtime.NumCPU() * 2`, not `GOMAXPROCS` | 2026-08-10T09:29:52Z |
| Local Docker CPU-affinity check | PASS | `GOMAXPROCS=4` alone exposed 11 CPUs through `nproc`; a four-CPU cpuset exposed four | 2026-08-10T09:29:52Z |
| Approved 56 GiB profile implementation | PASS | Compose renders exact 60,129,542,144-byte memory and memory-plus-swap limits, CPUs `0-3`, `GODEBUG=gctrace=1`, four workers, and no network | 2026-08-10T17:32:05Z |
| Revised cluster plumbing self-test | PASS | Profile overrides, CPU-affinity drift, Go diagnostic drift, mismatched keys, duplicate fields, unsafe runtime controls, and private result fields fail closed | 2026-08-10T17:32:05Z |
| Live Compose CPU-affinity check | PASS | The approved benchmark service reports four CPUs through `nproc` | 2026-08-10T17:32:05Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt pass after the profile amendment | 2026-08-10T17:32:05Z |
| Scoped diff check | PASS | `git diff --check` reports no whitespace errors | 2026-08-10T17:32:05Z |
| Pinned-container `pnpm lint` | PASS | Biome, Clippy with warnings denied, Go vet, and Buf lint pass; the existing `proc-macro-error2` future-compatibility warning remains | 2026-08-10T17:33:35Z |
| Independent VM 56 GiB preparation | PASS | `amd64` Docker `29.7.2`, source `44d58c7`, exact 56 GiB limit, pinned image, setup, guest ELF, proof test, and run-bundle hashes recorded. No proof created | 2026-08-10T17:38:09Z |
| Approved 56 GiB independent VM proof attempt | PASS | Execute, real Plonk generation, `LightProver` verification, and mutation gates passed; 46,301,016,064-byte peak, zero swap, and zero OOM events | 2026-08-10T18:16:44Z |
| Commit-safe public result export | FAIL | The saved pass log contains two identical `benchmark_profile` fields; all proof gates completed before this post-proof parser failure | 2026-08-10T18:16:44Z |
| Duplicate-profile exporter recovery tests | PASS | Default duplicate rejection, exact matching-value recovery, conflicting-value rejection, future wrapper output, Compose profile, and result redaction checks pass | 2026-08-10T18:23:24Z |
| `./scripts/dev pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt checks after the exporter repair | 2026-08-10T18:23:24Z |
| Recovery repair diff check | PASS | Shell syntax and `git diff --check` pass | 2026-08-10T18:23:24Z |
| First saved-log recovery attempt | FAIL | The verification metric follows its exact Rust test-harness prefix on the same line; no public Markdown file was written | 2026-08-10T18:30:38Z |
| Test-harness gate extraction regression | PASS | Exact verification-test prefix is accepted; unknown prefix, duplicate profile conflict, and all prior result mutations fail closed | 2026-08-10T18:30:38Z |
| Latest recovery format and diff checks | PASS | Pinned-container format checks, shell syntax, and `git diff --check` pass after exact test-prefix handling | 2026-08-10T18:30:38Z |

## Privacy Checks

- PASS: B-0010 agent-facing outputs contain no private PolicyIR, relationship tuple,
  selected path, Merkle opening, caveat value, or snapshot salt.
- PASS: B-0010 production code contains no logging call for guest input or witness
  data.
- PASS: B-0010 public values contain only ADR-0011 fields.
- PASS: The verifier policy source remains its trusted local registry.
- PASS: The independent `LightProver` accepted the real proof, exact trusted key, and
  exact public journal.
- PASS: Core remains excluded from agent-facing and verifier-facing capabilities.
- PASS: The approved Plonk path created a real proof and passed its mutation run.
  The raw log exposed no private witness output.
- PASS: Real transaction data and private runtime artifacts remain outside Git.
- PASS: The cluster result exporter copies only public aggregate fields and source
  identities. Raw scheduler and proof logs remain ignored.
- PASS: The benchmark log itself omits hostname and scheduler job ID.
- PASS: The NUS path is limited to the synthetic M3 fixture pending a separate
  decision for any real or production-derived data.
- PASS: The witness-bearing container receives only the allowlisted run bundle and
  pinned setup directory. It does not receive the repository or ignored private
  files.

## Risks and Private-Data Notes

- Proof serialization can accidentally duplicate or expose the private stdin. The
  artifact contract must include only the proof and approved public journal.
- A verification-key ID that is not derived from stable key material can permit key
  substitution or break registry lookup.
- Proof logs and benchmarks must not print guest input, witness data, context values,
  salts, or selected paths.
- Proof generation can consume substantial local CPU, memory, and time. Record exact
  hardware and stop on resource exhaustion.

## Exact Next Action

On the VM, pull the pushed duplicate-profile exporter repair. Then recover the saved
public result without rerunning the proof:

```bash
git pull --ff-only origin main
./scripts/cluster/render-sp1-plonk-result \
  --recover-duplicate-profile \
  .sp1/benchmarks/cluster-20260810T174732Z-76324.pass.log
```

Return the generated public Markdown result. Do not rerun the proof.
