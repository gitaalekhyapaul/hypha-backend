# Current Engineering Baton

- Baton ID: B-0067
- State: READY
- Milestone: M7, two-agent reconciliation dashboards
- Owner: unassigned
- Started UTC: 2026-10-01T06:38:26Z
- Last updated UTC: 2026-10-14T07:46:11Z
- Branch: `main`
- Worktree: release changes are checkpointed by the backend commit containing this
  file. SDK and agent commits below are already pushed and remote-verified.
- Execution: maintainer selected a Compose-managed ngrok tunnel with the account's
  fixed dev domain, portable between the Mac and VM. The bounded deployment follow-up
  is implemented and locally checked. No remote source or runtime edits.

## Objective

The SP1 native-runner cache repair is implemented and locally validated. The next
bounded objective is the maintainer's targeted VM prefetch and proof-runtime rebuild,
then full stack startup and Agent A's connected approval-to-data path. Both SP1 6.3.1
published runner lockfiles are prepared explicitly and checked offline. Raziel's
lockfile, proof features, source permissions, and private VM state stay unchanged.

Complete the connected agent approval, real proof, protected-query, and
reconciliation path using the packaged SDK and three dashboards.
The local feature and public-package checkpoints are ready for VM validation.
This release follow-up did not complete the live B-0067 acceptance gate.

## Acceptance Criteria

- [x] Separate agent identities, tokens, persisted task state, and desktop dashboards.
- [x] Bounded authenticated messages and integer reconciliation checks.
- [x] Editable, clearly labeled synthetic processor and bank seed drafts.
- [x] Server-side OpenAI-compatible LLM configuration and bounded SDK tools.
- [x] Public SDK `0.1.0`, exact agent dependency, lockfile, and production image.
- [x] Release documentation and migration committed and pushed to SDK/agent remotes.
- [ ] Each agent obtains its own real capability through operator approval and signing.
- [ ] SDK-only protected queries demonstrate independent verification and cache reuse.
- [ ] Connected dashboards show useful task/request/result states on the VM.
- [ ] Real proof-backed reconciliation and the full recording pass the M7 gate.

## Architecture Constraints

- ADR-0019, ADR-0020, and ADR-0021 remain authoritative. No proof semantics changed.
- One proof runs at a time. Signing follows capacity reservation and private preparation.
- Policy publication stays independent. Requests pin exact approved versions.
- Agents receive no PolicyIR, grant recipe, tuples, salts, openings, or private witness.
- Authentication and operator approval alone never authorize protected data.
- The verifier never calls the issuer. There is no production mock-proof flag.
- Use exact packaged SDK releases, never sibling source imports or floating `latest`.

## File Scope

- Agents: task runtime, desktop dashboards, tests, package/lockfile, Docker config,
  and release documentation. Current migration commit changes no runtime source.
- Backend: synthetic seed feature, dataset bindings, live deployment/runbook docs,
  current status, and dated release evidence.
- SDK: reviewed implementation fixes only. This checkpoint changes release docs,
  compatibility metadata, and the macOS ignore rule, not runtime or dependency pins.
- Sibling writes and Git metadata use scoped sandbox approval.
- Recheck writable roots, current files, and runtime availability on the next resume.
- The October 8 guide correction touches only the M6/M7 runbooks, agent demo plan,
  current baton, and its dated documentation evidence.
- Ngrok follow-up: Compose overlay and wrapper, startup/preflight scripts, ngrok
  configuration and route policy, one deployment ADR, focused checks, architecture
  and runbook docs. Agents: configuration, default-origin presentation, Compose,
  env example, README, and focused tests. SDK remains read-only.
- Startup mount repair: `docker-compose.m6-live.yml`, `scripts/check-m6-live-compose`,
  `scripts/check-ngrok-compose`, M7 runbook, and dated baton evidence. Both siblings
  remain read-only. Reproduce with an isolated, networkless container, not the stack.
- Nested runtime repair: `docker-compose.m6.yml`, `scripts/prepare-m6-policy-runtime`,
  the policy/live/ngrok Compose checks, M7 runbook, and dated baton evidence. Existing
  host data sources stay fixed. Test isolated cold-start mounts for all four roles.
