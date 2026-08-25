# Current Engineering Baton

- Baton ID: B-0031
- State: ACTIVE
- Milestone: M4, retained proof through the independent verifier
- Owner: Codex
- Started UTC: 2026-08-25T09:08:27Z
- Last updated UTC: 2026-08-25T18:17:44Z
- Branch: `main`
- Worktree: Clean at pushed commit `fd9ecd6` before correcting the stale public
  benchmark parser.

## Objective

Verify one retained public synthetic Plonk artifact through the isolated worker with
every `razield` role stopped, then measure the verification-only resource profile.

## Acceptance Criteria

- [x] The maintainer explicitly approves one synthetic Plonk rerun and retention of
      its public zero-knowledge proof artifact.
- [ ] The approved VM profile produces the proof without retaining witness, private
      journal input, or debug traces.
- [ ] `verifierd` verifies the retained proof through its isolated worker with all
      `razield` roles stopped.
- [ ] Mutated proof, public journal, key identity, policy, root, scope, expiry, and
      request-signature cases fail closed.
- [ ] Verification-only peak memory stays under the provisional 2 GiB cap or a new
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

## Open Questions And Blockers

- The NUS VM completed the B-0031 proof benchmark with zero OOM events. Public report
  rendering then rejected the new 424,312-instruction line against its stale
  422,138-only pattern. The parser defect is corrected and validated locally. The
  temporary public export was deleted, so one VM rerun remains required.

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
- The host test now pins that measurement as the B-0031 signable direct fixture.
- Commit `1492613` contains the measured instruction-pin correction.
- The fixed 60 GiB disk rejection is removed. Host preflight still records exact
  available worktree bytes for diagnostics.
- Commit `950601b` contains the disk-floor removal and corrected runbooks.
- The benchmark parser now accepts the validated historical M3 count and current
  B-0031 count. Its current-fixture plumbing vector uses 424,312 instructions.
- No retained public proof fixture exists yet.

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

## Privacy Checks

- PASS: the planned retained artifact is public zero-knowledge proof material only.
- PASS: staging rejects any file outside the four public payloads and writes only
  checksums plus public provenance.
- REQUIRED: no witness, private snapshot, path, opening, salt, or caveat value enters
  the fixture or logs.
- REQUIRED: verifier success must occur with every `razield` role stopped.

## Risks And Private-Data Notes

- The public proof artifact is safe to retain only after a no-witness and no-debug
  audit.
- The provisional 2 GiB cap lacks a real verification-only peak measurement.
- The proof rerun is computationally expensive and belongs on the NUS VM.
- Security review SR-B0031-1: the direct synthetic fixture changes its agent
  thumbprint to the digest of a public test key. This changes fixture commitments and
  requires a fresh proof. It does not change the public-value schema, hash algorithm,
  canonical encoding, authorization semantics, or production key handling.

## Exact Next Action

Validate and push the corrected benchmark parser, then refresh the prepared NUS VM
bundle and rerun the retained proof command once.
