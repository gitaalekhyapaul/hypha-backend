# B-0010: SP1 Guest Input and Execute Equivalence

- Baton ID: B-0010
- Final state: completed
- Milestone: M3, SP1 execution and proof
- Owner: Codex
- Started UTC: 2026-07-23T16:40:41Z
- Closed UTC: 2026-07-24T07:36:34Z
- Branch: `main`
- Base commit: `07a5a5a`

## Objective

Define and implement the first M3 slice: the bounded SP1 authorization guest input,
exact `AuthorizationPublicValues`, host input builder, and execute-mode equivalence.
Stop before local proof generation.

## Acceptance Criteria

- [x] One accepted decision record fixes the versioned guest input, private and public
      split, public-value encoding, request binding, witness-consumption rules, and
      failure rules.
- [x] The host input builder rejects malformed, oversized, duplicate, ambiguous, and
      out-of-order inputs before guest execution.
- [x] The guest recalculates the policy commitment, verifies every relationship
      opening, evaluates PolicyIR and caveats, rejects unused witness entries, and
      commits only `ALLOW`.
- [x] Execute-mode cases match the Rust host evaluator for the approved supported
      subset.
- [x] Public-value and witness field mutations fail closed.
- [x] Focused and affected workspace gates pass.

## Architecture Constraints

- The baseline guest proves `ALLOW` only.
- The agent never receives PolicyIR, relationship tuples, paths, openings, salts, or
  stored caveat values.
- Raw Protobuf bytes are never commitment inputs.
- Every proof binds an approved policy record and signed snapshot descriptor.
- The capability carries `policy_hash`, not PolicyIR.
- Guest public values contain only the fields named in
  `AuthorizationPublicValues`.
- `verifierd` never calls `razield`.
- Host-only signing and registry code cannot enter the SP1 guest dependency graph.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `ARCHITECTURE.md` and `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/decisions/0011-sp1-guest-io-and-public-values.md`
- `proto/raziel/v1/` for an approved public-value wire contract
- `programs/authorization-guest/`
- `crates/prover/` and `crates/witness/`
- Guest-compatible shared types in `crates/canonical/`, `crates/policy-ir/`, and
  `crates/snapshot/` only where the accepted contract needs them
- Execute-mode equivalence, mutation vectors, and tests
- Cargo metadata after dependency review and approval
- Baton records

## Out of Scope

- Local proof generation and proof benchmarking
- Proof verification inside `verifierd`
- Capability job APIs, request signatures, and replay control
- Next.js gateway, SDK, and console work
- Real transaction data
- Service process wiring, which remains M4 work
- A denial proof or private-policy model
- Hypha dataset relevance attestation

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0010.
- M2 supplies canonical PolicyIR commitments, blinded relationship openings, signed
  snapshot descriptors, and verifier-local registry semantics.
- ADR-0011 fixes the guest input, key-derived subject, caveat input binding, request
  and scope commitments, nullifier, witness usage, public journal bytes, guest
  ceilings, and local SP1 execute boundary.
- The maintainer approved the new `authorization-core` crate, key-derived subject
  rule, and pinned SP1 host and guest dependency changes on 2026-07-24.
- The maintainer approved the ADR-0011 amendment that uses local `LightProver` for
  execute-only B-0010 checks and reserves local `CpuProver` for proof generation.

## Open Questions and Blockers

- Resolved by ADR-0011: versioned guest input, canonical public journal bytes, shared
  crate boundary, request binding, and deterministic unused-witness accounting.
- Resolved: the pinned toolchain now contains
  `protobuf-compiler=3.21.12-3+deb12u1` and
  `libprotobuf-dev=3.21.12-3+deb12u1`. Bootstrap checks both `libprotoc 3.21.12` and
  `google/protobuf/empty.proto`.
- Resolved by maintainer approval: execute-only checks use local `LightProver`. The
  next M3 proof slice will use local `CpuProver`.
- Resolved by measured gates: the combined accepted ceiling uses 94,608,186
  instructions and the maximum-depth case uses 93,284,394. The 100,000,000 value is
  a provisional operating maximum, not a proof over every accepted input combination.
  Execution-budget exhaustion fails closed without an `ALLOW` journal.
- ADR-0011 establishes an explicit CPU-prover code path and disables SP1 network
  proving features. SP1 retains HTTP and RPC transitive libraries. M4 must define an
  operating-system no-egress boundary before private witness data enters a service.
