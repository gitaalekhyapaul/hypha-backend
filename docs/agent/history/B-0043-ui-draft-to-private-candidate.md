# Engineering Baton Archive

- Baton ID: B-0043
- Final state: REVIEW
- Milestone: M5, UI draft to private policy candidate
- Owner: Codex
- Started UTC: 2026-08-26T21:22:27Z
- Completed UTC: 2026-08-26T21:43:50Z
- Branch: `main`

## Objective

Implement one server-side owner adapter that accepts a runtime-validated
`PolicyDraftV1` plus bounded review metadata, renders deterministic compiler inputs,
invokes the reviewed compiler and non-signing inspector through direct argument
arrays, and returns only the browser-safe candidate projection.

## Acceptance Criteria

- [x] Caller input is strictly validated before any filesystem or process action.
- [x] Candidate and record identifiers cannot select a path or command argument.
- [x] The adapter creates one `0700` candidate directory and fixed `0600` files
      under one configured absolute owner root.
- [x] Schema and compiler configuration come only from the deterministic M5 renderer.
- [x] Compiler and inspector run without a shell, with fixed executables, a 30-second
      deadline, and bounded output.
- [x] The generated `candidate.pb` comes from strict inspection of compiler-produced
      PolicyIR, not from browser-supplied bytes.
- [x] One canonical request digest binds exact retries. A completed retry returns the
      same projection, and changed input under the candidate ID fails closed.
- [x] The result contains only the ADR-0018 safe projection and a bounded public
      failure code.
- [x] Focused tests, console checks, and `git diff --check` pass.

## Architecture Constraints

- The browser cannot submit schema, compiler configuration, PolicyIR, paths, command
  names, authority material, relationship data, or revision tokens.
- The adapter cannot sign, register, activate, write SpiceDB, publish snapshots, or
  expose an HTTP route before operator authentication and CSRF checks exist.
- Public proof values, PolicyIR semantics, and canonical encodings did not change.

## Files Changed

- `apps/demo-console/lib/policy-candidate/`
- `apps/demo-console/test/policy-candidate.test.ts`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0043-ui-draft-to-private-candidate.md`

## Decisions

- The browser supplies the typed draft, expected approval epoch, and requested policy
  lifetime. Trusted server time fixes the exact window and 24-hour candidate expiry.
- The server derives the policy record ID from validated policy identity and the
  candidate ID.
- The adapter uses a sanitized process environment, direct argument arrays, a
  30-second deadline, and a 64 KiB combined output limit.
- Candidate state binds a canonical request digest, original timestamps, and artifact
  digests. Exact failed transitions can resume. Exact completed requests return the
  stored safe projection without rerunning tools.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-console test` | PASS, 24 tests | Pinned console tests | 2026-08-26T21:43:50Z |
| `./scripts/dev pnpm --filter @raziel/demo-console typecheck` | PASS | Pinned TypeScript | 2026-08-26T21:43:50Z |
| `./scripts/dev pnpm --filter @raziel/demo-console lint` | PASS, 19 files | Pinned Biome lint | 2026-08-26T21:43:50Z |
| `./scripts/dev pnpm --filter @raziel/demo-console build` | PASS | Pinned Next.js production build | 2026-08-26T21:43:50Z |
| Direct pinned Biome format check | PASS, 6 files | B-0043 source and tests | 2026-08-26T21:43:50Z |
| Real `preparePolicyCandidate` execution | PASS | Pinned Go compiler and Rust inspector | 2026-08-26T21:43:50Z |
| Exact real candidate retry | PASS, same projection and no replacement | Pinned owner adapter | 2026-08-26T21:43:50Z |
| Private-field projection scan | PASS, only source schema digest matched | Public result type | 2026-08-26T21:43:50Z |
| `git diff --check` | PASS | Worktree | 2026-08-26T21:43:50Z |

## Risks And Private-Data Notes

- Compiler diagnostics remain in the private candidate directory and never enter a
  thrown error or returned projection.
- The existing package `format:check` scripts use a removed Biome `--check` option.
  B-0043 used the pinned direct formatter check and did not widen scope to repair all
  four affected workspace packages.
- HTTP exposure remains closed until B-0044 supplies operator authentication, exact
  Origin checks, and session-bound CSRF validation.

## Exact Next Action

Add the single-operator session and CSRF boundary required by ADR-0018, then expose
candidate preparation through that authenticated owner route.
