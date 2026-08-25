# Current Engineering Baton

- Baton ID: B-0031
- State: ACTIVE
- Milestone: M4, retained proof through the independent verifier
- Owner: Codex
- Started UTC: 2026-08-25T09:08:27Z
- Last updated UTC: 2026-08-25T19:25:57Z
- Branch: `main`
- Worktree: Permanent retained-verifier orchestration fixes are committed at
  `a8340ef`. The retained VM proof is bound to `35db270`; its public fixture and
  reports remain uncommitted on the VM.

## Objective

Verify one retained public synthetic Plonk artifact through the isolated worker with
every `razield` role stopped, then measure the verification-only resource profile.

## Acceptance Criteria

- [x] The maintainer explicitly approves one synthetic Plonk rerun and retention of
      its public zero-knowledge proof artifact.
- [x] The approved VM profile produces the proof without retaining witness, private
      journal input, or debug traces.
- [x] `verifierd` verifies the retained proof through its isolated worker with all
      `razield` roles stopped.
- [x] Mutated proof, public journal, key identity, policy, root, scope, expiry, and
      request-signature cases fail closed.
- [x] Verification-only peak memory stays under the provisional 2 GiB cap or a new
      decision records the measured requirement.
- [ ] Durable public evidence records source commit, artifact checksum, key ID,
      timing, peak memory, and zero swap.

## Architecture Constraints

- `verifierd` never calls `razield`.
- Retain only the public Plonk proof, public journal, key identifiers, checksums, and
  public provenance. Delete all witness and private inputs.
- Use direct `sp1-verifier 6.3.1` in the fixed one-shot worker.
- Do not change the public-value schema, canonical hashes, proof mode, or resource
  limits inside the measurement task.
- The synthetic fixture may bind the existing approved M4 agent-signature semantics.

## File Scope

- Retained public proof fixture and provenance manifest
- Isolated-verifier integration and mutation tests
- Verifier benchmark evidence
- SP1 host preflight profile and B-0031 operator runbooks
- ADR-0012 resource-profile amendment
- M4 current-state and baton documents

## Out Of Scope

- Gateway nonce storage, cache, and TypeScript canonicalization
- M5 frontend and synthetic transaction workflow
- M6 real transaction acceptance
- M7 or another scenario

## Decisions

- Inherited: accepted ADR-0012 and ADR-0013 permit local Plonk only.
- Inherited: B-0030 supplies the strict live service and isolated verifier worker.
- Made: The maintainer approved one synthetic Plonk rerun and public-artifact
  retention on 2026-08-25. The approval does not permit retaining witness, private
  journal input, or debug traces.
- Made: The synthetic direct fixture now uses the SHA-256 thumbprint of the public
  RFC 8032 Ed25519 test key. The former arbitrary bytes had no signing key, so they
  could not exercise the protected-request signature gate.
- Made: The maintainer approved removal of the unmeasured fixed 60 GiB free-disk
  preflight on 2026-08-26. Available disk remains diagnostic output. CPU, memory,
  swap, network, proof deadline, and artifact-size gates remain fixed.
- Made: The maintainer approved replacing the exact synthetic-fixture instruction
  equality with a positive count capped at 520,000 on 2026-08-26. Exact counts remain
  benchmark measurements and do not enter runtime authorization or proof semantics.

## Open Questions And Blockers

- The NUS VM retained the public proof at source commit `35db270`. The first verifier
  attempt lacked ignored generated Rust protocol bindings. The second exposed a
  `0700` public-fixture directory that the capability-dropped test could not traverse.
  The third loaded the fixture but exposed a verifier-client UID mismatch and missing
  worker-health wait. The source-preserving compatibility run passed verification,
  all mutations, and public report rendering after matching Cargo's full
  module-qualified test name. The public fixture and reports remain uncommitted on
  the VM.

## Work Completed

- B-0030 completed the strict live verifier service and controlled isolated worker.
- The verification-only release binaries build and run in the approved Compose
  sandbox.
- The proof process exports exactly four public files only after proof, baseline
  verification, and mutation gates pass. Default proof runs still delete them.
- A strict staging boundary validates the proof report, file allowlist, checksums,
  key identity, source identity, resource profile, and zero-swap result.
