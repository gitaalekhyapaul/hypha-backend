# Current Engineering Baton

- Baton ID: B-0067
- State: ACTIVE
- Milestone: M7, two-agent reconciliation dashboards
- Owner: Codex
- Started UTC: 2026-10-01T06:38:26Z
- Last updated UTC: 2026-10-01T11:59:49Z
- Branch: `main`
- Worktree: B-0066 backend baseline is `c5efbfb`. The B-0067 agent checkpoint is
  committed and pushed at `54aa809`. This backend checkpoint contains the seed
  feature, runbook, and current state. Unrelated outputs and generated `next-env.d.ts`
  remain excluded. SDK retains only the known untracked `.DS_Store`.
- Execution: resumed on 2026-10-01 after an interruption. Docker 29.8.0 and
  VM ControlMaster PID 4117 pass fresh checks. No remote source was changed.

## Objective

Build Agent A and Agent B dashboards around the packaged SDK, synthetic processor
and bank ledgers, bounded LLM tools, and a shared reconciliation message channel.
Validate the provisional M6 VM path before claiming a full proof-backed demo.

## Acceptance Criteria

- [x] Both agents retain separate identities, tokens, task state, and dashboards. Local checks and Docker startup pass.
- [ ] Each agent requests approval, signs its exact proposal, and receives a capability.
- [ ] SDK-only protected queries show independent verification and cache reuse.
- [x] Agent A requests bounded reconciliation evidence from Agent B through a simple channel. Authenticated role, payload, retry, and integer-matching checks pass locally.
- [ ] The operator and agent dashboards expose useful task and request states.
- [x] Seeded and operator-created records remain clearly labeled synthetic. Two editable draft ledgers are implemented.
- [x] OpenAI-compatible LLM configuration stays server-side with bounded SDK tools. Bounded tool checks and SoC completion connectivity pass.
- [ ] A real VM proof validates the connected M6 path, not a controlled verifier.

## Architecture Constraints

- Inherit ADR-0019, ADR-0020, and ADR-0021. Do not change proof semantics.
- One proof runs at a time. Final signing follows capacity reservation and snapshot preparation.
- Policy publication is independent. Exact approved versions stay pinned.
- Agents receive no PolicyIR, grant recipe, tuples, stored caveats, salts, openings, or witness.
- Authentication and inbox approval do not authorize data without gateway proof checks.
- The independent verifier never calls the issuer. No production mock-proof flag exists.

## File Scope

- All three repositories were reinspected on 2026-09-30. The task's writable root still
  covers only `hypha-backend`; sibling edits require scoped write approval.
- Agent repo: `src/`, `public/`, `test/`, `scripts/`, `vendor/`, package and Docker
  configuration, compatibility record, README, and local agent instructions.
- Backend: dataset-client seed helpers and authenticated seed route, dataset workspace
  seed control, focused seed tests, scenario documentation, and deployment/runbook docs.
- Extend the existing gateway dataset route/store with atomic create-only semantics.
  A seed must not replace a concurrently created operator draft.
- SDK: only fixes exposed by connected consumption, not a second implementation.
- Use Node 24's native HTTP, fetch, crypto, and TypeScript support with the packaged
  B-0066 SDK. Reuse approved TypeScript/protobuf/type pins for checks.

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
- Agent state persists the intent and exact issuance attempt before publication. Uncertain
  replies retry those records, not new task IDs or nonces. Pause does not cancel a proof.
- Query windows can change with the same capability. Task, dataset, and agent bindings
  stay fixed. Failed reads preserve the last successful query window.
- Pause rechecks local state after status reads and signing. A completed signing
  attempt stays private for exact reuse on Resume. In-flight API calls and admitted
  proofs are not interrupted. Paused cancellations keep status polling to terminal.
- Dashboard settlement results follow the latest fetched channel messages. Cached
  evidence is a fallback during channel failure, not a replacement for a fresh reply.