- The normal development `spicedb` service has a pre-existing command and key
  mismatch. Record its effect on live equivalence work during baton claim.
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
- B-0009 is archived as
  `docs/agent/history/B-0009-authority-signatures-and-local-registries.md`.
- Reconciled B-0010 against clean `main` at `07a5a5a`.
- Confirmed that the existing capability Protobuf names the public-value fields but
  does not define canonical journal bytes, the private guest input, request and scope
  commitments, or unused-witness accounting.
- Completed an independent protocol-security review. It identified missing subject
  binding, unbound caveat inputs, missing witness consumption, undefined derived
  claims, and caller-controlled ceilings.
- Accepted ADR-0011 after explicit maintainer approval.
- Added guest-compatible serde transport without making serde bytes a commitment
  input.
- Added deterministic evaluator usage tracking while preserving the M1 Boolean API
  and error semantics.
- Added `authorization-core` with compiled ceilings, key-derived agent subjects,
  fixed capability claims, verified-witness resolution, exact witness consumption,
  request and scope commitments, issuance nullifiers, and strict public journal
  bytes.
- Added checked-in public-value vectors and a deterministic vector generator.
- Wired the SP1 guest to read one private input, run the shared statement, and commit
  one canonical public byte slice.
- Added a local execute-only SP1 host executor and explicit ignored execute
  integration gate.
- Added typed request context to the issuance Protobuf transport and regenerated
  ignored generated outputs.
- Added focused request-binding tests for every committed request field, each public
  scope field, private fields excluded from the scope hash, nullifier changes, all ten
  fixed capability claims, unsupported claim declarations, malformed context, nonce,
  versions, timestamps, and request lengths.
- Added strict public-journal tests for every protocol length, domain, version,
  decision, time interval, truncation, trailing bytes, and byte mutation.
- Corrected the architecture host responsibility to collect only the selected
  positive path. Supplying every snapshot relationship would violate exact witness
  consumption.
- Completed independent evaluator and SP1 boundary reviews. No critical defect
  remained after review.
- Fixed stored caveat context accounting so repeated use of one opening consumes
  witness and relationship-context budgets once.
- Added independent ADR-0009 structural identity checks to witness-set verification
  and the generic evaluator. Direct and caveated forms cannot create two candidates
  for one tuple identity.
- Added multi-success trace tests for subject-set and arrow paths, traced CEL
  error-absorption tests, overlapping usage tests, and later-branch error propagation.
- Made the 100,000,000-cycle value a compiled maximum. Zero and larger caller
  overrides return a typed error.
- Added raw SP1 rejection cases that bypass host prevalidation and require guest
  failure without a successful journal.
- Restored every pre-existing transitive Cargo package to its prior locked version.
  The remaining lockfile additions are required by the approved SP1 SDK graph.
- Re-ran the full non-prover workspace tests, Clippy with warnings denied, formatting,
  and patch-integrity checks after the independent review fixes.
- Added the approved pinned Protobuf compiler to the container toolchain, Compose
  build arguments, bootstrap version check, and amended M0 toolchain decision.
- Rebuilt the toolchain and confirmed `libprotoc 3.21.12`. Prover compilation now
  reaches the SP1 build script and identifies the separate well-known-type package
  requirement.
- Added the approved pinned Protobuf development package and a direct bootstrap check
  for `google/protobuf/empty.proto`.
- Rebuilt the toolchain, passed its full bootstrap gate, compiled `raziel-prover`, and
  rebuilt the authorization guest ELF.
- Started the execute-mode tests first with Rust's default parallelism and then
  serially. Both runs were stopped without a result after the concrete `CpuProver`
  spent the test window initializing or computing. A read-only Docker status check
  confirmed active work at about 100% CPU, 5.0 GiB resident memory, and 29 processes.
- Reviewed the pinned SP1 SDK source. Its concrete `LightProver` exists for local
  execution and verification only. It does not generate proofs.
- Amended ADR-0011 after maintainer approval and replaced the execute-only
  `CpuProver` path with concrete local `LightProver`.
- Enabled SP1 gas reporting so cycle limits and instruction metrics are active.
  Added an independent host check for every non-success guest status.
- Passed execute-mode equality for direct, caveated, union, intersection,
  subject-set, and arrow cases at 422,138 to 518,217 instructions.
- Required exact SP1 status `1` for typed semantic rejection and status `3` for
  malformed serialized stdin. Every rejection leaves the public journal empty.
- Added host validation tests for mutated, truncated, and extended journal bytes
  after a successful guest exit.
