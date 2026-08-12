# Engineering Baton History

- Baton ID: B-0015
- State: REVIEW
- Milestone: M4, generated Rust service contract compilation
- Owner: Codex
- Started UTC: 2026-08-12T07:56:01Z
- Last updated UTC: 2026-08-12T08:06:55Z
- Branch: `main`
- Worktree: B-0015 implementation and documentation changes are ready to commit on
  `main` after `381794d`. B-0014 is archived at
  `docs/agent/history/B-0014-verification-only-prover-feature-split.md`.

## Objective

Compile the already generated Rust Tonic client and server bindings through an
explicit `raziel-proto` service feature. Pin only the approved Tonic dependencies.
Keep message-only consumers, including the authorization guest graph, free from Tonic.
Stop before implementing handlers, listeners, transport policy, or service behavior.

## Acceptance Criteria

- [x] Tonic `0.14.6` and `tonic-prost` `0.14.6` are workspace-pinned and locked.
- [x] `raziel-proto` exposes generated service bindings only under an explicit feature.
- [x] `razield` and `verifierd` select that service-contract feature.
- [x] Generated Snapshot, Capability, and Verification clients and servers compile.
- [x] The message-only `raziel-proto` graph and authorization guest graph exclude
      Tonic, Tokio, HTTP transport, and service codecs.
- [x] Generated files remain generator-owned and unchanged by manual patches.
- [x] Focused tests, affected workspace tests, Clippy, generation checks, formatting,
      and diff checks pass.

## Architecture Constraints

- Protobuf and Buf own cross-language wire contracts.
- Generated code stays outside manual patches.
- Raw Protobuf bytes are never commitment inputs.
- `verifierd` never calls `razield`.
- Compiling service traits does not authorize network listeners or outbound clients.
- The authorization guest and proof semantics cannot depend on RPC runtime crates.
- Request and response ceilings remain service-layer work for a later baton.

## File Scope

- `Cargo.toml`
- `Cargo.lock`
- `crates/proto/Cargo.toml`
- `crates/proto/src/lib.rs`
- `crates/proto/tests/` only for generated-symbol and feature coverage
- `services/razield/Cargo.toml`
- `services/verifierd/Cargo.toml`
- Existing feature-tree scripts only if needed for a durable guest exclusion gate
- `README.md`, `ARCHITECTURE.md`, and `docs/spec/POC-TECHNICAL-SPEC.md` only for
  current behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Tonic servers, listeners, handlers, interceptors, TLS, or Unix-socket transport
- Capability admission, queue publication, private envelopes, proof execution, or
  verifier-worker execution
- Public error mapping or audit receipts
- Protobuf schema or generated-file changes
- Another proof run or retained proof fixture
- ADR-0002 protected-request signatures
- Next.js, SDK, gateway, or console behavior
- Real transaction data, M6 acceptance, or another scenario

## Decisions

- Inherited: ADR-0001 and ADR-0003 through amended ADR-0013 are accepted.
- Inherited: The maintainer approved Tonic `0.14.6` and `tonic-prost` `0.14.6` on
  2026-08-12.
- Inherited: Tokio `1.53.1` is already approved and pinned. This contract-only slice
  must not add features or use its runtime.
- Inherited: No other crate, CLI, runtime, or system dependency is approved here.
- Made: `raziel-proto` keeps service bindings disabled by default. The `services`
  feature selects Tonic `codegen` and `transport` without Tonic's default `router`.
- Made: `clippy::empty_docs` is allowed only for `raziel-proto` service-feature
  builds, since the pinned generator emits empty RPC doc comments.

## Open Questions and Blockers

- None block generated contract compilation.
- Generated Tonic code exists under `gen/rust-services` and is now included only by
  the opt-in `raziel-proto` `services` feature.
- The authorization guest does not currently depend on `raziel-proto`. The durable
  feature-tree gate rejects RPC runtime packages from both the guest and the default
  message-only crate.
- ADR-0002 and O-007 remain open before the M4 stolen-capability exit gate can close.
- A later positive isolated-verifier gate still needs explicit proof-run approval.
- Documentation review found stale O-008 markers. ADR-0013's approval record accepts
  the 75-minute Plonk window and closes O-008, but its header, one consequence, and
  the technical-spec open-decision table still say it is open. B-0016 will reconcile
  those records without changing the accepted semantics.

## Work Completed

- B-0014 isolated direct Plonk verification from SDK proving and execution packages.
- Buf already generates Rust messages under `gen/rust` and Tonic bindings under
  `gen/rust-services` through pinned remote plugins.
