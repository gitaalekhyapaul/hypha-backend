# Engineering Baton Archive

- Baton ID: B-0047
- Final state: REVIEW
- Milestone: M5, visual policy builder and candidate review
- Owner: Codex
- Started UTC: 2026-08-26T22:32:34Z
- Completed UTC: 2026-08-26T22:55:51Z
- Branch: `main`

## Objective

Add a desktop visual policy workspace that lets an authenticated operator compose
the supported payment-reconciliation authorization rule families, compile a strict
policy draft through the real B-0045 route, and review its safe candidate projection.

## Acceptance Criteria

- [x] The Policy navigation item opens a functional desktop workspace.
- [x] The operator can configure direct assignment, team membership, alternative
      authorization, dual control, and a supported caveat through visual rule blocks.
- [x] The UI emits only the strict B-0033 policy draft contract and does not expose a
      free-form SpiceDB or CEL editor.
- [x] Candidate compilation uses the authenticated B-0045 route and the real pinned
      Go and Rust compiler path.
- [x] The review view shows candidate ID, `COMPILED` status, `policy_hash`, expiry,
      source schema digest, record ID, approval epoch, and public rule summary.
- [x] Loading, validation, compilation failure, success, stale-review, refresh, and
      expired-session states are usable at a 1440 by 900 desktop viewport.
- [x] Focused feature checks, console gates, live browser validation, and
      `git diff --check` pass.

## Architecture Constraints

- The browser never receives generated SpiceDB schema, compiler configuration,
  PolicyIR, diagnostics, private paths, relationships, or witness data.
- The compiler configuration and executable paths remain server-owned.
- Candidate creation does not approve, publish, register, or issue a capability.
- The UI is a bounded policy authoring form. It is not a client-side evaluator.

## Files Changed

- `.gitignore`
- `apps/demo-console/app/globals.css`
- `apps/demo-console/components/operator-portal.tsx`
- `apps/demo-console/components/policy-workspace.tsx`
- `apps/demo-console/lib/policy-builder/`
- `apps/demo-console/lib/portal-client/`
- `apps/demo-console/test/policy-builder.test.ts`
- `apps/demo-console/next.config.ts`
- `apps/demo-console/package.json`
- `docker-compose.yml`
- `scripts/prepare-m5-policy-tools`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0047-visual-policy-builder.md`

## Decisions

- The five operator-facing blocks map to existing typed rule nodes. Organization
  delegation remains available as one alternative-path leaf.
- Caveat controls select fixed templates and display their locked provenance labels.
  They never accept stored relationship values.
- Candidate identifiers are generated per compile so a changed draft cannot collide
  with an immutable candidate directory.
- The app profile builds the pinned Go compiler and Rust inspector once, then reuses
  ignored local build caches. The browser never selects executable paths.
- Detailed mobile work remains deferred. Desktop feature completeness is the current
  presentation target.
- The stale Biome `format --check` package command was replaced with Biome's current
  non-writing `format` check form.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-console test` | PASS, 42 tests | Console auth, datasets, policy draft, builder, candidate, and routes | 2026-08-26T22:51:41Z |
| `./scripts/dev pnpm --filter @raziel/demo-console typecheck` | PASS | Console TypeScript | 2026-08-26T22:54:59Z |
| `./scripts/dev pnpm --filter @raziel/demo-console lint` | PASS, 54 files | Console source and tests | 2026-08-26T22:52:16Z |
| `./scripts/dev pnpm --filter @raziel/demo-console build` | PASS, twelve routes | Console production build | 2026-08-26T22:52:25Z |
| `./scripts/dev pnpm --filter @raziel/demo-console format:check` | PASS, 55 files | Console formatting | 2026-08-26T22:53:24Z |
| First `demo-console-policy-tools` preparation | PASS, locked Go and Rust builds | Reproducible compiler and inspector runtime | 2026-08-26T22:47:31Z |
| Cached `demo-console-policy-tools` preparation | PASS, Rust build reused in 0.21 seconds | Repeated app startup | 2026-08-26T22:52:52Z |
| `./scripts/compose --profile apps config --quiet` | PASS | Local, VM, and tunnel configuration | 2026-08-26T22:52:57Z |
| Live Playwright direct, team, alternative, dual-control, and caveat compiles | PASS, five HTTP 201 responses | Real Go compiler and Rust inspector route | 2026-08-26T22:50:11Z |
| Live candidate refresh | PASS, HTTP 200 | Authenticated safe projection GET | 2026-08-26T22:50:24Z |
| Candidate response private-field inspection | PASS | Public browser response boundary | 2026-08-26T22:50:36Z |
| Browser console error check | PASS, zero errors | Desktop portal | 2026-08-26T22:50:31Z |
| Desktop screenshot inspection | PASS | 1440 by 900 policy workspace | 2026-08-26T22:53:50Z |
| `git diff --check` | PASS | Current worktree | 2026-08-26T22:53:30Z |

## Risks And Private-Data Notes

- Candidate directories contain generated schema, compiler configuration, PolicyIR,
  diagnostic output, and the private candidate manifest. They remain owner-only
  under ignored runtime storage.
- Rule summaries and public hashes reveal the intentionally reviewable policy shape.
- Compiled candidates remain untrusted. The next flow must use the isolated policy
  authority and registry writer before marking a policy active.

## Exact Next Action

Connect one explicit operator approval action to the existing isolated policy
authority and trusted registry writer, then return only a safe activation projection
to the Policy workspace.
