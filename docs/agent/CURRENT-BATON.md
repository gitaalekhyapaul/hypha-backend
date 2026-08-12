# Current Engineering Baton

- Baton ID: B-0014
- State: READY
- Milestone: M4, verification-only prover feature split
- Owner: unassigned
- Started UTC: None
- Last updated UTC: 2026-08-12T06:49:12Z
- Branch: `main`
- Worktree: B-0013 is archived at
  `docs/agent/history/B-0013-bullmq-redis-conformance.md`. Its BullMQ `1.2.3` and
  Redis `8.2.8` conformance gate passes. M4 remains active.

## Objective

Implement the approved `raziel-prover` Cargo feature split. A verification-only build
must keep the artifact codec, key identifier, and `LightProver` verification path. It
must exclude native Gnark proving, `sp1-core-executor`, `CpuProver`, guest execution,
local proving, and proving-profile code. Preserve the existing proving build and all M3
behavior. Do not run another proof.

## Acceptance Criteria

- [ ] `raziel-prover` declares explicit verification and proving feature sets.
- [ ] The normal proving build preserves current execution, proving, and test APIs.
- [ ] A verification-only build compiles the artifact and `LightProver` path.
- [ ] The verification-only dependency tree excludes `sp1-core-executor`,
      `sp1-prover`, native Gnark, `CpuProver`, execution, proving, and profile modules.
- [ ] `verifierd` selects the verification-only feature set without adding service
      behavior.
- [ ] Focused tests, affected workspace tests, Clippy, formatting, and diff checks
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
  keep SP1 blocking verification and exclude native proving and execution paths.
- Inherited: No other crate or system dependency is approved for this baton.
- Made: None.

## Open Questions and Blockers

- None block this feature-only slice.
- A later positive isolated-verifier gate still needs approval for one synthetic Plonk
  rerun and retention of its public proof artifact.
- ADR-0002 and O-007 remain open before the M4 stolen-capability exit gate can close.

## Work Completed

- B-0013 pinned and validated BullMQ `1.2.3`, Tokio `1.53.1`, and Redis `8.2.8`.
- The current `raziel-prover` package has one combined dependency surface. It imports
  `sp1-core-executor` and enables the workspace `sp1-sdk` native-Gnark feature for
  every consumer.
- Existing M3 proof and verification behavior passes the full non-guest workspace
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

## Privacy Checks

- NOT STARTED: No witness, relationship, caveat, opening, or proof data enters this
  feature-only slice.
- NOT STARTED: Verification-only dependency inspection must show no proving path.
- NOT APPLICABLE: This baton does not process real transaction data.

## Risks and Private-Data Notes

- Cargo features are additive. A workspace-level `native-gnark` feature can silently
  defeat a member crate's verification-only selection.
- Default-feature changes can break M3 test builds or remove required verification
  codecs. Test both feature sets.
- A verification-only crate that still links executor or prover code weakens the M4
  verifier isolation claim even if that code is never called.

## Exact Next Action

After the B-0013 checkpoint commit, claim B-0014 and inspect the exact module and
dependency feature closure before editing Cargo manifests. No new approval is needed
for the accepted feature split.
