# Current Engineering Baton

- Baton ID: B-0012
- State: READY
- Milestone: M4, capability service and independent verifier
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-10T18:34:56Z
- Branch: `main`
- Worktree: M0 through M3 are complete. B-0011 is archived at
  `docs/agent/history/B-0011-local-sp1-plonk-proof.md`. The validated public Plonk
  benchmark is recorded at
  `docs/benchmarks/sp1-plonk/cluster-20260810T174732Z-76324.md`. M4 implementation
  has not started.

## Objective

Define and implement the first M4 slice: independent proof-capability verification
through verifier-owned registries, bounded artifact ingress, local `LightProver`, and
process isolation. Stop before asynchronous capability jobs, protected-request
signatures, gateway caching, or product UI work.

## Acceptance Criteria

- [ ] One accepted decision record fixes verifier ingress limits, child-process
      isolation, trusted key selection, registry lookups, timeout behavior, and
      public versus audit errors.
- [ ] Verification resolves the exact approved policy, snapshot descriptor, and
      program key from verifier-owned local state.
- [ ] A memory-capped and time-capped child decodes and verifies the strict ADR-0012
      artifact without access to `razield` or any proving path.
- [ ] Altered proof bytes, public values, key identity, policy status, snapshot
      status, scope, and expiry fail closed.
- [ ] The verifier completes its focused integration gate with `razield` stopped.
- [ ] Focused and affected workspace gates pass.

## Architecture Constraints

- `verifierd` never calls `razield`.
- The verifier resolves `policy_hash` through its trusted local policy registry.
- The agent never receives PolicyIR, relationships, paths, openings, salts, stored
  caveat values, or witness traces.
- Request data cannot select the authorization guest ELF or verification key.
- Raw Protobuf bytes are never commitment inputs.
- The baseline guest proves `ALLOW` only.
- No hosted prover or SP1 network client receives private witness data.
- Policy authorization and dataset relevance remain separate systems.

## File Scope

- `docs/decisions/0013-*.md` for the M4 verifier execution contract
- `crates/verifier/`
- `services/verifierd/`
- `crates/prover/` only for an approved verification-only boundary split
- Existing capability and verifier Protobuf contracts after explicit review
- Focused verifier fixtures, mutation tests, and baton records

## Out of Scope

- `razield` asynchronous capability-job implementation
- Protected-request signature implementation and gateway cache behavior
- Next.js gateway, SDK, and console work
- Real transaction data and reconciliation acceptance
- Hosted proving, SP1 network proving, CUDA, or a new proof mode
- Hypha dataset relevance attestation

## Decisions

- Inherited: D-001 through D-019 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0012.
- ADR-0012 requires M4 to decode and verify remote artifacts in a memory-capped,
  time-capped child process.
- ADR-0002 remains proposed. Its protected-request envelope needs explicit maintainer
  approval before the later request-signature slice.
- Made: None.

## Open Questions and Blockers

- The child-process protocol, memory ceiling, timeout, and crash mapping need an
  accepted M4 decision before implementation.
- The existing `raziel-verifier` crate owns authority and registry validation. Its
  boundary with ADR-0012 artifact decoding and `LightProver` needs source review.
- O-005 and O-007 remain open before gateway integration. They do not block the first
  verifier-only slice.
- No new dependency, package, CLI, runtime, or toolchain change is approved.
- Deferred follow-up: after the M7 exit gate passes and the PoC meets its definition
  of done, remind the maintainer about Hypha relevance attestation, the Obsidian
  `Current Implementation Discussion Post`, and the TEE-backed local-authorization
  experiment.

## Work Completed

- M0 established the repository, pinned toolchain, contracts, and service shells.
- M1 delivered strict SpiceDB compilation, PolicyIR semantics, and live equivalence.
- M2 delivered canonical commitments, blinded snapshots, authority signatures, and
  verifier-local registries.
- M3 delivered the bounded authorization guest, real local Plonk proof generation,
  independent `LightProver` verification, mutation rejection, and public benchmark.

## Validation Plan

- Strict artifact ingress and malformed-length tests
- Trusted registry and verification-key selection tests
- Proof, journal, policy, snapshot, scope, status, and expiry mutations
- Child timeout, memory exhaustion, crash, and output-bound tests
- Verifier completion with `razield` stopped
- SP1 feature-tree exclusion of proving and network paths where the crate boundary
  permits it
- Focused Rust tests, affected workspace tests, formatting, lint, and diff checks
- Private-data and log-output review

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0011 archive | PASS | M3 guest, local Plonk proof, independent verification, mutation gates, and benchmark evidence | 2026-08-10T18:34:56Z |

## Privacy Checks

- PASS: M3 public output contains only approved public fields and aggregate metrics.
- PASS: Raw proof logs, proof bytes, public-journal files, and witness data remain
  outside Git.
- PENDING: M4 verifier ingress and child-process logs need review before service use.
- PENDING: M4 must prove verifier independence with `razield` stopped.

## Risks and Private-Data Notes

- A remote artifact can trigger large allocations or verifier panics before
  cryptographic rejection. Bound bytes before decode and isolate the verifier.
- A caller-selected key or policy record can turn valid cryptography into invalid
  authorization. Resolve all trusted material from verifier-owned registries.
- Generic client failures must not expose policy structure, registry state, or proof
  internals. Detailed codes belong only in access-controlled audit records.

## Exact Next Action

Claim B-0012, inspect the existing verifier crate, service shell, Protobuf contracts,
and ADR-0002, then draft ADR-0013 for maintainer review before adding dependencies or
changing verifier behavior.
