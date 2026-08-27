# Engineering Baton: B-0034

- State at archive: REVIEW
- Milestone: M5, TypeScript agent signing SDK
- Owner: Codex
- Started UTC: 2026-08-26T19:36:24Z
- Archived UTC: 2026-08-26T19:49:13Z
- Branch: `main`

## Objective

Implement the TypeScript agent signing SDK for capability issuance and protected
requests, with byte-for-byte agreement against the approved Rust commitments.

## Acceptance Criteria

- [x] The SDK creates an ephemeral Ed25519 agent identity and derives the approved
      raw-key thumbprint.
- [x] The SDK builds and signs the exact ADR-0011 capability request commitment.
- [x] The SDK builds and signs the exact ADR-0002 protected-request envelope.
- [x] The SDK exposes typed signed issuance and protected-request values without
      exposing PolicyIR or private witness data.
- [x] Cross-language vectors and focused SDK tests pass.
- [x] No policy editor, transaction store, console screen, or proof semantic changes.

## Architecture Constraints

- The gateway never accepts an unproved `ALLOW` from `razield`.
- `verifierd` remains independent and never calls `razield`.
- Raw Protobuf bytes are never commitment inputs.
- The SDK never receives PolicyIR, graph data, caveat values, paths, openings, salts,
  or witness traces.
- Agent private keys remain process-local.

## File Scope

- `packages/sdk-ts/**`
- `docs/agent/CURRENT-BATON.md`

## Decisions

- B-0034 was narrowed to proof-critical SDK signing after the initial audit found
  O-005 still open before gateway integration.
- Node WebCrypto owns ephemeral Ed25519 key generation, SHA-256, randomness, signing,
  and verification. No dependency was added.
- The SDK normalizes request-context order before commitment construction and rejects
  duplicate names.
- Approved for a later gateway baton: `@connectrpc/connect` `2.1.2`,
  `@connectrpc/connect-node` `2.1.2`, and `redis` `6.2.1`.

## Work Completed

- Added canonical big-endian encoders for the ADR-0011 issuance commitment.
- Ported the ADR-0002 method and origin-form target canonicalization rules.
- Added ephemeral raw-key Ed25519 identity creation and SHA-256 thumbprints.
- Added capability-request and protected-request signing.
- Added Rust-vector, ambiguity, signature, and route-mutation checks.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/sdk-ts test` | PASS, 6 tests | Rust commitments, canonical targets, Ed25519 signatures, mutation rejection | 2026-08-26T19:48:33Z |
| `./scripts/dev pnpm --filter @raziel/sdk-ts typecheck` | PASS | SDK TypeScript | 2026-08-26T19:48:38Z |
| `./scripts/dev pnpm --filter @raziel/sdk-ts lint` | PASS | SDK source and tests | 2026-08-26T19:48:43Z |
| Direct pinned Biome formatter check | PASS | SDK source and tests | 2026-08-26T19:48:48Z |
| `./scripts/dev pnpm --filter @raziel/sdk-ts build` | PASS | SDK package output | 2026-08-26T19:48:53Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T19:49:13Z |

## Known Tooling Note

The existing package `format:check` command uses `biome format --check`, which the
pinned Biome `2.5.4` rejects before reading files. B-0034 used the formatter-only
`biome check` command directly. This baton did not expand into workspace script work.

## Handoff

O-005 must close before live gateway integration. Independent M5 dataset and policy
features can continue first.
