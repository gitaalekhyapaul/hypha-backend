# B-0067 State Before the 2026-10-08 Release Checkpoint

This is a dated state snapshot, not the current engineering status. The release
checkpoint resolves its uncommitted inventory. See `CURRENT-BATON.md` for the
current SDK and agent commits, validation, remaining gates, and next action.
The broader B-0067 objective is not complete.

# Current Engineering Baton

- Baton ID: B-0067
- State: ACTIVE
- Milestone: M7, two-agent reconciliation dashboards
- Owner: Codex
- Started UTC: 2026-10-01T06:38:26Z
- Last updated UTC: 2026-10-08T01:41:00Z
- Branch: `main`
- Worktree: B-0066 backend baseline is `c5efbfb`. The B-0067 agent checkpoint is
  committed and pushed at `54aa809`. This backend checkpoint contains the seed
  feature, runbook, and current state. Unrelated outputs and generated `next-env.d.ts`
  remain excluded. SDK README and release metadata changed for the authorized
  publication attempt. Its known untracked `.DS_Store` remains excluded. Agent
  package, lockfile, Docker/configuration, and release docs now contain the checked
  public-registry migration. Release follow-up edits remain uncommitted/unpushed.
- Execution: local checkpoint committed and pushed. The later VM readiness check
  found that SSH ControlMaster no longer accepts connections. Work is paused under
  the maintainer's standing instruction to stop and notify on missing SSH access.
  No remote source was changed.

## Objective

Authorized release checkpoint: verify the published SDK, clean generated metadata,
commit the SDK/agent/backend release changes, and push all three repositories.
Run narrow consumer checks and preserve dated evidence. No VM deployment, real
proof run, dependency-version change, or proof-contract edit belongs to this work.
The pending B-0067 feature objective remains:

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
  The separately authorized release follow-up covers README/release metadata,
  prebuilt package staging, one npm publication attempt, and release evidence.
- Backend release follow-up: current-state SDK status in README, architecture,
  technical spec, M6/M7 runbooks, this baton, and the dated release record.
- Agent registry-consumption follow-up: package.json, generated pnpm lockfile,
  pnpm workspace settings, Dockerfile, package-store ignores, README, and historical
  vendor provenance. Validate frozen install,
  existing agent tests, types/format, and a production image build. No SDK runtime
  or backend compatibility-shim change is in scope.
- Use Node 24's native HTTP, fetch, crypto, and TypeScript support with the packaged
  B-0066 SDK. Reuse approved TypeScript/protobuf/type pins for checks.

## Out Of Scope

- Real-data acceptance, another domain, parallel proofs, new guest semantics, identity infrastructure.
- Further npm publication attempts or version changes outside the separately
  authorized `0.1.0` release follow-up.
- Automatic VM pulls, deployment, or private data migration during the pause.

## Decisions

- Finish B-0066, commit/push both changed repos, report progress, then pause.
- B-0066 closes as an implementation checkpoint. The maintainer deferred npm publication.
  The packaged candidate suffices for provisional VM testing.
- Later authorization permits one npm publication attempt for the same `0.1.0`
  implementation. Scope: SDK README/release metadata, public package staging,
  authentication and registry checks, one publish command, and result evidence.
  No dependencies, runtime code, proof contracts, or version changes are permitted.
- The one `npm publish` command exited successfully after maintainer account approval.
  Anonymous package-wide registry metadata now serves `0.1.0` with `latest: 0.1.0`.
  The downloaded public archive matches byte-for-byte. npm's website still showed
  its temporary holding page at the last browser check. No tag repair or second
  publication was needed. The subsequent agent migration pins public `0.1.0` and
  its integrity in the lockfile. The backend compatibility shim remains unchanged.
- The maintainer requested public-registry agent consumption after publication.
  Migrate the existing dependency to exact npm version `0.1.0`. Preserve all other
  dependency pins, agent identities/state, proof contracts, and the backend shim.
  This bounded local follow-up does not resume VM deployment. Public package
  installation needs no npm publishing token or 2FA.
