# Archived Engineering Baton

- Baton ID: B-0052
- State: REVIEW
- Milestone: M5, verified capability reuse
- Owner: Codex
- Started UTC: 2026-08-30T06:36:30Z
- Last updated UTC: 2026-08-30T07:04:52Z
- Branch: `main`
- Worktree: B-0051 is committed locally as `1afecc9`. UF-09 implementation and its
  focused validation are complete and uncommitted.

## Objective

Reuse one unexpired proof capability for a second in-scope protected query. The
gateway must use a bounded Redis verification cache, retain every per-request check,
skip only repeat SP1 verification, and record a new public request receipt.

## Acceptance Criteria

- [x] A cold request verifies through `verifierd` and caches only its approved public
      projection.
- [x] The cache key binds both capability ID and proof digest, and the entry expires
      no later than the capability.
- [x] A second request with the same capability and a fresh nonce uses the cache
      without calling `verifierd`.
- [x] Cache hits still validate key thumbprint, Ed25519 signature, freshness, nonce,
      canonical target, action, resource type, and resource scope.
- [x] A changed proof package cannot reuse a cached verification.
- [x] Cold and cached requests each write a distinct public protected-request receipt,
      and the response identifies the verification path.
- [x] Focused SDK and gateway checks pass.

## Architecture Constraints

- `verifierd` never calls `razield`, and the gateway never calls `razield` during
  protected queries.
- A cache hit skips proof verification only. It cannot skip signature, time, replay,
  key-binding, target, or scope checks.
- Cache entries contain public verified claims, policy record ID, and proof digest.
  They contain no proof bytes, PolicyIR, relationships, openings, caveat values, or
  witness material.
- TypeScript uses the canonical protected-request message from the existing SDK.
- Redis remains the shared cache and replay store. No new dependency is added.

## File Scope

- `apps/demo-gateway/src/protected-request/`
- `apps/demo-gateway/test/protected-query.test.ts`
- `apps/demo-console/app/api/reconciliation/query/contract.ts`
- `apps/demo-console/components/reconciliation-workspace.tsx`
- `apps/demo-console/lib/portal-client/reconciliation.ts`
- `apps/demo-console/test/reconciliation-route.test.ts`
- `packages/sdk-ts/src/`
- `packages/sdk-ts/test/index.test.ts`
- `tools/raziel-admin/src/verifier_client.rs`
- `docker-compose.yml`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0052-verified-capability-reuse.md`

## Out Of Scope

- User-controlled denial and replay lab controls
- Receipt browsing, filtering, and export UI
- Real transaction data
- New proof fields, guest semantics, capability format, or policy semantics
- Mobile layout polish

## Decisions

- Inherited: ADR-0002 requires one gateway-owned atomic nonce store for cold and
  cached requests.
- Inherited: FR-084 keys cached verification by capability ID and proof digest until
  expiry.
- Local implementation choice: the existing Rust verifier bridge will inspect the
  canonical capability and return its capability ID plus proof digest without a
  network call.
- Local implementation choice: the cache response marker is gateway execution
  metadata. It is not a proof claim or authorization input.

## Open Questions And Blockers

- None. Docker and the SSH ControlMaster are available. Their loss is an external
  prerequisite failure that must be reported before related work continues.
- The public audit Protobuf has no cache-disposition field. UF-09 will keep each
  protected-request receipt schema-conformant and expose cold or cache status in the
  protected-query response. UF-12 can propose a versioned receipt field if exported
  cache events need one.

## Work Completed

- Reconciled UF-09 against ADR-0002, FR-084, FR-088, and the existing cold query path.
- Confirmed that the Rust inspection command decodes canonical capability Protobuf
  locally and does not contact `verifierd`.
- Extended that inspection projection with the SHA-256 proof digest. Empty proof
  artifacts fail before cache lookup.
- Added SDK verification for the existing canonical Ed25519 protected-request
  message. The gateway uses it only on cache hits.
- Added a Redis verification cache keyed by capability ID and proof digest. Each
  entry contains at most 4,096 bytes and lives for at most 24 hours or the shorter
  remaining capability lifetime.
- Added cold and cached query paths. Both claim a fresh nonce, validate time and
  scope, query bounded rows, and write a new protected-request receipt.
- Added the `VERIFIER` or `CACHE` response marker and the visible `CACHE HIT` result
  state in Reconciliation.
- Updated the runbook with the second-click capability reuse flow.

## Validation Plan

- Add focused Rust inspection tests for the proof digest projection.
- Add SDK signature-verification tests using the frozen canonical message builder.
- Add gateway cold, cache-hit, altered-proof, expiry, replay, and infrastructure tests.
- Run focused Rust, SDK, and gateway format, type, lint, and test commands in the
  pinned Docker toolchain.
- Run `git diff --check` and inspect the public cache projection for private fields.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Source reconciliation against ADR-0002 and FR-084/FR-088 | PASS | UF-09 design boundary | 2026-08-30T06:36:30Z |
| `cargo test --locked -p raziel-admin verifier_client` | PASS, 10 tests | Canonical capability inspection and proof digest | 2026-08-30T06:59:05Z |
| `cargo fmt --all -- --check` | PASS | Rust formatting | 2026-08-30T07:02:00Z |
| `cargo clippy --locked -p raziel-admin --all-targets -- -D warnings` | PASS | Rust inspection static checks | 2026-08-30T07:02:57Z |
| SDK build, test, lint, and typecheck | PASS, 6 tests | Canonical cached-request signature verification | 2026-08-30T07:02:00Z |
| Demo-gateway test, lint, and typecheck | PASS, 18 tests | Cold verification, cache hit, altered proof, key, signature, replay, receipt | 2026-08-30T07:01:17Z |
| Demo-console test, lint, and typecheck | PASS, 55 tests | Visible verification path and existing portal flow | 2026-08-30T07:01:55Z |
| Demo-gateway and demo-console production builds | PASS | Next.js runtime SDK import and route builds | 2026-08-30T07:03:50Z |
| Live Compose `config --quiet` | PASS | M5 deployment graph | 2026-08-30T07:04:00Z |
| `git diff --check` | PASS | UF-09 worktree | 2026-08-30T07:04:52Z |

## Risks And Private-Data Notes

- A key built from only capability ID would let a changed proof package borrow a
  valid cache entry. The key must include the SHA-256 proof digest.
- Cache parsing must reject unknown fields and malformed values. A Redis value is
  stateful input, not trusted code state.
- The agent public key, proof digest, and verified claims are approved public data.
  Raw proof bytes remain outside Redis.

## Exact Next Action

Archive B-0052, commit UF-09 locally, and open B-0053 for the controlled out-of-scope
denial flow. Do not push until the maintainer approves a VM handoff.