- Native-runner preparation repair: `scripts/prepare-sp1-native-dependencies`,
  `scripts/prepare-m6-demo`, `scripts/bootstrap`, `scripts/prepare-proof-sandbox-runtime`,
  build-boundary architecture/spec notes, M6/M7 runbooks, focused checks, current
  baton, and dated evidence. Both siblings remain read-only.

## Out Of Scope

Real-data acceptance, another scenario, identity infrastructure, parallel proofs,
new guest semantics, automatic VM pulls, or private-state migration.
No new SDK version or npm upload belongs to an unchanged release checkpoint.

## Decisions

- Maintainer authorized cleanup, scoped commits, and pushes across all three repos.
- SDK `0.1.0` is already public. Its pending docs match the public archive exactly.
  No second publication or tag repair is required.
- Agents now consume exact public `0.1.0`, with matching lockfile integrity.
  The obsolete archive is removed from the agent checkout. Its provenance remains
  in Git history and dated backend release evidence, not a build dependency.
  Local and VM installs use the same manifest. pnpm's local store supports offline
  installs without a file dependency or environment-specific SDK switch.
- Retain the previously checked pnpm age exception for this exact verified version.
  No global override or wildcard exception exists.
- The backend compatibility shim keeps its committed B-0066 candidate pin.
- ADR-0021 uses dedicated PostgreSQL 17.10 and fresh `raziel-m6-live` state.
  Private snapshots remain owner-only files.
- Pause stops new task actions, not admitted proof work. Cancellation keeps polling.
  Exact intents and signing attempts persist for uncertain retries and restart.
- A changed query window can reuse a capability within its task/dataset binding.
- SSH forwarding supplies Mac browser access to all three dashboards. It does not
  supply an API listener inside the VM agent containers.
- Maintainer selected ngrok ingress on October 8. Use the account's fixed HTTPS dev
  domain, start the tunnel inside Compose on the active backend host, and disable
  pooling. Expose agent API/protected-query routes, not operator routes or services.
  No TLS bypass, automatic VM pull, SDK release, or private-state migration.
- ADR-0022 records the chosen route and cloud trust boundary. The supplied domain is
  `https://either-anymore-immunize.ngrok-free.dev`. Image download was approved and
  completed. Agent dashboard origins protect their own browser actions, not SDK
  requests to the backend. This change adds no backend CORS or origin allowlist.
- Ask for Docker or SSH ControlMaster if unavailable. Never modify VM source directly.
- On October 14, the maintainer approved prefetching dependencies from SP1 6.3.1's
  two published runner lockfiles, including optional `gecko_profile` 0.4.0. This is
  cache preparation, not a new direct dependency or approval to enable profiling.

## Open Questions And Blockers

- The maintainer's October 8 VM ngrok render check passed. It confirms valid token
  configuration, not a working public endpoint. No token was shared or inspected.
  Local repair probes use synthetic settings and never start a tunnel. Keep the
  real token in ignored backend settings, never chat or Git.
- The October 14 VM retry passed the repaired runtime mounts and initializers.
  The proof build failed during SP1's nested offline Cargo metadata resolution,
  with missing `gecko_profile`. The later supplied access-API health report is
  healthy with five successful probes. Its earlier unhealthy state is no longer
  a confirmed blocker. Full stack and real M6/M7 acceptance remain unverified.
- Docker was initially unavailable locally. The maintainer started it and approved
  the pinned SP1 prefetch. No VM source, restart, or private-state action has run.
- The October 8 read-only check found a working SSH ControlMaster and reachable VM.
  The backend was clean at `7710e9a`; the agent checkout was absent at that check.
  Those are dated observations, not a claim about later maintainer changes.
- On resume, check the master first. If unavailable, pause and ask the maintainer
  to restore the interactive connection. Do not recreate MFA sessions.
- VM updates still require maintainer pulls through the server's PAT flow.

## Work Completed