- Rebuilt the boundary fixture to combine a 1,046,611-byte policy, all 64 static
  permission nodes, a selected 33-node permission plus one arrow target, all 64
  caveat nodes, a selected 34-node caveat, all 16 maximum-length request-context
  values, 16 used witnesses, and the deepest supported one-arrow path.
- Passed the combined boundary at 94,608,186 instructions and the separate
  256-relationship, depth-8 boundary at 93,284,394 instructions.
- Completed a final independent review. Three medium evidence and test-specificity
  findings were fixed. No critical or high-severity finding remained.

## Validation Plan

- Deterministic guest input and public-value vectors
- Host and guest equivalence for every approved positive case
- Malformed, oversized, unused, duplicate, ambiguous, and out-of-order witness tests
- One-field public-value and witness mutations
- SP1 execute-mode tests without proof generation
- Guest dependency-tree check
- Rust focused tests and affected workspace gates
- Format, lint, typecheck, build, generation, `git diff --check`, and private-data scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0009 archive | PASS | M2 authority signatures and local registries completed with all gates | 2026-07-23T11:45:01Z |
| Baton start reconciliation | PASS | Clean `main` at `07a5a5a`; B-0010 matches M3 | 2026-07-23T16:40:41Z |
| Guest contract inspection | PASS | Public field names exist; proof-critical guest input and bytes remain open | 2026-07-23T16:40:41Z |
| Protocol-security review | PASS | Required subject, caveat, witness, commitment, and ceiling rules identified | 2026-07-23T16:56:36Z |
| ADR-0011 approval | PASS | Maintainer approved all three requested M3 protocol and dependency items | 2026-07-23T16:56:36Z |
| `cargo test -p raziel-policy-ir` | PASS | 25 tests, including deterministic direct, union, intersection, subject-set, and arrow usage traces | 2026-07-23T17:34:02Z |
| `cargo test -p raziel-authorization-core` | PASS | 16 tests, including statement, request binding, fixed claims, strict journal decoding, real opening, and vector coverage | 2026-07-24T03:54:44Z |
| `cargo prove build --locked --packages raziel-authorization-guest` | PASS | Guest compiled with SP1 rustc 1.94.0-dev | 2026-07-23T17:34:02Z |
| Guest normal dependency tree | PASS | No Prost, authority, verifier, service, SP1 SDK, or network client in guest graph | 2026-07-23T17:34:02Z |
| `cargo test -p raziel-prover --lib` | BLOCKED | `sp1-prover-types` build script could not find `protoc` | 2026-07-23T17:34:02Z |
| Non-prover Rust workspace tests | PASS | Every workspace crate except the guest and protoc-blocked prover | 2026-07-24T04:10:59Z |
| Non-prover Rust workspace Clippy | PASS | All targets with warnings denied, excluding guest and prover | 2026-07-24T04:10:59Z |
| `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-07-24T04:10:59Z |
| `pnpm generate:check` | PASS | Two independent Buf generations matched | 2026-07-24T03:54:44Z |
| `buf lint` and main-branch breaking check | PASS | Protobuf style and compatibility | 2026-07-24T03:54:44Z |
| TypeScript typecheck and tests | PASS | Four package typechecks and 20 tests | 2026-07-24T03:54:44Z |
| Go test and vet | PASS | SpiceDB compiler adapter and equivalence packages | 2026-07-24T03:54:44Z |
| Private-data logging scan | PASS | No runtime logging of guest input, witness, context, salts, or resolver state | 2026-07-24T03:54:44Z |
| `git diff --check` | PASS | B-0010 worktree | 2026-07-24T04:10:59Z |
| Independent evaluator review | PASS after fixes | Context reuse and structural identity findings fixed with regressions | 2026-07-24T04:08:12Z |
| Independent SP1 boundary review | PASS after fixes | Raw rejection coverage, cycle maximum, egress wording, and lock attribution corrected | 2026-07-24T04:08:12Z |
| Focused evaluator, snapshot, and authorization tests | PASS | 64 tests after review fixes | 2026-07-24T04:08:12Z |
| Focused evaluator, snapshot, and authorization Clippy | PASS | All targets with warnings denied | 2026-07-24T04:08:12Z |
| `./scripts/compose build toolchain` | PASS | Pinned `protobuf-compiler=3.21.12-3+deb12u1` installed in the native ARM64 image | 2026-07-24T04:19:07Z |
| `pnpm bootstrap:check` | PASS | All tool pins, including `libprotoc 3.21.12` and SP1 `8252c29` | 2026-07-24T04:19:07Z |
| `cargo test -p raziel-prover --lib` | BLOCKED | SP1 imports `google/protobuf/empty.proto`; Debian supplies it through the unapproved `libprotobuf-dev` package | 2026-07-24T04:19:07Z |
| `./scripts/compose build toolchain` | PASS | Approved `libprotobuf-dev=3.21.12-3+deb12u1` and its well-known types installed | 2026-07-24T06:22:49Z |
| `pnpm bootstrap:check` | PASS | All tool pins plus `google/protobuf/empty.proto` | 2026-07-24T06:22:49Z |
| `cargo test -p raziel-prover --lib` | PASS | SP1 host SDK and `raziel-prover` compile | 2026-07-24T06:22:49Z |
| `cargo prove build --locked --packages raziel-authorization-guest` | PASS | Authorization guest rebuilt with pinned SP1 guest toolchain | 2026-07-24T06:22:49Z |
| Execute-mode tests with `CpuProver` | INCONCLUSIVE | Parallel and serial runs stopped after sustained ARM64 CPU and memory use without a completed test | 2026-07-24T06:22:49Z |
| ADR-0011 `LightProver` amendment | PASS | Maintainer approved execute-only local `LightProver`; proof generation retains local `CpuProver` | 2026-07-24T06:22:49Z |
| SP1 execute equivalence and rejection suite | PASS, 3 tests | Six positive paths, low cycle limit, ten typed semantic rejections, and one malformed serialized input | 2026-07-24T07:36:34Z |
| SP1 boundary suite | PASS, 2 tests | Combined accepted ceiling at 94,608,186 instructions; depth-8 case at 93,284,394 | 2026-07-24T07:36:34Z |
| Prover result validation tests | PASS, 2 tests | Non-success status plus mutated, truncated, and extended successful journals fail closed | 2026-07-24T07:36:34Z |
| Independent final SP1 review | PASS after fixes | Cost coverage, exact invalid statuses, malformed stdin, journal mismatch coverage, and stale baton state corrected | 2026-07-24T07:36:34Z |
| `cargo test --workspace --exclude raziel-authorization-guest` | PASS | Full Rust workspace, including final prover result tests | 2026-07-24T07:36:34Z |
| Workspace and focused Clippy | PASS | All Rust targets with warnings denied | 2026-07-24T07:36:34Z |
| `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `pnpm build` | PASS | TypeScript, Rust, Go, Buf, Next.js, guest, and build gates | 2026-07-24T07:36:34Z |
| Format, generation, Buf compatibility, and guest dependency tree | PASS | Deterministic generated output, main compatibility, and no host authority or service crate in guest graph | 2026-07-24T07:36:34Z |
| Logging, credential, and `git diff --check` scans | PASS | No production logging call, common credential signature, whitespace error, or private runtime artifact | 2026-07-24T07:36:34Z |

