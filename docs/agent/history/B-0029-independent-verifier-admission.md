# Engineering Baton Archive

- Baton ID: B-0029
- Final state: completed and archived
- Milestone: M4, independent verifier admission
- Owner: Codex
- Started UTC: 2026-08-24T11:56:43Z
- Completed UTC: 2026-08-24T12:43:48Z
- Branch: `main`
- Base commit: `8670c9a`

## Objective

Close the protected-request signature decision, then implement the strict public
admission boundary that prepares one capability for independent verification without
calling `razield` or reading private graph data.

## Acceptance Criteria

- [x] ADR-0002 is explicitly accepted before its signature rules enter production
      code.
- [x] Raw `VerifyCapabilityRequest` ingress applies the 2 MiB ceiling and rejects
      malformed, unknown, duplicate, explicit-default, and noncanonical fields.
- [x] Admission validates raw-key thumbprint binding and the exact ADR-0002 Ed25519
      protected-request signature.
- [x] Trusted time, capability expiry, request timestamp, nonce, method, target, and
      body-hash checks fail closed with typed private causes and generic public codes.
- [x] Verifier-owned local state resolves the exact policy, snapshot, authority
      signatures, and program-key identity without a `razield` endpoint.
- [x] Admission reconstructs the complete ADR-0011 expected public-value bytes and
      emits only a bounded public worker request.
- [x] Focused mutation, registry, feature-tree, formatting, lint, type, workspace,
      privacy, and diff gates pass without running SP1.

## Architecture Constraints

- `verifierd` never calls `razield`.
- The capability carries `policy_hash`, not PolicyIR.
- No relationship, caveat value, salt, opening, path, witness, or private envelope
  enters verifier admission.
- Request data cannot select proof mode, setup, verification key, registry path,
  worker endpoint, or resource profile.
- ADR-0011 public proof values did not change.
- The isolated verifier worker and live service binding remain later bounded work.
- No package, CLI, runtime, or system dependency was added. Existing pinned workspace
  packages became direct `verifierd` dependencies.

## Decisions

- Maintainer accepted ADR-0002 on 2026-08-24.
- The signature covers one domain-separated canonical message, not Protobuf or JSON.
- The gateway owns atomic nonce consumption. `verifierd` validates the signed nonce
  but does not create a second replay authority.
- Version 1 strict ingress uses a frozen descriptor and exact re-encoding check.
- A public capability-scope model reconstructs the existing scope hash without
  inventing private issuance fields. Hash bytes remain unchanged.
- Corrected stale four-CPU and 29-minute text in ADR-0013 to the approved eight-CPU,
  15-minute 30-second profile from B-0028.

## Work Completed

- Added strict method and origin-form target canonicalization with deterministic
  signature message construction.
- Added a language-neutral protected-request vector and Rust consumer.
- Added strict raw-Protobuf decoding against a frozen version 1 descriptor and
  checksum.
- Added pure verifier admission for capability claims, SP1 artifact bounds, raw-key
  identity, Ed25519 request signatures, trusted time, and local registry state.
- Reconstructed the exact expected ADR-0011 public journal and emitted a narrow
  worker request with proof bytes and the trusted paired program key.
- Kept admitted claims explicitly unverified until the later worker succeeds.
- Applied the registry time floor to freshness checks so host clock rollback cannot
  revive an old signed request.
- Updated README, architecture, specification, ADR-0002, and ADR-0013 current state.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| `cargo test -p raziel-canonical -p raziel-authorization-core -p verifierd` | PASS | Canonical vector, scope hash, strict ingress, admission, mutation, and time tests |
| `cargo test -p raziel-verifier --test registry` | PASS, 10 tests | Verifier-local policy and snapshot trust |
| `./scripts/check-sp1-features` | PASS | Verification-only dependency graph and RPC exclusion |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust, and Go checks |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy, Go vet, and Buf |
| `./scripts/dev pnpm test` | PASS | Default TypeScript, Rust, Go, and documentation tests |
| `./scripts/dev pnpm format:check` | PASS | Repository formatting |
| Verifier private-boundary scan | PASS | No private graph or witness types in `verifierd` |
| Current-state profile scan | PASS | ADR-0013 matches the approved eight-CPU profile |
| `git diff --check` | PASS | Patch hygiene |
| Real SP1 proof | NOT RUN | No proof required for pure admission |

The default workspace suite ignored real SP1 proof tests as designed. The existing
Rust toolchain warning about future incompatibility in `proc-macro-error2 2.0.1`
remains unchanged.

## Privacy Checks

- PASS: no agent response contains PolicyIR, relationship tuples, paths, or Merkle
  openings.
- PASS: no standard log contains caveat values, salts, witness data, or proof bytes.
- PASS: admitted capability claims contain approved public fields only and remain
  candidates until proof verification succeeds.
- PASS: `verifierd` resolves policy and snapshot state from its own `LocalRegistry`.
- PASS: verifier admission completes without a prover connection or `razield` client.
- PASS: real transaction data remains outside Git and generated output.
- NOT APPLICABLE: isolated proof verification does not run in this baton.

## Remaining Risks And Follow-Up

- The generated Tonic server still uses its standard Prost codec. B-0030 must bind the
  strict raw decoder at the live service edge before the service can claim strict
  ingress.
- The worker must verify the proof before candidate claims become verified claims.
- The gateway nonce store and matching TypeScript vector consumer remain M5 work.
- The provisional 2 GiB verifier cap still needs a retained real-proof measurement.
- One approved synthetic Plonk rerun and public artifact retention remain a separate
  maintainer gate.

## Exact Next Action

Claim B-0030 for the isolated verifier worker and live service boundary. Do not run or
retain another SP1 proof without explicit maintainer approval.