M0-M5 remain validated. B-0064 delivered registration and signed SDK authentication.
B-0065 delivered inbox review, independent policy versions, and private preparation.
B-0066 delivered serial dispatch, exact issuance signatures, terminal retention,
SDK polling, and protected-query reuse. Its local consumer used a controlled verifier,
not a replacement for the pending real VM gate.

B-0067 implements separate agent dashboards/keys/state, bounded messages and LLM
tools, task controls, query windows, and create-only synthetic ledger seeding.
Three task-control bugs were fixed in the feature checkpoint. Earlier local
Docker/browser and SoC completion checks remain dated evidence, not current VM claims.

Public SDK release docs: `6239fad9d02b1047806b3c449eb593ea7cd9f119`.
Public-package agent migration: `bb66ee3c2e9ea546661fd9e1134a5c956dedec24`.
Agent archive cleanup checkpoint: `84f9eb135532cc52b89bbfbf20d77c1f85b12b7d`.
These dated release commits were pushed. The later fixed-origin agent head is below.
The backend checkpoint is the commit containing this baton and the updated runbooks.

The SDK ignores macOS metadata. Next's existing declaration generator restored the
production route import without a handwritten generated-file edit.
The earlier advisor-report outputs are absent at this inspection. This checkpoint
did not delete or move them. Private runtime storage and ignored caches stay intact.

The deployment guides now separate VM backend, VM agents, private SoC files, and
Mac forwarding. They distinguish Compose settings, LLM credentials, generated
identity keys, and UI-provisioned Raziel tokens. The stale plan statement about
deferred npm publication is corrected. No app, SDK, VM, or credential changed.

The ngrok follow-up adds opt-in Compose startup on the VM and standalone Mac ingress.
Its API-only traffic policy excludes operator routes. Pooling and local inspection
are disabled. Both agents prefill the configured domain without replacing stored
connections. The runbook now documents separate browser origins and API destinations.
The SDK and its exact public package dependency are unchanged.
Agent fixed-origin checkpoint: `71d238ecca44fe5a98c50193fc028c1b2208da6e`.
The backend ingress checkpoint is the commit containing this updated baton.

The reported startup mount error was reproduced against Docker. An unquoted flow
list split the dispatcher's `/tmp` options into six mount entries. Quoting it keeps
one valid mount with the original 0700 mode and 256 MiB size. Both Compose check
scripts now reject relative tmpfs entries. A real isolated container passes with
the rendered mount. The runbook documents a pull and startup retry without deletion
or rebuilding. No VM or sibling source changed in this repair.

The maintainer's next startup exposed `/workspace/.raziel/m6` beneath a read-only
source mount. A disposable fresh workspace reproduced Docker's mountpoint-creation
failure. The repair moves only the container target to `/var/lib/raziel/m6` and
updates initializer, operator, policy, transaction, and receipt paths. Host sources
and existing state are unchanged. All four rendered role mounts pass real Docker
probes. Actual operator provisioning creates a 0600 record and retains it unchanged
on restart. Both initializers still reject writes to the source mount.

The October 14 proof-build failure is reproduced exactly after fetching only the
workspace lockfile in a fresh toolchain cache. SP1 runs Cargo metadata from its own
published runner manifest, which includes optional dependencies absent from the
workspace graph. The helper prefetches both checksum-pinned published lockfiles and
resolves metadata offline before the cache becomes read-only. Standard bootstrap
and M6 preparation invoke it. The proof build runs only its offline check. The
approved prefetch passes, and both actual metadata graphs resolve in a networkless,
read-only container. No direct dependency, profiling feature, SDK, API, or proof
semantics changed. The access-API report now shows healthy with five successful probes.

## Validation Plan

For ingress, check rendered networks, image pin, fixed URL, route policy, disabled
inspection/pooling, and credential scope. Validate the real CLI/config after image
approval. Agent tests cover configured-origin defaults without replacing stored
connections. No expensive proof rerun is needed for the transport-only checkpoint.

On resume, follow [M6 Live VM](../runbooks/M6-LIVE-VM.md) and
[M7 Agent Demo](../runbooks/M7-AGENT-DEMO.md). Start with Agent A's approval-to-data
path, then Agent B and shared reconciliation. Use real local Plonk capabilities.
Do not repeat broad test infrastructure work before the connected feature gate.

