# Current Engineering Baton

- Baton ID: B-0067
- State: READY
- Milestone: M7, two-agent reconciliation dashboards
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-09-27T10:44:00Z
- Branch: `main`
- Worktree: B-0066 checkpoint archived. Its backend commit contains this baton.
- Execution: pause after B-0066 commits and pushes. Wait for explicit resume.

## Objective

Build Agent A and Agent B dashboards around the packaged SDK, synthetic processor
and bank ledgers, bounded LLM tools, and a shared reconciliation message channel.
Validate the provisional M6 VM path before claiming a full proof-backed demo.

## Acceptance Criteria

- [ ] Both agents retain separate identities, tokens, task state, and dashboards.
- [ ] Each agent requests approval, signs its exact proposal, and receives a capability.
- [ ] SDK-only protected queries show independent verification and cache reuse.
- [ ] Agent A requests bounded reconciliation evidence from Agent B through a simple channel.
- [ ] The operator and agent dashboards expose useful task and request states.
- [ ] Seeded and operator-created records remain clearly labeled synthetic.
- [ ] OpenAI-compatible LLM configuration stays server-side with bounded SDK tools.
- [ ] A real VM proof validates the connected M6 path, not a controlled verifier.

## Architecture Constraints

- Inherit ADR-0019, ADR-0020, and ADR-0021. Do not change proof semantics.
- One proof runs at a time. Final signing follows capacity reservation and snapshot preparation.
- Policy publication is independent. Exact approved versions stay pinned.
- Agents receive no PolicyIR, grant recipe, tuples, stored caveats, salts, openings, or witness.
- Authentication and inbox approval do not authorize data without gateway proof checks.
- The independent verifier never calls the issuer. No production mock-proof flag exists.

## File Scope

- Reinspect all three repositories after the maintainer changes the primary project folder.
- Agent repo: dashboards, identity persistence, task state, SDK tools, LLM adapter, message channel.
- Backend: synthetic seeding, narrow demo orchestration, and VM evidence.
- SDK: only fixes exposed by connected consumption, not a second implementation.
- No B-0067 source has been edited. Claim exact paths after explicit resume.

## Out Of Scope

- Real-data acceptance, another domain, parallel proofs, new guest semantics, identity infrastructure.
- Public npm publication without its separate release authorization.
- Automatic VM pulls, deployment, or private data migration during the pause.

## Decisions

- Finish B-0066, commit/push both changed repos, report progress, then pause.
- B-0066 closes as an implementation checkpoint. The maintainer deferred npm publication.
  The packaged candidate suffices for provisional VM testing.
- ADR-0021 approves dedicated PostgreSQL 17.10 for live M6 SpiceDB.
- Use fresh `raziel-m6-live` state. Do not implicitly migrate M5 or the partial preview.
- The maintainer will select the parent project directory after B-0066. Recheck writable roots then.
- Ask for Docker or SSH ControlMaster availability if absent. Do not modify VM source directly.

## Open Questions And Blockers

- No feature decision blocks B-0067, but execution is paused by the maintainer.
- Public npm authentication/publication is deferred, not needed for the candidate install.
- The complete VM release build and live M6 proof validation are unrun.
- Project source-folder listings do not change this task's supplied filesystem permissions.

## Work Completed

M0-M5 remain complete. B-0064 delivered registration, token/key authentication, and
SDK transport. B-0065 delivered the inbox, independent policy versions, approval,
and real snapshot preparation. B-0066 delivered serial dispatch, exact signatures,
durable proof handoff/results, SDK polling, and protected-query reuse.

The live Compose overlay and VM runbook are implemented. PostgreSQL migration and
synthetic grant persistence through restart pass. The existing preview is unchanged.
No new VM proof, npm publication, or agent-dashboard implementation ran.
M6 remains in progress until its live proof and release follow-ups are resolved.

See [B-0066 archive](history/B-0066-serial-dispatch-and-sdk-delivery.md).

## Validation Plan

On resume, use [M6 Live VM](../runbooks/M6-LIVE-VM.md) for the provisional deployment.
Keep checks focused on features. Distinguish real from controlled proof evidence.
Build desktop agent flows before extended layout or test-infrastructure work.

## Validation Evidence

| Check | Result | Scope |
| --- | --- | --- |
| B-0066 Rust | PASS | 79 focused tests, scoped Clippy and feature checks; controlled proof delivery |
| SDK | PASS | 249 tests, build/types/lint/format, packaged candidate |
| Console | PASS | 457 tests, 18 opt-in checks skipped in final regression |
| Gateway | PASS | 27 tests, exact dataset mapping and cold/cache checks |
| Packaged consumer | PASS | SDK signatures, facade/gateway, controlled verifier, reuse and revocation |
| Static/deployment | PASS | Types, generation, format, Compose, shell syntax, migration and restart persistence |
| New VM proof and npm release | SKIP | Explicit follow-ups, not claimed complete |

## Risks And Private-Data Notes

- Preserve owner-only keys, snapshots, grants, and request comparison files.
- Live M6 uses PostgreSQL. The old partial preview still uses memory-backed SpiceDB.
- Use the packaged candidate, not sibling source imports or private generated files.
- SDK commit: `e7f1cd6b354f236f0323d9d19aa4a431c5dfa470`.
- Candidate SHA-256: `556abe87978429a1542bfbbe8d81edd48a8be92ceeb07b1731eb18e8233ce572`.
- Agent repo unchanged. SDK `.DS_Store` remains untracked and excluded.

## Exact Next Action

Wait for explicit maintainer resume after the B-0066 commit/push handoff. Do not
start B-0067 or deploy to the VM during the pause. Recheck the parent workspace
permissions first, then follow the provisional M6 VM runbook.