- pnpm 11 generated a release-age exception only for the newly published exact
  SDK `0.1.0`. Its compiled code and registry archive were verified in the preceding
  publication gate. Retain that version-scoped setting in Docker; do not disable
  release-age checks globally or add a wildcard exception.
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

- No feature decision blocks B-0067. VM validation is blocked by SSH ControlMaster:
  `ssh -O check -o BatchMode=yes gapaul-vm-i` returns `Connection refused` both
  inside and outside the sandbox. Do not recreate the interactive MFA connection.
- The seed overwrite race is fixed through gateway create-only storage. The focused
  operator-write, concurrent seed, partial retry, and finalization checks now pass.
- Sibling source writes still need the task sandbox's scoped execution approval.
- Public npm publication is verified. `npm view` and anonymous registry metadata
  resolve `@gitaalekhyapaul/raziel-sdk-ts@0.1.0`, with `latest` pointing to `0.1.0`.
  The early 404 and temporary holding state resolved in the registry without a tag
  change. The website still displayed the holding page at the last browser check.
- The publication reused the checked B-0066 compiled tarball. Only README and
  release-status metadata changed. All compiled files, package.json, and LICENSE
  stayed byte-identical. No dependency installation or runtime rebuild ran.
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
or runtime deployment ran in this session. The authorized npm release follow-up
published `0.1.0` successfully. Anonymous registry metadata, `latest`, and public
archive integrity are verified.
See [SDK publication evidence](2026-10-01-sdk-publication-attempt.md).
The agent repository now installs exact public SDK `0.1.0`, not the vendored
candidate. Its frozen install, 13 tests, typecheck, formatting, production image
build, and networkless SDK import pass. Existing dependency versions and runtime
source are unchanged. VM deployment and running previews were not changed.
See [agent migration evidence](2026-10-01-agent-public-sdk-consumption.md).
M6 remains in progress until its live proof and release follow-ups are resolved.
The bounded task-control review found three application bugs. Pause during a status
read or signing could still submit issuance. Paused cancellation stopped polling.
Repeated settlement replies could leave the displayed result stale. All three are
fixed, with four focused regressions. Proof contracts and queue behavior are unchanged.

See [B-0066 archive](B-0066-serial-dispatch-and-sdk-delivery.md).

## Validation Plan

On resume, use [M6 Live VM](../../runbooks/M6-LIVE-VM.md) for the provisional deployment.
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
| New VM proof | SKIP | Explicit follow-up, not claimed complete |
| SDK public publication attempt, 2026-10-01 | PASS | One `npm publish` exited 0 after account approval; version metadata and archive match |
| SDK normal registry availability, 2026-10-01 | PASS | Early 404 resolved; anonymous package metadata and `npm view` serve `0.1.0`; `latest: 0.1.0` |
| SDK website display, 2026-10-01 | NOT AVAILABLE | Last browser refresh still showed the temporary holding page; registry publication is independently verified |
| Agent public SDK consumption, 2026-10-01 | PASS | Exact npm 0.1.0; matching integrity; frozen install, 13 tests, types/format, production image, and networkless ESM import |
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
| Backend Git checkpoint | PASS | `b12893d` committed and pushed; remote `refs/heads/main` matched |
| Later VM readiness check | NOT AVAILABLE | ControlMaster connection refused; no VM commit or runtime state read |
| Local ngrok configuration | PASS | Existing config validates; no credentials printed and no public tunnel started |
| SoC API completion connectivity | PASS | Supplied private configuration, bounded request; no business tool/proof claim |
| Agent Docker/browser checkpoint | PASS | Three healthy containers; both sign-ins, retained Agent A identity, Agent B task form |
| Connected M6/M7 VM proof and final recording | SKIP | Not run; no synthetic verifier result substituted into the production runtime |

## Risks And Private-Data Notes

- Preserve owner-only keys, snapshots, grants, and request comparison files.
- Live M6 uses PostgreSQL. The old partial preview still uses memory-backed SpiceDB.
- Agents now use exact public SDK `0.1.0`, not sibling source imports, private
  generated files, or the historical tarball. The backend shim still uses its
  existing candidate pin. Neither VM nor running previews were redeployed.
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