## Validation Evidence

| Check | Result | Scope |
| --- | --- | --- |
| Public registry, 2026-10-08 | PASS | Anonymous metadata: latest 0.1.0; 40-file archive, integrity/SHA-1/SHA-256 verified |
| SDK release source | PASS | README, compatibility metadata, package.json, and LICENSE match public archive; no runtime diff |
| Agent types, 2026-10-08 | PASS | Pinned Docker `pnpm typecheck` |
| Agent tests, 2026-10-08 | PASS | 13 passed, zero failed/skipped |
| Agent cache-only installation | PASS | Frozen lockfile, offline install and disabled networking; archive removed; 13 tests still pass |
| Agent formatting, 2026-10-08 | PASS | 21 files, no fixes |
| Agent image, 2026-10-08 | PASS | Pinned arm64 toolchain; frozen-lockfile build passes with cached install layer |
| Networkless SDK image import | PASS | Public 0.1.0, initial-public-release metadata, no /app/vendor |
| Initial image assertion | FAIL, resolved | Ad-hoc check used sdkVersion; corrected to the actual packageVersion field, no app edit |
| M6 Compose, 2026-10-08 | PASS | 56 GiB/eight CPU/no swap proof sandbox; 2 GiB/networkless verifier; runtime not started |
| Next declaration generation | PASS | Production import regenerated; file matches committed content |
| Diff whitespace checks | PASS | All three repos and staged SDK/agent diffs |
| SDK/agent Git pushes | PASS | Remote main heads match 6239fad and 84f9eb1 |
| Backend checkpoint | PASS | Release docs reviewed; final clean-tree and remote-head checks follow commit |
| SDK runtime rebuild/tests | SKIP | No runtime changes; existing 249-test B-0066 gate and public artifact audit retained |
| Connected VM proofs/deployment | SKIP | No remote source, runtime, proof, or recording action ran |
| Deployment guide static checks | PASS | Relative links, shell example syntax, ignored settings paths, and diff whitespace |
| Guide correction runtime tests | SKIP | Documentation only; no services, credentials, or VM configuration changed |
| Ngrok image and CLI | PASS | Approved exact digest downloaded; real version 3.39.11, config check, flags and health-check tools confirmed |
| Ngrok Compose layouts | PASS | Standalone Mac and integrated M6 render; fixed domain, token scope, network/mount isolation and pooling/inspection flags |
| Existing M6 limits | PASS | Render keeps 56 GiB/eight CPUs/no swap and 2 GiB/networkless verifier |
| API route vectors | PASS | Nine allowed routes and twelve denied methods/paths; static check, not cloud acceptance |
| Missing URL/token | PASS | Both preflight cases reject configuration without starting services |
| Agent types and tests | PASS | Direct pinned Node/TypeScript: 14 tests, zero failed/skipped in sequential run |
| Initial concurrent agent run | FAIL, resolved | Channel child missed existing startup window under contention; isolated and complete sequential runs pass without app/test changes |
| Agent formatting and image | PASS | 22 files, no fixes; frozen install layer cached; both image roles load supplied default origin |
| Initial networkless image build | FAIL, resolved | Different build cache key missed package cache; normal cache-backed build passes with no dependency edits |
| Ingress documentation | PASS | Relative links and shell examples parse; private settings are ignored |
| Public endpoint and real proofs | SKIP | No token, live tunnel, remote runtime, or proof was used in this follow-up |
| Reported mount failure | PASS | Reproduced Docker invalid-path error from the six-entry rendered dispatcher tmpfs |
| Repaired mount runtime | PASS | Real networkless container starts from rendered tmpfs; 0700 mode and 256 MiB size verified |
| Repair Compose checks | PASS | Baseline and ngrok-integrated M6 layouts, all tmpfs targets absolute, dispatcher exactly one mount |
| VM repair retry | SKIP | Maintainer must pull and retry startup; no remote action taken |
| Maintainer VM retry after tmpfs repair | PARTIAL | Render checks pass and dispatcher is created; nested runtime mount blocks policy-tools creation |
| Nested runtime failure reproduction | PASS | Fresh workspace reproduces Docker read-only mountpoint creation at the old target |
| Runtime path Compose checks | PASS | Policy preview, live M6, and ngrok-integrated layouts agree on all four runtime mounts |
| Actual cold runtime mounts | PASS | Four networkless disposable containers start with a fresh workspace; shared runtime is writable and both initializer source mounts stay read-only |
| Operator provisioning and restart | PASS | Actual initializer script creates a 0600 record at the configured path; second run leaves its contents unchanged |
| Runtime repair VM acceptance | SKIP | No remote source, services, tunnel, or proof changed locally |
| October 14 VM startup | FAIL | Native-runner metadata lacks gecko_profile; full stack did not complete |
| Later access-API health | PASS | Maintainer-supplied health report: healthy, zero failing streak, five successful probes; functional API acceptance not established |
| Native-runner error reproduction | PASS | Root locked fetch alone reproduces the exact gecko_profile error in a networkless, read-only cache |
| Published-lock integrity | PASS | Both original crate archives match workspace checksums; their lockfiles match the two helper SHA-256 pins |
| Approved native prefetch | PASS | Actual pinned Docker toolchain resolves both published lockfiles without rewriting workspace or package lockfiles |
| Offline native metadata | PASS | Both graphs resolve with no network and a read-only Cargo cache |
| Helper focused cases | PASS | Fetch/check modes, no check-mode fetch, changed/missing/duplicate inputs, lock mutation, and unknown option rejection |
| SP1 feature boundaries | PASS | Existing check retains blocking/native-gnark, excludes hosted/CUDA/experimental proving, and preserves verifier/RPC graph isolation |
| Native-repair full VM build | SKIP | No remote edit, rebuild, service start, or proof generation executed locally |

