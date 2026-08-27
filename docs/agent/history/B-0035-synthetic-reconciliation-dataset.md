# Engineering Baton: B-0035

- State at archive: REVIEW
- Milestone: M5, synthetic reconciliation dataset workflow
- Owner: Codex
- Started UTC: 2026-08-26T19:49:13Z
- Archived UTC: 2026-08-26T19:55:14Z
- Branch: `main`

## Objective

Implement the bounded synthetic transaction dataset store and operator API used by
the M5 reconciliation portal.

## Acceptance Criteria

- [x] The gateway validates bounded canonical synthetic transaction rows.
- [x] An operator can create, replace, inspect, finalize, and clone one dataset
      revision through typed server APIs.
- [x] Finalization is immutable and records `SYNTHETIC`, source label, checksum,
      schema fingerprint, row count, and whole-second UTC time.
- [x] Query logic supports a bounded time range, exact transaction or merchant
      reference, deterministic ordering, and limited result rows.
- [x] Files use owner-only permissions and atomic replacement.
- [x] Focused persistence, validation, finalization, clone, and query tests pass.
- [x] No policy, capability, verifier, Redis, proof, or console changes.

## Architecture Constraints

- Synthetic transaction rows remain separate from SpiceDB and proof inputs.
- Every dataset and query response is labeled `SYNTHETIC`.
- Finalization is an operator record, not an authority signature.
- The store contains no policy key, relationship graph, or witness material.

## Decisions

- Use an owner-only atomic JSON file and one serialized Node process for the PoC.
- Store `amount_minor` as a canonical signed 64-bit decimal string.
- Sort stored rows by transaction identifier and query results by time then identifier.
- Finalized revisions are immutable. Editing requires an explicit clone.

## Work Completed

- Added the bounded synthetic transaction model and row digest.
- Added draft, finalization, clone, query, checksum, and schema-fingerprint logic.
- Added a `0600` atomic file store under a `0700` owner directory.
- Added list, inspect, replace, finalize, clone, and bounded-query route handlers.
- Added invalid amount, duplicate row, sub-second timestamp, range, permission, and
  immutability checks.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-gateway test` | PASS, 6 tests | Health and synthetic dataset behavior | 2026-08-26T19:54:08Z |
| `./scripts/dev pnpm --filter @raziel/demo-gateway typecheck` | PASS | Gateway TypeScript | 2026-08-26T19:54:13Z |
| `./scripts/dev pnpm --filter @raziel/demo-gateway lint` | PASS | Gateway source and tests | 2026-08-26T19:54:18Z |
| Direct pinned Biome formatter check | PASS | Dataset source, routes, and tests | 2026-08-26T19:54:47Z |
| `./scripts/dev pnpm --filter @raziel/demo-gateway build` | PASS | Five dataset routes and gateway app | 2026-08-26T19:54:40Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T19:54:52Z |

## Risks And Private-Data Notes

- The store supports one Node writer. A multi-instance deployment needs a database.
- Operator validation errors contain field-level messages but never local file paths.
- Query range, row count, and result limits prevent an unbounded export in one call.

## Handoff

Create the bounded policy-draft security decision and owner workflow before exposing
policy artifacts in the console.