- The generated file contains Snapshot, Capability, and Verification client and
  server modules.
- B-0015 claimed at commit `381794d` with a clean worktree.
- Confirmed that generated clients require `tonic::transport` and that neither
  service crate currently selects `raziel-proto`.
- Pinned Tonic and `tonic-prost` `0.14.6`. Tonic enables only `codegen` and
  `transport`; its default `router` feature remains disabled.
- Added an opt-in `raziel-proto` `services` feature and compiled all three generated
  client and server pairs through it.
- Selected the service feature in `razield` and `verifierd` without adding handlers,
  listeners, or client construction.
- Extended the dependency-tree gate to require the approved service-contract pins and
  reject RPC runtime packages from message-only and guest graphs.
- Scoped `clippy::empty_docs` suppression to `raziel-proto` builds with the generated
  service feature. The pinned generator emits empty RPC doc comments; generated files
  remain unchanged.

## Validation Plan

- Compile and test `raziel-proto` with and without its service feature.
- Compile `razield` and `verifierd` against the generated contracts.
- Inspect the message-only and authorization guest dependency graphs for RPC crates.
- Run `pnpm generate:check` without modifying generated output.
- Run the affected non-guest workspace tests.
- Run focused Clippy, repository formatting, shell checks, and `git diff --check`.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0014 archive | PASS | Verification-only prover feature split | 2026-08-12T07:50:04Z |
| `git status --short` | PASS, empty | B-0015 start state | 2026-08-12T07:56:01Z |
| `cargo test -p raziel-proto --features services` | PASS, 1 test | Generated clients and servers | 2026-08-12T07:58:03Z |
| `cargo test --locked --offline -p raziel-proto --no-default-features` | PASS | Message-only crate | 2026-08-12T07:58:42Z |
| `cargo test --locked --offline -p razield` | PASS, 4 tests; 4 live Redis tests ignored | Service contract consumer | 2026-08-12T07:59:17Z |
| `cargo test --locked --offline -p verifierd` | PASS, 1 test | Service contract and isolated verifier consumer | 2026-08-12T07:58:58Z |
| `./scripts/dev ./scripts/check-sp1-features` | PASS | SP1, verifier, service-contract, message, and guest graphs | 2026-08-12T07:58:58Z |
| `sh -n scripts/check-sp1-features` | PASS | Feature-tree gate syntax | 2026-08-12T07:58:42Z |
| First service-feature Clippy run | FAIL | Generated Tonic file emits ten `clippy::empty_docs` findings | 2026-08-12T08:01:34Z |
| Include-level Clippy suppression check | FAIL | Rust ignores lint attributes on `include!` macro invocations | 2026-08-12T08:02:18Z |
| `pnpm generate:check` | PASS | Two clean generated outputs match | 2026-08-12T08:01:34Z |
| `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-12T08:01:34Z |
| Final service-feature Clippy with `-D warnings` | PASS | Generated contracts and both service consumers | 2026-08-12T08:02:58Z |
| Full non-guest Cargo workspace | PASS | Ordinary Rust tests; 9 SP1 and 4 live Redis tests ignored | 2026-08-12T08:05:35Z |
| Final message-only Clippy with `-D warnings` | PASS | Default `raziel-proto` graph | 2026-08-12T08:06:14Z |
| Final feature-tree gate | PASS | SP1, verifier, service, message, and guest graphs | 2026-08-12T08:06:14Z |
| Final generated-file diff | PASS, empty | `gen/rust` and `gen/rust-services` | 2026-08-12T08:06:14Z |
| Final `pnpm format:check` and `git diff --check` | PASS | Repository formatting and patch hygiene | 2026-08-12T08:06:14Z |

## Privacy Checks

- PASS: Contract compilation introduces no runtime payload, handlers, or logs.
- PASS: Message-only and guest graphs remain free from service runtime packages.
- NOT APPLICABLE: This baton does not process proof, witness, or real transaction data.

## Risks and Private-Data Notes

- Cargo features are additive. A future guest dependency on `raziel-proto` must keep
  default features disabled. The durable dependency-tree gate checks this boundary.
- Generated clients provide outbound connection APIs. Their presence does not grant
  `verifierd` permission to construct a `razield` client.
- Generated default message ceilings are not accepted service limits. Later handlers
  must set the ADR-0013 ceilings explicitly.

## Exact Next Action

Commit the B-0015 generated-contract checkpoint. Then claim B-0016 and reconcile the
stale O-008 status markers with ADR-0013's accepted approval record before capability
service code relies on the 75-minute Plonk window.
