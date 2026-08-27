# Current Engineering Baton

- Baton ID: B-0037
- State: REVIEW
- Milestone: M5, typed policy draft and deterministic rendering
- Owner: Codex
- Started UTC: 2026-08-26T20:05:18Z
- Last updated UTC: 2026-08-26T20:16:10Z
- Branch: `main`
- Worktree: Approved M5 docs plus completed B-0034 and B-0035 features are uncommitted.

## Objective

Implement the reusable runtime-validated `PolicyDraftV1` model and deterministic
SpiceDB schema and compiler-configuration renderer for the visual policy builder.

## Acceptance Criteria

- [x] Runtime validation rejects unknown fields, invalid identifiers, unsupported
      rule nodes, invalid caveat inputs, and values outside fixed bounds.
- [x] The model supports direct relation, subject-set relation, one-arrow delegation,
      `ANY`, `ALL`, and one named caveat from the approved catalog.
- [x] Rendering produces stable `.zed` schema and compiler configuration bytes.
- [x] Compiler limits and caveat provenance remain fixed server-side.
- [x] Tests cover at least direct, team, union, intersection, and caveated shapes plus
      the rejected ADR-0018 constructs.
- [x] Console tests, typecheck, lint, build, formatting, and `git diff --check` pass.

## Architecture Constraints

- The browser model contains no PolicyIR, raw `.zed`, compiler diagnostics, keys,
  relationships, stored caveat values, or ZedTokens.
- The renderer stays inside the existing SpiceDB and CEL compiler subset.
- The proof guest, PolicyIR nodes, canonical hashes, capability, and snapshot formats
  do not change.
- The server remains the authoritative draft validator.

## File Scope

- `apps/demo-console/lib/policy-draft/`
- `apps/demo-console/test/policy-draft.test.ts`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0037-typed-policy-draft-and-rendering.md`

## Out Of Scope

- Compiler process invocation or PolicyIR inspection
- Policy signatures, registry publication, SpiceDB writes, or snapshot publication
- Relationship submission or graph readback
- Console screens and visual styling
- Gateway, proof queue, and capability transport
- New dependencies

## Decisions

- Inherited: ADR-0018 defines the typed subset and fixed server-side compiler profile.
- Inherited: M5 must support at least four visual policy shapes.
- Local: keep the draft, validator, catalog, and renderer in separate modules.

## Open Questions And Blockers

- None.

## Work Completed

- B-0036 archived after maintainer approval of ADR-0018.
- Added separate draft types, fixed caveat catalog, strict validator, deterministic
  renderer, and public exports under `apps/demo-console/lib/policy-draft`.
- Added five accepted visual policy shapes and fixed templates for task, purpose,
  scope, amount, override, and approval checks.
- Added focused tests for accepted forms, deterministic normalization, unknown fields,
  wildcard input, exclusion, recursion, duplicate branches, arrow count, arbitrary
  caveats, identifiers, versions, values, limits, and rule depth.

## Validation Plan

- Run focused Vitest cases for accepted and rejected drafts.
- Run console typecheck, lint, build, and formatting checks through the pinned tooling.
- Run `git diff --check` and inspect the client-visible model for private fields.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-console test` | PASS, 18 tests | Console and policy draft | 2026-08-26T20:15:39Z |
| `./scripts/dev pnpm --filter @raziel/demo-console typecheck` | PASS | Console TypeScript | 2026-08-26T20:15:52Z |
| `./scripts/dev pnpm --filter @raziel/demo-console lint` | PASS, 13 files | Console lint | 2026-08-26T20:16:00Z |
| `./scripts/dev pnpm --filter @raziel/demo-console build` | PASS | Console production build | 2026-08-26T20:13:51Z |
| Biome formatter-only check | PASS, 6 files | Policy draft source and tests | 2026-08-26T20:15:47Z |
| Pinned Go compiler conformance | PASS | One generated policy covering direct, subject set, arrow, `ANY`, `ALL`, and caveat | 2026-08-26T20:14:45Z |
| Client-visible private-field scan | PASS | Policy draft model and tests | 2026-08-26T20:16:10Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T20:16:10Z |

## Risks And Private-Data Notes

- A generic schema editor would exceed ADR-0018 and expose unsupported policy surface.
- Caveat provenance must come from the server catalog, not request data.
- Stable output must not rely on object insertion order from unvalidated JSON.

## Exact Next Action

Archive B-0037 and open B-0038 for non-signing policy inspection and safe projection.