- A test-only fixture loader rejects extra files, links, malformed manifests,
  checksum drift, oversized public files, and private-retention claims.
- The verifier-only acceptance role covers the positive proof and proof, public
  journal, key, policy, root, scope, expiry, and request-signature mutations.
- The networkless acceptance client can write its disposable Cargo target and read
  the pinned offline dependency metadata. The measured worker remains non-root.
- The NUS VM operator runbook records the exact proof and verifier commands.
- The NUS VM passed the host, run-bundle, Plonk-setup, cgroup, and execute preflight.
  The current direct fixture measured 424,312 instructions before proof generation.
- The host test records that measurement and enforces the 520,000-instruction bound.
- Commit `1492613` contains the measured instruction-pin correction.
- The fixed 60 GiB disk rejection is removed. Host preflight still records exact
  available worktree bytes for diagnostics.
- Commit `950601b` contains the disk-floor removal and corrected runbooks.
- The benchmark parser records any positive count through 520,000. Its plumbing gate
  rejects 520,001 before accepting benchmark evidence.
- The wider hard-coded-value audit found no other exact real-run measurement gate.
  Exact protocol, profile, queue, encoding, and supply-chain checks remain exact. The
  renderer now requires proof artifact size in `1..=1,048,576` and tests the exact
  instruction ceiling rather than the incidental current measurement.
- The retained public proof fixture exists on the NUS VM with proof SHA-256
  `d2014a6d012fc8b8013d87c226a2e54822bdd6a47f13405d676fc334a49558f9`.
- The preparation path now generates ignored Protobuf bindings with a writable Buf
  cache. The verifier runner rejects missing generated bindings before Cargo starts.
- The public fixture stager now writes its directory as `0755`; all six public files
  remain non-writable to other users.
- The acceptance client now shares the private IPC owner UID without Linux
  capabilities. A one-shot init owns its disposable Cargo cache, and Compose waits
  for the verifier worker to become healthy.
- The final retained-proof run verified in 447 ms. All eight mutation cases rejected.
  The isolated worker peaked at 8,146,944 bytes with zero swap and OOM events.
- The report parser now matches Cargo's full retained-test name.

## Validation Plan

- Run the approved proof once on the NUS VM profile.
- Transfer only the public artifact and public provenance evidence.
- Verify positive and mutation cases through the isolated worker with `razield`
  stopped.
- Rerun the focused prover, verifier, feature-tree, privacy, formatting, lint, and
  Compose gates.
