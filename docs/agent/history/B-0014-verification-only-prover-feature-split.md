# Engineering Baton History

- Baton ID: B-0014
- State: REVIEW
- Milestone: M4, verification-only prover feature split
- Owner: Codex
- Started UTC: 2026-08-12T06:57:24Z
- Last updated UTC: 2026-08-12T07:50:04Z
- Branch: `main`
- Worktree: B-0014 implementation and documentation changes are ready to commit on
  `main` after `26429b2`. B-0013 is archived at
  `docs/agent/history/B-0013-bullmq-redis-conformance.md`.

## Objective

Implement the amended `raziel-prover` Cargo feature split. A verification-only build
must keep the artifact decoder, key identifier, and direct SP1 Plonk verification
path. It must exclude `sp1-sdk`, native Gnark proving, `sp1-core-executor`,
`sp1-prover`, `CpuProver`, guest execution, local proving, and proving-profile code.
Preserve the existing proving build and all M3 behavior. Do not run another proof.

## Acceptance Criteria

- [x] `raziel-prover` declares explicit verification and proving feature sets.
- [x] The normal proving build preserves current execution, proving, and test APIs.
- [x] A verification-only build compiles the artifact and direct Plonk verifier path.
- [x] The verification-only dependency tree excludes `sp1-core-executor`,
      `sp1-prover`, `sp1-sdk`, native Gnark, `CpuProver`, execution, proving, and
      profile modules.
- [x] `verifierd` selects the verification-only feature set without adding service
      behavior.
- [x] Focused tests, affected workspace tests, Clippy, formatting, and diff checks
      pass.

## Architecture Constraints

- `verifierd` never calls `razield`.
- Request data cannot select the guest ELF, verification key, or proof mode.
- The verifier resolves `policy_hash` through its trusted local policy registry.
- The agent never receives PolicyIR, relationships, paths, openings, salts, caveat
  values, or witness traces.
- No hosted prover or SP1 network path receives private witness data.
- Plonk remains the only public capability proof mode.
- Core proofs remain barred from capabilities.
- Raw Protobuf bytes are never commitment inputs.
- Policy authorization and dataset relevance remain separate systems.

## File Scope

- `Cargo.toml`
- `Cargo.lock` only if Cargo changes feature-resolution metadata
- `crates/prover/Cargo.toml`
- `crates/prover/src/lib.rs`
- `crates/prover/src/` only for feature-boundary fixes
- `crates/prover/tests/` only for feature-gate coverage
- `services/verifierd/Cargo.toml`
- `scripts/check-sp1-features`
- `docs/decisions/0013-m4-service-and-verifier-execution.md`
- `docs/agent/CURRENT-BATON.md`
- Current-state documentation only if the proven feature boundary changes its status

## Out of Scope

- Tonic or Tokio service implementation
- Capability admission, private envelopes, proof jobs, or worker supervision
- `verifierd` RPC ingress or child-process behavior
- Changes to proof bytes, public values, commitments, keys, or verification semantics
- Another SP1 proof run or retained proof fixture
- ADR-0002 protected-request signatures
- Next.js gateway, SDK, or console behavior
- Real transaction data, M6 acceptance, or another scenario

## Decisions

- Inherited: ADR-0001 and ADR-0003 through ADR-0013 are accepted.
- Inherited: ADR-0012 permits Plonk for public capabilities and limits Core to
  non-sensitive internal measurements.
- Inherited: ADR-0013 approves the `raziel-prover` feature split. Verification builds
  use direct `sp1-verifier` `6.3.1` Plonk verification and exclude SDK proving and
  execution paths.
- Inherited: No other crate or system dependency is approved for this baton.
- Made: The maintainer approved the ADR-0013 direct-verifier correction and direct
  `sp1-verifier = 6.3.1` dependency on 2026-08-12.

## Open Questions and Blockers

- RESOLVED: SP1 SDK `6.3.1` declares `sp1-core-executor` and `sp1-prover` as
  unconditional dependencies. Its `blocking` feature does not create a lightweight
  verifier graph. The accepted `LightProver` wording cannot meet ADR-0013's standalone
  feature-tree exclusion.
- RESOLVED: The maintainer approved direct
  `sp1-verifier = { version = "=6.3.1", default-features = false }` verification
  against one verifier-owned paired program-key record. ADR-0013 records the
  amendment.
- Rejected fallback: retain SDK `LightProver` and permit `sp1-prover` plus
  `sp1-core-executor` in the verifier binary. That weakens the accepted isolation
  boundary and expands the verifier's compiled attack surface.
- Waiting for a future SP1 SDK feature split avoids the amendment but stops M4 at this
  baton for an unbounded period.
- A later positive isolated-verifier gate still needs approval for one synthetic Plonk
  rerun and retention of its public proof artifact.
- ADR-0002 and O-007 remain open before the M4 stolen-capability exit gate can close.

## Work Completed

