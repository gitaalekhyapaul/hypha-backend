# Current Engineering Baton

- Baton ID: B-0011
- State: READY
- Milestone: M3, SP1 execution and proof
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-24T07:36:34Z
- Branch: `main`
- Worktree: B-0010 is archived and ready for its scoped commit. Reconcile Git state
  before claiming B-0011.

## Objective

Define and implement the second M3 slice: local SP1 proof generation, verification-key
identity, proof artifact encoding, local verification, and proof benchmarks. Stop
before `verifierd`, capability job APIs, or gateway wiring.

## Acceptance Criteria

- [ ] One accepted decision record fixes the local proof mode, verification-key ID,
      proof artifact encoding, public-journal handling, and failure rules.
- [ ] A concrete local `CpuProver` creates a proof without selecting an SP1 network
      prover.
- [ ] The pinned local verifier accepts the proof, exact verification key, and exact
      public journal.
- [ ] Proof, verification-key, and public-journal mutations fail closed.
- [ ] At least one representative allowed case records proof time, proof size,
      verification time, instruction count, hardware, and tool versions.
- [ ] Focused and affected workspace gates pass.

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
- Cargo metadata only after dependency review and approval
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
- B-0010 measured the combined accepted ceiling at 94,608,186 instructions and the
  depth-8 case at 93,284,394 under the provisional 100,000,000 operating maximum.

## Open Questions and Blockers

- Decision required: select the pinned SP1 proof mode for the PoC artifact. Core,
  compressed, and succinct on-chain modes have different proof size, runtime, and
  verification dependencies.
- Decision required: define `verification_key_id` from stable SP1 verification-key
  material rather than a display string or process-local object.
- Decision required: define stable proof bytes for the Protobuf `zk_proof` field and
  reject alternate or trailing encodings.
- Measurement required: confirm local ARM64 CPU and memory cost for the selected mode.
- SP1's transitive `proc-macro-error2 v2.0.1` emits a Rust future-compatibility
  warning under the pinned toolchain.
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
| Not run | B-0011 is unclaimed | Proof generation and verification | 2026-07-24T07:36:34Z |

## Privacy Checks

- PASS: B-0010 agent-facing outputs contain no private PolicyIR, relationship tuple,
  selected path, Merkle opening, caveat value, or snapshot salt.
- PASS: B-0010 production code contains no logging call for guest input or witness
  data.
- PASS: B-0010 public values contain only ADR-0011 fields.
- PASS: The verifier policy source remains its trusted local registry.
- NOT APPLICABLE: Independent proof verification is the B-0011 and M4 work.
- PASS: Real transaction data and private runtime artifacts remain outside Git.

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

After the B-0010 commit, claim B-0011, inspect the pinned SP1 `6.3.1` local proving,
verification, key, and serialization APIs, then draft ADR-0012 choices for maintainer
approval before implementation.