- Run shell and cluster-plumbing checks after removing the fixed disk floor.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0030 archive | PASS | Strict live verifier and isolated worker | 2026-08-25T07:22:35Z |
| `cargo test -p verifierd --all-targets` | PASS | 25 active tests, 2 ignored controlled gates | 2026-08-25T09:35:22Z |
| `cargo clippy -p verifierd --all-targets -- -D warnings` | PASS | Verifier code and tests | 2026-08-25T09:35:22Z |
| `./scripts/cluster/check-sp1-plonk-plumbing` | PASS | Public export and strict staging self-test | 2026-08-25T09:35:22Z |
| `./scripts/check-verifier-sandbox-compose` | PASS | Verifier sandbox and acceptance role | 2026-08-25T09:35:22Z |
| Acceptance-client Docker smoke | PASS | Read-only source, offline metadata, writable disposable target | 2026-08-25T09:48:28Z |
| Signable direct fixture test | PASS | RFC 8032 key thumbprint binding | 2026-08-25T09:35:22Z |
| Full workspace test | STOPPED | Full SP1 proving graph compile reached 25.16 GiB; focused gates used | 2026-08-25T09:48:28Z |
| Retained real proof | NOT RUN | Manual NUS VM action after commit and push | 2026-08-25T09:35:22Z |
| VM evidence availability | NOT AVAILABLE | No retained fixture or verifier benchmark after three checks | 2026-08-25T10:37:14Z |
| B-0031 VM execute gate | FAIL | Measured 424,312 instructions; stale assertion expected 422,138; no proof started | 2026-08-25T17:38:15Z |
| `cargo fmt --check` and `git diff --check` | PASS | Instruction pin and decision record | 2026-08-25T17:40:16Z |
| Pinned container prover test | NOT AVAILABLE | Local Docker daemon is stopped | 2026-08-25T17:41:53Z |
| Host prover test fallback | FAIL | SP1 Gnark build requires Go from the pinned Docker image | 2026-08-25T17:41:53Z |
| B-0031 VM proof preflight | FAIL | Unmeasured 60 GiB free-disk floor rejected prepared run before Docker started | 2026-08-25T18:00:07Z |
| Shell syntax and `git diff --check` | PASS | Disk-floor removal and affected scripts | 2026-08-25T18:01:53Z |
| `./scripts/cluster/check-sp1-plonk-plumbing` | PASS | Locked profile, Compose, exporter, and retained-artifact staging | 2026-08-25T18:01:53Z |
| B-0031 VM proof benchmark | PASS, host post-processing failed | Proof completed with zero OOM events; stale execute-line parser rejected 424,312 instructions; public export deleted | 2026-08-25T17:53:21Z |
| Shell syntax, `git diff --check`, and cluster plumbing | PASS | Current and historical execute-line parser formats plus retained-artifact staging | 2026-08-25T18:17:44Z |
| Hard-coded-value audit and cluster plumbing | PASS | Instruction count and proof size use positive bounds; exact protocol and profile checks retained | 2026-08-25T18:30:28Z |
| B-0031 retained Plonk proof | PASS | Public proof retained; zero proof OOM and swap events; proof SHA-256 `d2014a6d...58f9` | 2026-08-25T18:23:53Z |
| First retained-verifier attempt | FAIL | Missing ignored generated Rust protocol contract | 2026-08-25T18:58:05Z |
| Second retained-verifier attempt | FAIL | Public fixture directory was `0700`; strict loader failed closed | 2026-08-25T19:01:16Z |
| Local verifier UID and socket check | PASS | Healthy worker; capability-dropped host UID can access private `0700` IPC and owned test cache | 2026-08-25T19:08:00Z |
| Retained-fixture unit tests | PASS, 2 tests | Exact file allowlist, checksums, provenance, and mutation rejection | 2026-08-25T19:08:00Z |
| Shell syntax, Compose gate, and cluster plumbing | PASS | Preparation, public fixture mode, verifier identity, and readiness changes | 2026-08-25T19:08:00Z |
| B-0031 isolated retained verifier | PASS, report parser failed | Real proof verified in 442 ms; eight mutations rejected; 9,355,264-byte peak; zero swap and OOM; no network; read-only root | 2026-08-25T19:21:30Z |
| Verifier report parser syntax | PASS | Full Cargo module-qualified test name replaces stale short name | 2026-08-25T19:21:30Z |
| B-0031 final isolated verifier | PASS | Real proof verified in 447 ms; eight mutations rejected; 8,146,944-byte peak; zero swap and OOM; no network; read-only root; all `razield` roles stopped | 2026-08-25T19:22:14Z |

## Privacy Checks

- PASS: the planned retained artifact is public zero-knowledge proof material only.
- PASS: staging rejects any file outside the four public payloads and writes only
  checksums plus public provenance.
- PASS: no witness, private snapshot, path, opening, salt, or caveat value enters
  the fixture or logs.
- PASS: verifier succeeded with every `razield` role stopped.

## Risks And Private-Data Notes

- The public proof artifact passed the no-witness and no-debug staging audit.
- The provisional 2 GiB verifier cap has more than 1.99 GiB of headroom against the
  measured 8,146,944-byte peak.
- The unused generic local benchmark entrypoint still defaults to the known-failing
  30 GiB diagnostic profile. The approved cluster path overrides it with the fixed
  56 GiB profile. Retiring or requiring explicit selection for the generic path is a
  separate maintenance change.
- Security review SR-B0031-1: the direct synthetic fixture changes its agent
  thumbprint to the digest of a public test key. This changes fixture commitments and
  requires a fresh proof. It does not change the public-value schema, hash algorithm,
  canonical encoding, authorization semantics, or production key handling.

## Exact Next Action

Fast-forward the VM, review the exact public-file allowlist, and commit the retained
public fixture plus the Plonk and verifier benchmark reports. No proof or verifier
rerun remains.