- B-0013 pinned and validated BullMQ `1.2.3`, Tokio `1.53.1`, and Redis `8.2.8`.
- Source inspection confirmed that `sp1-sdk` `6.3.1` imports `sp1-core-executor` and
  `sp1-prover` without feature guards. Its `LightProver` wraps `SP1LightNode` from
  `sp1-prover` and performs ELF setup through that node.
- Amended ADR-0013 to use direct `sp1-verifier` Plonk verification and one atomic
  verifier-registry key record containing the Raziel key ID and SP1 BN254 key hash.
- Added explicit `proving` and `verification` Cargo features. The default proving
  surface retains the SDK, CpuProver, executor, input, profile, and artifact APIs.
- Added a strict compatibility decoder for the existing M3 proof artifact. A unit
  test proves byte-for-byte agreement with the SDK payload layout.
- Replaced runtime SDK verification with `sp1-verifier::PlonkVerifier`. The verifier
  checks the trusted key ID, SP1 key hash, public journal, all five public inputs,
  encoded metadata, success exit code, recursion key root, and Plonk proof.
- Centralized the pinned SDK and circuit versions and checked the circuit value
  against SDK `6.3.1` in proving builds.
- Extended `scripts/check-sp1-features` with a durable `verifierd` dependency gate.
  Its selected direct dependencies are `bincode`, `serde`, and `sp1-verifier` only.
- Updated current-state documents without claiming the later retained-proof worker
  gate.

## Validation Plan

- Compile and test the normal proving feature set without running ignored SP1 proof
  gates.
- Compile and test `raziel-prover` with verification features only.
- Inspect Cargo feature trees for native proving, executor, and network paths.
- Run the affected non-guest workspace tests.
- Run focused Clippy, repository formatting, and `git diff --check`.
- Confirm that no proof, witness, key, registry, or private artifact changed.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0013 archive | PASS | BullMQ and Redis conformance gate | 2026-08-12T06:49:12Z |
| `./scripts/dev cargo test --locked --offline --workspace --exclude raziel-authorization-guest` | PASS | Pre-split non-guest workspace baseline | 2026-08-12T06:49:12Z |
| `cargo tree --locked --offline -p raziel-prover -e features` and SP1 `6.3.1` manifest inspection | FAIL | Accepted `LightProver` isolation assumption | 2026-08-12T07:04:38Z |
| Maintainer approval and ADR-0013 amendment | APPROVED | Direct `sp1-verifier` `6.3.1` and paired trusted key record | 2026-08-12T07:20:18Z |
| `./scripts/dev cargo test --locked --offline -p raziel-prover --no-default-features --features verification --lib` | PASS | Six verification-only decoder, key, conversion, and fail-closed tests | 2026-08-12T07:50:04Z |
| `./scripts/dev cargo test --locked --offline -p raziel-prover` | PASS | Twenty-eight unit tests, one safe-environment integration test, nine resource-heavy tests ignored | 2026-08-12T07:50:04Z |
| `./scripts/dev ./scripts/check-sp1-features` | PASS | Required proving features plus verifier SDK, prover, executor, JIT, native-Gnark, and network-client exclusions | 2026-08-12T07:50:04Z |
| `./scripts/dev cargo tree --locked --offline -p verifierd --depth 2` | PASS | Direct graph is `bincode`, `serde`, and `sp1-verifier` | 2026-08-12T07:50:04Z |
| `./scripts/dev cargo test --locked --offline --workspace --exclude raziel-authorization-guest` | PASS | Full affected non-guest Rust workspace, with documented proof gates ignored | 2026-08-12T07:50:04Z |
| Strict Clippy for both `raziel-prover` features and `verifierd` | PASS | `-D warnings` | 2026-08-12T07:50:04Z |
| `./scripts/dev pnpm format:check`, shell syntax, and `git diff --check` | PASS | Final repository hygiene | 2026-08-12T07:50:04Z |

## Privacy Checks

- PASS: No witness, relationship, caveat, opening, proof, key, or registry artifact
  entered the worktree or test output.
- PASS: Verification-only tests use synthetic public proof structures and public key
  identifiers. They contain no private authorization input.
- PASS: The `verifierd` dependency gate excludes SDK proving, execution, native Gnark,
  JIT, and outbound SP1 clients.
- PASS: `verifierd` still has no `razield` dependency or client.
- NOT APPLICABLE: This baton does not process real transaction data.

## Risks and Private-Data Notes

- Cargo features remain additive. The durable feature-tree gate must run after any
  SP1 or `verifierd` dependency change.
- The paired program-key record relies on trusted registry installation. A later
  service slice must reject unpaired or request-supplied key representations.
- Byte compatibility and malformed-proof tests pass. A real retained public Plonk
  artifact has not run through this direct verifier path. That later M4 gate still
  needs explicit approval and the approved VM profile.
- The upstream `proc-macro-error2 2.0.1` future-incompatibility warning remains in the
  proving graph and predates this slice.

## Exact Next Action

Archive B-0014, commit the validated feature split, and open B-0015 to compile the
already generated Tonic service bindings into `raziel-proto` with the approved Tonic
and `tonic-prost` pins. Do not implement service handlers in that contract-only slice.