- Use separate agent containers/volumes and an internal authenticated message channel.
  Do not connect agents to backend owner-only networks.
- No paid ngrok subscription is required. One free assigned HTTPS domain can serve
  both SDK clients. Keep the three dashboard ports SSH-forwarded for the recording.
- The Mac has ngrok 3.39.11. Use its existing client with an SSH-forwarded VM console
  port as the no-install ingress option. That API route requires the Mac and tunnel
  to stay running. The runbook includes matching nondefault dashboard origins.

## Open Questions And Blockers

- No feature decision blocks B-0067. Docker and SSH ControlMaster are available.
- The seed overwrite race is fixed through gateway create-only storage. The focused
  operator-write, concurrent seed, partial retry, and finalization checks now pass.
- Sibling source writes still need the task sandbox's scoped execution approval.
- Public npm authentication/publication is deferred, not needed for the candidate install.
- The complete VM release build and live M6 proof validation are unrun.
- The maintainer approved the local checkpoint, finished runbook, commits, and
  pushes on 2026-10-01. No VM source or runtime deployment changed.
- VM agent API reachability needs a trusted HTTPS endpoint. The Mac client is present.
  The VM SSH path has no ngrok command. No installation is needed for the Mac/SSH
  route. Account configuration, assigned endpoint, and API reachability remain
  deployment checks. No public tunnel has been started by this session.
- Project source-folder listings do not change this task's supplied filesystem permissions.

## Work Completed

M0-M5 remain complete. B-0064 delivered registration, token/key authentication, and
SDK transport. B-0065 delivered the inbox, independent policy versions, approval,
and real snapshot preparation. B-0066 delivered serial dispatch, exact signatures,
durable proof handoff/results, SDK polling, and protected-query reuse.

The live backend Compose overlay and VM runbook are implemented. PostgreSQL migration
and synthetic grant persistence through restart pass. The existing backend preview
is unchanged. The agent repo now has both desktop dashboards, persistent separate
Ed25519 identities, owner-only JSON state, SDK request/sign/poll/query flows,
authenticated bounded messages, exact integer matching, and a bounded LLM adapter.
The task runtime persists immutable intents and issuance attempts before publication.
The dashboards expose manual controls, public capability claims, authorized rows,
verifier/cache receipts, messages, results, pause/resume/retry/cancel, and model output.
Editable UTC query windows reuse the existing capability. The operator seed creates
draft processor and bank ledgers and cannot replace concurrent operator edits.