## Privacy Checks

- PASS: Agent-facing outputs contain no private PolicyIR, relationship tuple, selected
  path, Merkle opening, caveat value, or snapshot salt.
- PASS: Production code contains no logging call for guest input, witness data,
  request context, salts, or resolver state.
- PASS: Capability public values contain only the fields approved in ADR-0011.
- PASS: The verifier resolves policy through its trusted local registry.
- NOT APPLICABLE: Proof verification without a prover connection belongs to the next
  M3 slice and service wiring belongs to M4.
- PASS: Real transaction data and private runtime artifacts remain outside Git and
  generated output.

## Risks and Private-Data Notes

- Guest inputs contain private witness material and must never enter normal logs,
  capability artifacts, browser responses, or checked-in runtime files.
- Public-value bytes become proof-system protocol. Any field, order, width, or
  canonicalization change needs mutation vectors and a decision record.
- The host builder cannot silently remove unused witness data to make a guest case
  pass.
- A host evaluator result cannot replace SP1 execute-mode agreement.
- The 100,000,000 instruction limit has a 5,391,814-instruction measured margin. It
  remains provisional. Accepted combinations outside the boundary fixtures can
  exhaust the budget and fail closed.
- SP1's transitive `proc-macro-error2 v2.0.1` emits a Rust future-compatibility
  warning. The current pinned toolchain still compiles and tests it.

## Exact Next Action

Archive B-0010, reset the active baton to B-0011, and define the bounded local
`CpuProver` proof-generation and verification-key-binding slice.
