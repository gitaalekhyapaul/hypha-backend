# Current Engineering Baton

- Baton ID: B-0078
- State: REVIEW
- Milestone: Selected Raziel identity across all three consoles
- Owner: Codex coordinator, with bounded frontend workers
- Started UTC: 2026-11-03T02:49:59Z
- Last updated UTC: 2026-11-03T03:28:00Z
- Branch: `main`
- Observed worktree: backend clean at `1019c67`; demo-agents clean before this task.
- Rollback: Existing remote `v1` is unchanged. No commit, push or deployment is part of this task.

## Objective

Apply the selected Chamber Lantern brand to the operator and shared Agent A/B
consoles without changing behavior. Prepare a separate 20-minute evaluator
presentation from Obsidian's final-presentation rules and the finalized report.

## Acceptance Criteria

- [x] Operator login, loading shell and console use the selected logo and typography.
- [x] Agent A/B use the same identity and retain clear role labels.
- [x] Local fonts and assets work offline without new runtime dependencies.
- [x] Existing desktop commands and workflows remain intact in focused checks.
- [x] Relevant type/lint checks, focused tests and browser checks record exact outcomes.
- [x] Obsidian presentation rules are mapped to the planned deck.
- [x] Timed evaluator slides, editable output and review copy use source-backed claims.
- [x] Handoff records changed files and deployment boundaries.

## Architecture Constraints

All existing proof, privacy, signature, registry and SDK invariants remain fixed.
No policy, graph, lifecycle or proof-semantic changes. No private runtime material
enters assets or slides. No VM edits, proof submissions, active-job restarts,
commits or pushes.

## File Scope

- Operator global CSS, metadata/icon assets, login/loading/shell brand components.
- `apps/demo-console/public/brand/` and focused brand tests.
- Sibling `raziel-demo-agents/public/`, selected assets and focused tests.
- Bounded agent static-asset delivery changes only if required for branding.
- This baton and its archive.
- Presentation deliverables outside application repos in the Capstone folder.

## Out Of Scope

Proof/backend redesign, dependencies, SDK releases, VM deployment and B-0077
diagnostics. Branding does not close the pending live M7 gate.

## Decisions

- Final Chamber Lantern `design.md` and assets under `Capstone/brand/final/` are authoritative.
- Every subagent uses `gpt-6.1-sol`, `xhigh`, per the current user request.
- B-0077 is preserved verbatim in `history/B-0077-deferred-for-brand-rollout.md`.
  It is deferred, not completed.
- Old baton text described skill changes as uncommitted. Current Git shows that
  checkpoint committed at `1019c67`. No diagnostic implementation is inferred.
- User approved this local branding and presentation task on 3 November 2026.

## Open Questions And Blockers

None for local implementation. Docker was already available. No SSH or VM
connection was needed. Local deliverables are ready for the user's review.

## Work Completed

Read instructions, baton/protocol, architecture and specification. Confirmed clean
starting worktrees and the final selected brand. Archived the prior diagnostic state.

Operator and shared Agent A/B branding is implemented with unchanged approved SVG
assets, local fonts and licenses. The agent HTTP server adds only ten named public
asset entries. Authenticated API handling and workflow code are unchanged. Parent
review raised the agent header logo to 164px, above the 160px brand minimum.

Obsidian's final instructions specify a 20-minute talk including the demonstration,
followed by five minutes of Q&A. The deck follows that split and the reviewed report.

The presentation contains 20 main slides, a three-minute recorded synthetic demo
inside the 20-minute schedule, and four separate backup slides. Editable diagrams,
tables, a native chart and source-linked presenter notes accompany the PPTX. The
PDF review copy preserves the branded type. Historical report screenshots remain
unchanged. Today's branded fixtures are stored separately and are not proof evidence.

All four specialist workers used GPT-6.1 Sol with xHigh reasoning. Parent review
covered the actual app diffs, desktop screenshots, every slide render and the final
PDF/PPTX package counts. The original report and SDK repository remain unchanged.

## Validation Plan

Run existing frontend type/lint checks and focused tests. Inspect desktop login and
shell states with local fixtures where live services are unnecessary. Render the
deck and verify content, timing and factual sources.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Initial Git state | PASS, clean | Backend and demo-agents | 2026-11-03 |
| Operator focused tests | PASS, 212 tests across seven files | Brand, auth, transport, status, request/policy/role clients | 2026-11-03 |
| Operator lint/format/typecheck | PASS | Existing app commands, no installs | 2026-11-03 |
| Operator production build | PASS | Existing pinned Docker image, network disabled | 2026-11-03 |
| Agent full tests | PASS, 18 tests, zero skips | Shared Agent A/B server and workflows | 2026-11-03 |
| Agent type/format checks | PASS | Existing pinned Docker image | 2026-11-03 |
| Agent brand regression rerun | PASS, three tests | Exact assets, logo size and static access boundaries | 2026-11-03 |
| Desktop browser checks | PASS | 1600px and 1280px fixtures, zero page errors/broken images/page-width overflow | 2026-11-03 |
| Obsidian instruction lookup | PASS | Exact current note, 20-minute talk/demo and five-minute Q&A | 2026-11-03 |
| Presentation schedule | PASS | 20 main slides, 1200 seconds, 180-second included demo, four backups | 2026-11-03 |
| Presentation structural checks | PASS | 24 slides and 24 note parts, native tables/chart, font policy, re-import | 2026-11-03 |
| Presentation visual review | PASS | All 24 slides and changed-slide wording pass | 2026-11-03 |
| PDF review copy | PASS | 24 widescreen pages, embedded page images match the inspected renders | 2026-11-03 |
| Final diff checks | PASS | Backend and demo-agents, SDK remains clean | 2026-11-03 |

## Deliverables And Changed Files

- Backend: `apps/demo-console/app/{globals.css,access-requests.css,layout.tsx,icon.svg}`,
  `components/{login-view.tsx,operator-portal.tsx}`, `public/brand/`, and
  `test/brand.test.tsx`. The generated `next-env.d.ts` returned to its tracked value
  after the production build and is not part of the change.
- Demo-agents: `public/{index.html,style.css}`, `public/brand/`, `src/server.ts`
  (named static assets only), and `test/brand-assets.test.ts`.
- Presentation: `Capstone/Final-Presentation/` contains the final PPTX, PDF,
  timed speaker notes, rehearsal/demo run sheet, sources/guidelines, renders and
  reproducible authoring source. `support/branding-validation/` labels UI fixtures.
- No dependency, lockfile, SDK release, environment secret, VM file or proof job
  changed. No commit or push occurred. Existing `v1` remains unchanged.
- The isolated local operator preview is at `http://127.0.0.1:3310`, container
  `raziel-brand-console-b0078`. It is a visual preview, not a provisioned live stack.
  Temporary synthetic agent fixture servers on 3311/3312 were stopped after capture.

## Risks And Private-Data Notes

Preserve authentication and origin rules. Fixture checks do not establish live
proof acceptance. Distinguish measured results from broader Hypha research aims.

## Exact Next Action

Archive this completed local checkpoint. Present the assets for user review and
await a requested commit or deployment step. Do not resume B-0077 diagnostics or
restart any VM service under this branding scope.
