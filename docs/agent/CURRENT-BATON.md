# Current Engineering Baton

- Baton ID: B-0015
- State: READY
- Milestone: M4, generated Rust service contract compilation
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-12T07:50:04Z
- Branch: `main`
- Worktree: B-0014 is archived at
  `docs/agent/history/B-0014-verification-only-prover-feature-split.md`. Its strict
  verifier feature graph and artifact compatibility gates pass. M4 remains active.

## Objective

Compile the already generated Rust Tonic client and server bindings through an
explicit `raziel-proto` service feature. Pin only the approved Tonic dependencies.
Keep message-only consumers, including the authorization guest graph, free from Tonic.
Stop before implementing handlers, listeners, transport policy, or service behavior.

## Acceptance Criteria

- [ ] Tonic `0.14.6` and `tonic-prost` `0.14.6` are workspace-pinned and locked.
- [ ] `raziel-proto` exposes generated service bindings only under an explicit feature.
- [ ] `razield` and `verifierd` select that service-contract feature.
- [ ] Generated Snapshot, Capability, and Verification clients and servers compile.
- [ ] The message-only `raziel-proto` graph and authorization guest graph exclude
      Tonic, Tokio, HTTP transport, and service codecs.
- [ ] Generated files remain generator-owned and unchanged by manual patches.
- [ ] Focused tests, affected workspace tests, Clippy, generation checks, formatting,
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
- Made: None.

## Open Questions and Blockers

- None block generated contract compilation.
- Generated Tonic code currently exists under `gen/rust-services` but
  `raziel-proto` does not include it.
- The shared message crate reaches the SP1 guest through PolicyIR. Service bindings
  must be feature-gated to prevent RPC dependencies from entering that graph.
- ADR-0002 and O-007 remain open before the M4 stolen-capability exit gate can close.
- A later positive isolated-verifier gate still needs explicit proof-run approval.

## Work Completed

- B-0014 isolated direct Plonk verification from SDK proving and execution packages.
- Buf already generates Rust messages under `gen/rust` and Tonic bindings under
  `gen/rust-services` through pinned remote plugins.
- The generated file contains Snapshot, Capability, and Verification client and
  server modules.

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

## Privacy Checks

- NOT STARTED: Contract compilation must introduce no runtime payload or logs.
- NOT STARTED: Message-only and guest graphs must remain free from service runtime
  packages.
- NOT APPLICABLE: This baton does not process proof, witness, or real transaction data.

## Risks and Private-Data Notes

- Cargo features are additive. Enabling service bindings on the shared message crate
  at workspace scope can silently contaminate guest builds.
- Generated clients provide outbound connection APIs. Their presence does not grant
  `verifierd` permission to construct a `razield` client.
- Generated default message ceilings are not accepted service limits. Later handlers
  must set the ADR-0013 ceilings explicitly.

## Exact Next Action

After the B-0014 checkpoint commit, claim B-0015 and inspect the generated Tonic file
and current generation checks. Then add the smallest feature-gated contract surface
without implementing a service handler.