Review the accounted release changes below, then checkpoint them as three scoped
commits before VM deployment. Do not stage the four excluded local artifacts.
Wait for the maintainer to restore the SSH ControlMaster and resume execution.
Then have the maintainer pull both changed repositories on the VM using
`docs/runbooks/M7-AGENT-DEMO.md`. The agent checkpoint is `54aa809`; the matching
backend checkpoint includes this baton and the runbook. Check HTTPS API
reachability, deploy with the M6 live and M7 agent runbooks, and validate one connected
real SDK proof path before marking B-0067 complete. B-0068 retains both real proofs,
the three-dashboard walkthrough, negative controls, screenshots, and final demo evidence.

## Uncommitted Review Inventory

Checked all three working trees at 2026-10-01T13:34:10Z. There are 22 uncommitted
paths: 18 release/migration files and four excluded local artifacts. No changes are
staged. No handwritten application, SDK implementation, Rust, Go, Protobuf, or proof
logic remains uncommitted. Every pending path is accounted for below.

| Repository | Files | Count | Reason |
| --- | --- | --- | --- |
| Backend | `README.md`, `ARCHITECTURE.md`, `docs/spec/POC-TECHNICAL-SPEC.md` | 3 | Current status now records public SDK publication and checked agent migration; live proof gates remain pending |
| Backend | `docs/agent/CURRENT-BATON.md` | 1 | Publication/migration scope, evidence, current SSH pause, and this inventory |
| Backend | `docs/runbooks/M6-LIVE-VM.md`, `docs/runbooks/M7-AGENT-DEMO.md` | 2 | Public-registry agent installation and commit-before-deployment instructions |
| Backend | `docs/agent/history/2026-10-01-sdk-publication-attempt.md`, `docs/agent/history/2026-10-01-agent-public-sdk-consumption.md` | 2 | New dated publication and consumer-validation evidence |
| SDK | `README.md`, `compatibility.json` | 2 | Correct supported methods/install instructions and public-release status; both match the submitted release archive |
| Agents | `package.json`, `pnpm-lock.yaml` | 2 | Exact public version 0.1.0 and verified registry integrity, replacing the local file dependency |
| Agents | `Dockerfile`, `pnpm-workspace.yaml` | 2 | Registry install without vendor copy; exact-version release-age exception for the verified new SDK |
| Agents | `.gitignore`, `.dockerignore` | 2 | Exclude generated pnpm cache and historical vendor files from the image |
| Agents | `README.md`, `vendor/README.md` | 2 | Explain public consumption, release order, and historical-only candidate storage |

Excluded local artifacts are preserved, not deleted, reverted, or staged:

| Repository | Path | Reason |
| --- | --- | --- |
| Backend | `apps/demo-console/next-env.d.ts` | Generated Next.js route-type import changed from `.next/types/routes.d.ts` to `.next/dev/types/routes.d.ts`; outside this release scope |
| Backend | `outputs/01a05808-0af7-70b0-a45b-ac91f8057949/NUS-Raziel-Hypha-External-Advisor-Shortlist.xlsx` | Unrelated advisor-report artifact, not created by this release follow-up; preserve ownership |
| Backend | `outputs/01a05808-0af7-70b0-a45b-ac91f8057949/NUS-Raziel-Hypha-External-Advisor-Shortlist.xlsx.inspect.ndjson` | Companion inspection artifact for that report; preserve ownership |
| SDK | `.DS_Store` | macOS local metadata, not source or release content |

Review focus: retain the exact `@gitaalekhyapaul/raziel-sdk-ts@0.1.0` age-check
exception, not a global or wildcard exception. No new architecture/ADR approval is
required. The backend compatibility shim is intentionally unchanged. Its local
candidate pin is committed baseline code, not unexplained pending work.

Validation: `git diff --check` passes in all three repositories. Agent frozen
install, 13 tests, types/format, production image, and networkless import passed in
the preceding follow-up. SDK README and metadata match the publication archive.
The recommended checkpoint is one commit per repository: SDK release documentation,
agent public-package consumption, and backend release/runbook evidence. The trees
are accounted for, not yet clean or committed.
