# Engineering Baton Archive

- Baton ID: B-0046
- Final state: REVIEW
- Milestone: M5, operator portal and synthetic dataset workspace
- Owner: Codex
- Started UTC: 2026-08-26T22:03:08Z
- Completed UTC: 2026-08-26T22:32:34Z
- Branch: `main`

## Objective

Replace the M0 scaffold with an authenticated, work-focused M5 operator portal and
connect its first visible vertical slice to the existing synthetic reconciliation
dataset workflow.

## Acceptance Criteria

- [x] An unauthenticated operator sees a functional login screen, not private
      workspace state.
- [x] An authenticated operator sees stable scenario navigation and a clear
      `SYNTHETIC` data classification.
- [x] The Dataset workspace can create, load, edit, finalize, and clone bounded
      transaction revisions through server-controlled gateway access.
- [x] Finalized revisions show checksum, schema fingerprint, row count, and
      finalization time.
- [x] Mutation requests use the B-0044 Origin and CSRF boundary before reading input.
- [x] Loading, empty, validation, success, immutable, unavailable, and
      session-expired states are usable on desktop.
- [x] Mobile behavior has basic containment. Detailed mobile polish is deferred by
      maintainer direction so M5 feature work remains the priority.
- [x] Focused UI and API tests, console and gateway checks, Compose validation,
      desktop browser inspection, and `git diff --check` pass.

## Architecture Constraints

- Transaction rows remain reconciliation data. They are not SpiceDB relationships
  and do not enter the authorization Merkle tree.
- Browser responses never include private graph data, PolicyIR, authority material,
  witness data, or local filesystem paths.
- The console authenticates operator actions. Dataset storage remains owned by the
  B-0035 gateway adapter.
- Synthetic data cannot satisfy the M6 acceptance gate.

## Files Changed

- `apps/demo-console/app/`
- `apps/demo-console/components/`
- `apps/demo-console/lib/dataset-client/`
- `apps/demo-console/lib/portal-client/`
- `apps/demo-console/test/dataset-route.test.ts`
- `apps/demo-console/next.config.ts`
- `docker-compose.yml`
- `scripts/prepare-m5-demo.mjs`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0046-operator-portal-dataset-workspace.md`

## Decisions

- The browser calls authenticated console routes. Only the server-owned console
  facade can call the configured gateway address.
- The gateway address remains private to Compose. The browser origin and development
  asset host remain deployment inputs for local, VM, and tunnel deployments.
- Finalized revisions are immutable. Editing continues through an explicit clone.
- Sign-out keeps the current portal visible on transport failure so the UI cannot
  claim that a server session ended before confirmation.
- Desktop feature completion is the M5 presentation target. Mobile receives basic
  containment only during this proof of concept.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-console test` | PASS, 36 tests | Console routes, auth, datasets, and policy preparation | 2026-08-26T22:29:14Z |
| `./scripts/dev pnpm --filter @raziel/demo-console typecheck` | PASS | Console TypeScript | 2026-08-26T22:31:12Z |
| `./scripts/dev pnpm --filter @raziel/demo-console lint` | PASS, 50 files | Console source and tests | 2026-08-26T22:31:19Z |
| `./scripts/dev pnpm --filter @raziel/demo-console build` | PASS, twelve routes | Console production build | 2026-08-26T22:32:16Z |
| `./scripts/dev pnpm --filter @raziel/demo-gateway test` | PASS, 6 tests | Dataset gateway | 2026-08-26T22:30:09Z |
| Gateway typecheck, lint, and production build | PASS | Dataset gateway | 2026-08-26T22:30:31Z |
| `./scripts/compose --profile apps config --quiet` | PASS | Local and VM Compose configuration | 2026-08-26T22:31:27Z |
| Owner secret initializer and filesystem mode check | PASS, `0700` directory and `0600` file | Local operator bootstrap | 2026-08-26T22:31:39Z |
| Playwright desktop workflow through live Compose services | PASS, login, save, finalize, metadata review, and clone | Desktop portal at 1440 by 900 | 2026-08-26T22:28:00Z |
| `git diff --check` | PASS | Current worktree | 2026-08-26T22:32:23Z |

## Risks And Private-Data Notes

- Synthetic dataset rows are intentionally browser-visible. They remain bounded and
  cannot satisfy the M6 real-data acceptance gate.
- Exact Origin remains fail-closed. VM and tunnel deployments must configure their
  public browser origin instead of changing source code.
- The portal has no client-side policy evaluator and receives no private policy or
  relationship graph material.

## Exact Next Action

Build the desktop visual policy workspace from the supported rule blocks, submit its
strict draft through the real authenticated candidate route, and display the safe
compiled candidate projection.