The isolated local agent preview is running at ports 3004/3005 under Docker project
`raziel-agents-b0067-check`. Both agents and the channel are healthy. Browser sign-in,
identity retention across recreation, model configuration, and task-form startup pass.
These previews have no registered Raziel connection or fabricated capability/data.
The SoC file is injected only into agent runtime environments. A bounded completion
connectivity check passed. Full model-driven reconciliation remains unrun.
The three-dashboard deployment runbook is `docs/runbooks/M7-AGENT-DEMO.md`.
The agent checkpoint is committed and pushed at
`54aa8090d1b2ff20d4bb545a71b7745fc9025efd`. No new VM proof, remote source update,
runtime deployment, or public npm release ran in this session.
M6 remains in progress until its live proof and release follow-ups are resolved.
The bounded task-control review found three application bugs. Pause during a status
read or signing could still submit issuance. Paused cancellation stopped polling.
Repeated settlement replies could leave the displayed result stale. All three are
fixed, with four focused regressions. Proof contracts and queue behavior are unchanged.

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
| Resume repository check, 2026-09-30 | PASS | Backend and agents clean; known SDK `.DS_Store` preserved |
| Docker availability, 2026-09-30 | NOT AVAILABLE | `docker info` failed: Docker socket does not exist |
| SSH ControlMaster, 2026-09-30 | NOT AVAILABLE | `ssh -O check -o BatchMode=yes gapaul-vm-i` found no control socket |
| Runtime checks, 2026-10-01 | PASS | Docker 29.8.0 Linux arm64; VM master PID 4117; remote hostname matched on resume |
| Initial seed check, 2026-10-01 | FAIL, resolved | Operator-write race found and fixed through atomic create-only storage |
| Seed and console dataset routes | PASS | 43 focused checks: 40 seed and 3 existing route cases |
| Gateway store/routes and dataset bindings | PASS | 15 focused checks: 11 store/route and 4 binding cases |
| B-0067 console regression | PASS | 497 passed, 18 opt-in checks skipped; no real VM proof claim |
| B-0067 gateway regression | PASS | 34 passed, including protected queries and exact dataset bindings |
| Edited backend source check | PASS | Biome check on 9 edited seed, route, store, UI, and test files |
| Console and gateway types/format | PASS | Both scoped TypeScript checks and edited-file formatting pass |
| Agent dependency install | PASS | Approved exact pins and packaged SDK installed; frozen-lockfile production Docker build passes |
| Agent runtime | PASS | 13 focused Node checks: persistence, SDK signing, query reuse, range bounds, channel, sessions, identity, matching, bounded tools, pause races, cancellation polling, and fresh results |
| Task-control fixes | PASS | Four regressions plus scoped TypeScript check; no production proof substitute |
| Agent types/format/browser script syntax | PASS | Pinned Docker typecheck, final 21-file format check, and browser script syntax |
| Final agent image rebuild | PASS | Frozen-lockfile production build on the existing pinned arm64 toolchain after task-control fixes |
| Final local startup | PASS | Rebuilt Agent A, Agent B, and channel all healthy; private volumes retained |
| M6 live Compose render | PASS | `check-m6-live-compose`: 56 GiB, eight CPUs, no swap; 2 GiB networkless verifier; runtime not started |
| Agent Git checkpoint | PASS | `54aa809` committed and pushed to its own GitHub `origin/main`; candidate checksum rechecked |
| SoC API completion connectivity | PASS | Supplied private configuration, bounded request; no business tool/proof claim |
| Agent Docker/browser checkpoint | PASS | Three healthy containers; both sign-ins, retained Agent A identity, Agent B task form |
| Connected M6/M7 VM proof and final recording | SKIP | Not run; no synthetic verifier result substituted into the production runtime |

## Risks And Private-Data Notes

- Preserve owner-only keys, snapshots, grants, and request comparison files.
- Live M6 uses PostgreSQL. The old partial preview still uses memory-backed SpiceDB.
- Use the packaged candidate, not sibling source imports or private generated files.
- SDK commit: `e7f1cd6b354f236f0323d9d19aa4a431c5dfa470`.
- Candidate SHA-256: `556abe87978429a1542bfbbe8d81edd48a8be92ceeb07b1731eb18e8233ce572`.
- Agent repo has B-0067 scaffold and foundation changes. SDK `.DS_Store` stays excluded.
- The maintainer supplied `soclaas.env`. Use only a private runtime reference.
  Keep API credentials out of Git, dashboard responses, model context, and logs.
- Unrelated `outputs/` and generated `next-env.d.ts` changes are preserved.
- Local preview screenshots are ignored under `.raziel/b0067-preview/`. They contain
  no tokens, private keys, witness, or fabricated proof result. Keep them distinct from VM evidence.
- The seed store retains its singleton writer assumption. It is not a multi-process
  file-store locking change. Seed checks use the real store with controlled HTTP transport.

## Exact Next Action

Have the maintainer pull both changed repositories on the VM using
`docs/runbooks/M7-AGENT-DEMO.md`. The agent checkpoint is `54aa809`; the matching
backend checkpoint includes this baton and the runbook. Check HTTPS API
reachability, deploy with the M6 live and M7 agent runbooks, and validate one connected
real SDK proof path before marking B-0067 complete. B-0068 retains both real proofs,
the three-dashboard walkthrough, negative controls, screenshots, and final demo evidence.
