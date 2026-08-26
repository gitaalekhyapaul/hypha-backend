# Current Engineering Baton

- Baton ID: B-0033
- State: READY
- Milestone: M5, TypeScript protected-request signing
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-26T09:35:00Z
- Branch: `main`
- Worktree: Clean after the M4 closure commit. No M5 implementation has started.

## Objective

Implement the TypeScript SDK boundary that canonicalizes and signs one ADR-0002
protected request for the M5 gateway.

## Acceptance Criteria

- [ ] The SDK canonicalizes the fixed HTTP method and origin-form request-target
      subset from ADR-0002 and rejects every unsupported spelling.
- [ ] The SDK hashes the exact request-body bytes and frames every signed field with
      the approved domain and fixed lengths.
- [ ] The SDK creates a 32-byte request nonce, accepts whole-second UTC timestamps,
      and signs the canonical bytes with an Ed25519 agent key.
- [ ] One deterministic TypeScript vector matches the existing Rust canonical bytes
      and signature result.
- [ ] Focused tests reject altered method, target, body, nonce, timestamp, and key
      inputs without exposing private authorization data.
- [ ] SDK formatting, lint, typecheck, build, and focused tests pass.

## Architecture Constraints

- ADR-0002 defines the exact signed bytes, target canonicalization, time bounds, and
  gateway-owned replay authority. This baton cannot change those rules.
- TypeScript must not define a second cryptographic format or hash raw Protobuf bytes.
- The SDK never receives PolicyIR, relationships, satisfying paths, Merkle openings,
  salts, or stored caveat values.
- Use the pinned Node runtime and built-in cryptography. Ask for approval before
  adding any package.

## File Scope

- `packages/sdk-ts/src/`
- `packages/sdk-ts/test/`
- Existing cross-language canonical vectors needed by the protected-request boundary
- `docs/agent/CURRENT-BATON.md`

## Out Of Scope

- Gateway verifier transport and protected payment routes
- Redis nonce storage, capability cache, and audit receipts
- Synthetic transaction storage and console screens
- M6 real-data ingestion and reconciliation
- Public proof values, PolicyIR, snapshot, capability, or verifier semantics

## Decisions

- Inherited: accepted ADR-0002 fixes the protected-request signature input,
  canonical method and target, freshness bounds, and gateway-owned nonce store.
- Inherited: accepted ADR-0014 fixes raw-key Ed25519 agent identity.
- Made: None.

## Open Questions And Blockers

- None. No dependency approval is needed for the planned Node built-in cryptography
  implementation.

## Work Completed

- M4 is complete and archived under B-0032.
- `packages/sdk-ts` remains a scaffold with no authorization behavior.

## Validation Plan

- Run the focused SDK tests and cross-language canonical vector test.
- Run SDK formatting, lint, typecheck, and build checks.
- Run `git diff --check` and scan client-visible outputs for private policy or witness
  fields.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Not run | NOT AVAILABLE | B-0033 has not started | 2026-08-26T09:35:00Z |

## Risks And Private-Data Notes

- Target canonicalization must match the gateway reconstruction rules exactly.
- A later gateway baton must consume `(capability_id, request_nonce)` atomically
  before verification and must not release it after a service failure.
- This SDK feature carries only the public capability identifier, request metadata,
  request body hash, nonce, timestamp, public key, and signature.

## Exact Next Action

Claim B-0033, inspect the Rust ADR-0002 canonical implementation and vectors, then
add the matching TypeScript SDK signing boundary and focused tests.
