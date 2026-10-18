# Current Engineering Baton

- Baton ID: B-0071
- State: READY
- Milestone: M7, VM role demonstration and two-agent acceptance
- Owner: unassigned
- Started UTC: not started
- Last updated UTC: 2026-10-18T09:05:00Z
- Branch: `main`
- Worktree: scoped B-0070 UI checkpoint; no known unrelated source changes.
- Rollback: remote annotated `v1` at `1a297cb4fea1f1a1a736126606b1a1409c323c12`.

## Objective

Deploy the operator-managed role checkpoint through the maintainer's VM pull.
Demonstrate empty/add/remove membership on fresh agent requests, with one real
Plonk success and protected query. Then complete the still-open two-agent M7
reconciliation and recording gate without changing proof semantics.

## Acceptance Criteria

- [ ] Maintainer pulls the backend feature commit and rebuilds owner tools.
- [ ] Existing VM data, keys, SDK installation, and approved guest stay intact.
- [ ] Fresh empty approved policy enters managed mode through Agents.
- [ ] Missing role yields no capability. Add role yields a real capability and query.
- [ ] Remove role yields no capability on another fresh request.
- [ ] Both agents retain distinct keys and obtain separate real capabilities.
- [ ] SDK-mediated queries, independent verification/cache reuse, and reconciliation pass.
- [ ] Recording evidence covers negative controls, receipts, source IDs, and timings.

## Architecture Constraints

- Follow ADR-0019, ADR-0023, and the existing approved guest/key identity.
- No PolicyIR, tuples, caveats, paths, or openings in agent responses or logs.
- `verifierd` never calls `razield`.
- Role edits affect fresh snapshots. Old capabilities are not immediately revoked.
- Missing membership is rejected before expensive Plonk work, not a DENY proof.
- Exact retries keep their captured revision. Fresh requests test later role changes.

## File Scope

- Current baton, dated VM evidence, and M7 runbook corrections backed by actual results.
- No new implementation scope is claimed. Record a concrete failure before repairs.
- SDK and demo-agents repositories need no change for B-0068 deployment.

## Out Of Scope

New proof semantics, new guest, live graph checks on protected queries, immediate
role revocation, real-data acceptance, second scenarios, or deleting/resetting volumes.

## Decisions Inherited

- B-0068 adds operator-only direct roles and team membership through the owner writer.
- Managed mode is permanent per opted-in policy version and needs an empty graph.
- Legacy versions retain automatic grant provisioning.
- Team membership removal affects that team's datasets across the same version.
- Public SDK remains `@gitaalekhyapaul/raziel-sdk-ts@0.1.0`. No new package publication.
- Real local Plonk jobs remain serial. No hosted proving or mock capability in the demo.

## Open Questions And Blockers

- No new approval or dependency is needed for deployment.
- VM acceptance has not run in this checkpoint. Ask the maintainer to pull first.
- Check Docker and the SSH ControlMaster before remote diagnostics. Pause and notify
  the maintainer if either is unavailable. Do not modify VM source silently.
- The old M7 proof, negative-control, screenshot, and recording requirements remain
  open. See the preserved pre-role checkpoint, not an assumed milestone completion.

## Work Completed

- [B-0070 saved-dataset selector](history/B-0070-role-editor-saved-datasets.md):
  the role dropdown reads saved datasets and clones, shows revision/status, and refreshes.
  Browser regression, full console suite, TypeScript, and scoped Biome pass.
- B-0069 was deferred for the maintainer-approved selector correction, not completed.

- [B-0068 implementation evidence](history/B-0068-operator-managed-agent-roles.md):
  operator editor/API/IPC, managed graph capture, immutable retries, local tests.
- [Preserved previous state](history/2026-10-18-pre-operator-roles-checkpoint.md):
  earlier VM repairs and all unresolved two-agent acceptance requirements.
- [ADR-0023](../decisions/0023-operator-managed-agent-roles.md) records scope and rollback.

## Validation Plan

1. Follow the M7 runbook upgrade without clearing state or rebuilding the guest.
2. Use a new Team membership policy version and `processor-demo-v1`.
3. Execute missing/add/remove controls with a fresh task/request for each attempt.
4. Confirm the successful capability reaches the independent verifier and data route.
5. Complete Agent B and the shared reconciliation, then record public-only evidence.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0068 owner-tool library | PASS, 131 tests, 4 ignored | Local pinned toolchain | 2026-10-18 |
| B-0068 real SpiceDB provisioning gate | PASS, 1 test | Empty/add/remove and immutable retries | 2026-10-18 |
| B-0068 access-service library | PASS, 34 tests, 17 ignored | Local downstream regressions | 2026-10-18 |
| B-0068 console | PASS, 673 tests, 18 skipped | Full console suite, typecheck, scoped Biome | 2026-10-18 |
| B-0068 browser flows | PASS | Controlled HTTP fixtures, not a real proof | 2026-10-18 |
| B-0070 saved-dataset selector | PASS | Clone/refresh/error/empty browser checks; 673 console tests, 18 skipped | 2026-10-18 |
| B-0071 VM role and full two-agent demonstration | SKIP | Await maintainer pull and live run | 2026-10-18 |

## Risks And Private-Data Notes

Do not interrupt admitted proof jobs. Finish or cancel them before stopping services.
Preserve private volumes, runtime settings, and agent identities. Code rollback to
`v1` does not restore graph state. Old binaries must not issue against the managed
demo version, as they do not understand its mode record.

Retain public-only timings, checksums, source commits, receipts, and dashboard
screenshots. Keep API keys, SoC credentials, raw witness material, and capability
bytes outside Git. Synthetic data and local test results do not establish the full
M7 gate or production readiness.

## Exact Next Action

Have the maintainer pull backend `main` and follow
[Operator Role Assignment Demo](../runbooks/M7-AGENT-DEMO.md#operator-role-assignment-demo).
An already deployed B-0068 stack needs only the documented console recreation for
B-0070. Earlier deployments need the full owner-tools upgrade after active proofs
finish or reach terminal cancellation. Start the negative/positive role controls
with a fresh managed policy version and fresh requests. Do not reset volumes or
rebuild the approved guest for these features.
