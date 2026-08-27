# Engineering Baton Archive

- Baton ID: B-0045
- Final state: REVIEW
- Milestone: M5, authenticated policy candidate route
- Owner: Codex
- Started UTC: 2026-08-26T21:52:40Z
- Completed UTC: 2026-08-26T22:03:08Z
- Branch: `main`

## Objective

Expose B-0043 policy candidate preparation through one B-0044 authenticated owner
route, enforce body bounds before compilation, and provide an authenticated safe
projection read for the visual policy review screen.

## Acceptance Criteria

- [x] Candidate creation requires a valid operator session, exact Origin, and the
      session-bound CSRF header before reading the request body.
- [x] The route accepts only the strict B-0043 request and enforces a 32 KiB limit.
- [x] The configured candidate root and executable paths remain server-owned.
- [x] Successful creation returns only the strict ADR-0018 candidate projection.
- [x] Candidate reads accept only a validated symbolic candidate ID and return only a
      checked `COMPILED` projection bound to private state and artifact digests.
- [x] Authentication, validation, conflict, expiry, compiler, inspector, and storage
      failures map to bounded HTTP status and error codes.
- [x] Route tests, console checks, and `git diff --check` pass.

## Architecture Constraints

- No route returns schema, compiler configuration, PolicyIR, diagnostics, private
  paths, manifest bytes, authority material, or relationship data.
- Candidate creation cannot approve, register, write SpiceDB, publish a snapshot, or
  issue a capability.
- Caller input never selects an executable or filesystem path.

## Files Changed

- `apps/demo-console/lib/policy-candidate/`
- `apps/demo-console/app/api/policies/candidates/route.ts`
- `apps/demo-console/app/api/policies/candidates/[candidateId]/route.ts`
- `apps/demo-console/test/policy-candidate-route.test.ts`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0045-authenticated-policy-candidate-route.md`

## Decisions

- Exact Origin validation intentionally precedes session and CSRF validation for
  mutations. Rejected requests never reach body parsing.
- The authenticated GET revalidates the private state, candidate binding, expiry,
  artifact digests, and strict projection before returning public data.
- Candidate lookup uses the same bounded symbolic identifier grammar as creation.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-console test` | PASS, 33 tests | Console and authenticated candidate routes | 2026-08-26T22:02:21Z |
| `./scripts/dev pnpm --filter @raziel/demo-console typecheck` | PASS | Console TypeScript | 2026-08-26T22:02:29Z |
| `./scripts/dev pnpm --filter @raziel/demo-console lint` | PASS, 33 files | Console source and tests | 2026-08-26T22:02:35Z |
| `./scripts/dev pnpm --filter @raziel/demo-console build` | PASS, seven routes | Console production build | 2026-08-26T22:02:42Z |
| Direct pinned Biome format check | PASS, 8 files | B-0045 source and tests | 2026-08-26T22:02:51Z |
| Real authenticated candidate POST in pinned Linux toolchain | PASS, `COMPILED` | Go compiler, Rust inspector, and route boundary | 2026-08-26T22:01:59Z |
| Public projection private-field assertions | PASS | Route response | 2026-08-26T22:02:21Z |
| Host direct real-route attempt | NOT AVAILABLE, pinned binaries are Linux executables | macOS host | 2026-08-26T22:00:58Z |
| `git diff --check` | PASS | Current worktree | 2026-08-26T22:03:00Z |

## Risks And Private-Data Notes

- The route exposes only the reviewed projection. Compiler diagnostics and private
  candidate artifacts remain in the owner-only candidate directory.
- HTTP status codes reveal broad failure classes but no policy path, relationship,
  caveat value, or compiler diagnostic.

## Exact Next Action

Build the authenticated operator portal shell and connect the first visible
synthetic dataset workflow to the existing B-0035 gateway routes.