## Risks And Private-Data Notes

Keys, API tokens, SoC credentials, issuance attempts, capabilities, snapshots, and
private witnesses remain outside Git. Ignored runtime/cache/macOS files are local
artifacts, not pending source changes. No proof/public-value contract changed.
Real-data acceptance and a second scenario remain deferred and unclaimed.

## Exact Next Action

Have the maintainer pull the SP1 native-cache repair. Run
`./scripts/compose-m6 run --rm --no-deps m6-toolchain ./scripts/prepare-sp1-native-dependencies`,
then `./scripts/compose-m6 --profile apps run --rm --no-deps m6-proof-build`.
After that build passes, run `./scripts/start-m6-demo --ngrok` and inspect service
health. This targeted preparation creates no proof and deletes no state. No image
rebuild or package upgrade is required. Retain the existing `.env.m6`, volumes, and
agent checkout. For first setup, add the private
ngrok token and supplied domain to `.env.m6`, and the matching API default to
`.env.agents`. Follow the updated runbook and start the backend with `--ngrok`.
Keep browser access on SSH forwarding. Configure the two repo-local settings
files and private SoC credentials path as documented. Check Docker and the existing
SSH ControlMaster before runtime work. Validate Agent A's registration, inbox
approval, exact signing, real proof, and protected query, then Agent B and reconciliation.
B-0068 retains the two-proof recording, negative controls, screenshots, and evidence.

## Dated Evidence

- [B-0066 archive](history/B-0066-serial-dispatch-and-sdk-delivery.md)
- [SDK publication and October 8 follow-up](history/2026-10-01-sdk-publication-attempt.md)
- [Agent migration and October 8 follow-up](history/2026-10-01-agent-public-sdk-consumption.md)
- [Preserved pre-release state](history/2026-10-08-b0067-pre-release-state.md)
- [VM guide and secret-loading correction](history/2026-10-08-vm-runbook-and-secret-loading.md)
- [Fixed-domain ingress checkpoint](history/2026-10-08-fixed-domain-ngrok-ingress.md)
- [Dispatcher tmpfs startup repair](history/2026-10-08-m6-dispatcher-tmpfs-repair.md)
- [Runtime bind-mount startup repair](history/2026-10-08-m6-runtime-bind-mount-repair.md)
- [SP1 native-runner cache repair](history/2026-10-14-sp1-native-dependency-cache-repair.md)
