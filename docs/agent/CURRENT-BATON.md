# Current Engineering Baton

- Baton ID: B-0067
- State: READY
- Milestone: M7, two-agent reconciliation dashboards
- Owner: unassigned
- Started UTC: 2026-10-01T06:38:26Z
- Last updated UTC: 2026-10-14T23:46:00Z
- Branch: `main`
- Worktree: release changes are checkpointed by the backend commit containing this
  file. SDK and agent commits below are already pushed and remote-verified.
- Execution: maintainer selected a Compose-managed ngrok tunnel with the account's
  fixed dev domain, portable between the Mac and VM. The guest/key and policy-aware
  grant repairs are implemented and checked. VM validation used disposable public-only
  containers. No remote checkout or production service was changed.

## Objective

Completed bounded objective: preserve the existing approved guest/key pair, reject
identity drift before private input/proving, and derive operator grant controls
from the exact approved policy. No trusted key, guest semantics, grant construction,
SDK release, remote checkout, or production service changed. The checkpoint is ready
for the maintainer's VM pull and rebuild, not a completed real-proof claim.

Permanent M6 proof and verifier zero-swap enforcement is implemented and reviewed.
Per-container systemd annotations and actual cgroup health checks pass focused
validation. The maintainer confirmed the immediate runtime correction works and
approved this permanent repair.

Next bounded objective: deploy both repairs after admitted proofs finish or reach
terminal cancellation. Rebuild owner/prover binaries, recreate the stopped backend
roles, verify the public-only key check, then continue the real agent proof-to-data
gate. Complete proof success, independent queries, and the full two-agent gate are
not established by the diagnostic or isolated validation output.

Complete the connected agent approval, real proof, protected-query, and
reconciliation path using the packaged SDK and three dashboards.
The local feature and public-package checkpoints are ready for VM validation.
This release follow-up did not complete the live B-0067 acceptance gate.

## Acceptance Criteria

- [x] Approved public guest artifact is pinned and preparation cannot silently install a mismatched rebuild.
- [x] Child checks both approved program-key representations before receiving private input and retains post-proof validation.
- [x] Operator-only grant requirements come from the exact approved policy, with no agent-visible policy/recipe data.
- [x] Approval form hides irrelevant fields and focused Rust/TypeScript/desktop checks pass.
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
- Preparation/readiness follow-up: `scripts/prepare-m6-proof-runtime`,
  `scripts/prepare-m6-demo`, `docker-compose.m6.yml`, the policy/live/ngrok Compose
  checks, M6/M7 runbooks, current baton, and dated evidence. Reuse existing approved
  dependencies. No remote source edit, runtime restart, or proof generation.
- Public-cache recovery: `scripts/restore-m6-proof-go-cache`,
  `scripts/prepare-proof-sandbox-dependencies`, `scripts/check-m6-proof-preparation.mjs`,
  the M6 live overlay's public-cache comment, focused preparation checks, and
  build-boundary documentation. Source cache stays read-only. No private volume
  or credentials are mounted by the recovery container.
- Ngrok auth-file repair: `docker-compose.ngrok.yml`, `scripts/check-ngrok-compose`,
  M7 runbook, current baton, and dated evidence. SDK and agents remain read-only.
  No remote edit, live tunnel, real token, package download, or proof generation.
- Public health route: demo-console health handler and focused test,
  `tools/ngrok/traffic-policy.json`, `scripts/test-ngrok-policy.mjs`, ADR-0022,
  architecture/spec ingress notes, M7 runbook, current baton, and dated evidence.
- Permanent zero-swap repair: `docker-compose.m6-live.yml`,
  `scripts/check-m6-live-compose`, architecture and M6/M7 runbook notes, current
  baton, and dated evidence. SDK/agents remain read-only. Validate rendered
  annotations and health rejection with disposable, networkless containers only.
- Guest/key repair: approved public guest artifact and provenance under tools,
  `services/razield/src/one_shot`, prover startup/preparation scripts and their
  focused checks, child program-key environment settings, architecture/runbook
  docs, dated evidence. Preserve the approved c809 key pair.
- Policy-aware grant form: owner catalogue and grant-requirement derivation in
  `tools/raziel-admin`, console catalogue types/client/form and focused tests.
  Only the owner-only projection changes. No Protobuf or public agent API change.

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
- The maintainer approved recovery of the existing public Go cache. Validate exact
  SP1 module pins and source checksums, then verify module completeness and integrity
  offline before use. The M6 destination stays separate from the legacy source.
- Ngrok's image entrypoint keeps its standard token-file generation. Its auth
  directory is private bounded tmpfs, not a persistent volume. `NGROK_CONFIG`
  selects the existing read-only base config. This is an implementation repair,
  not a new provider, image, or exposure decision.
- The maintainer explicitly requested unauthenticated deployment health on October 15.
  Security review scope: public liveness only. No policy, identity, registry, queue,
  dataset, environment, version, or proof-state fields. Existing authenticated
  routes keep their guards. This endpoint makes no proof-stack readiness claim.
- On October 15 the maintainer confirmed targeted runtime MemorySwapMax=0 works
  and approved a permanent repair. The existing ADR-0015/ADR-0013 no-swap rule is
  unchanged. The M6 sandbox annotations set an explicit per-unit systemd zero.
  Health checks read the actual cgroup limits, not Docker's requested values.
  No host-wide drop-in, swapoff, extra privilege, package change, or child bypass.

## Open Questions And Blockers

- The later VM job ran 966,922 ms and peaked at 48,544,919,552 bytes, with zero
  swap/OOM/restarts, then returned generic SERVICE_UNAVAILABLE. Its detailed
  internal error was not retained. A public-only LightProver probe confirms the
  installed guest derives 925b...18d5, but deployment expects approved c809...08a1.
  The repaired child checks both public identifiers before private input. The
  recovered public c809 guest is checksum-pinned and verified. The running stack
  still needs the maintainer's pull/rebuild. Do not claim the lost internal error
  is known or the new proof has passed.
- Security review item: operator grant-requirement field names are policy metadata.
  Derive them only from the owner-approved package, keep them behind existing
  operator auth, and do not add them to agent APIs. This does not change grant
  materialization, proof/public-value semantics, or any trusted key. The internal
  catalogue payload is now version 2 so pre-repair responses cannot hide required
  controls. Independent review's mixed-version finding is fixed and retested.
- October 15 proof attempt failed 35 ms after claim. VM cgroup swap was `max`,
  although Docker recorded equal memory/memory-swap limits. Systemd showed
  MemorySwapMax=infinity for both sandboxes. No OOM events occurred. The child
  rejects this resource profile before private input. The maintainer reports the
  scoped runtime systemd correction works. The permanent Compose repair passes
  disposable VM recreation and local health-positive/negative checks. Production
  deployment has since been observed with actual swap=0. The next successful real
  proof remains pending.
- The October 15 VM log confirms ngrok cannot create its auth file on the read-only
  root. The failure is reproduced locally, and the repair passes actual image
  entrypoint/config checks plus local health-API startup with no external network.
  The real VM token, public endpoint, and signed agent connection still need validation.
- The maintainer's October 8 VM ngrok render check passed. It confirms valid token
  configuration, not a working public endpoint. No token was shared or inspected.
  Local repair probes use synthetic settings and never start a tunnel. Keep the
  real token in ignored backend settings, never chat or Git.
- The October 14 native-runner repair now passes on the VM. The supplied access-API
  health report is healthy with five successful probes. Neither missing
  `gecko_profile` nor the earlier access-API health failure is a current blocker.
- Docker was initially unavailable locally. The maintainer started it and approved
  the pinned SP1 prefetch. No VM source, restart, or private-state action has run.
- The later VM retry lacks the finalized Go modfile in the M6 cache. The earlier
  targeted instructions omitted Go preparation. The new ordered preparation helper
  corrects that omission. The console HTTP health check corrects ngrok's missing
  readiness dependency. Both repairs pass focused local checks, not live VM acceptance.
- A fresh local Go fetch fails: SP1's pinned gnark-ignition-verifier pseudo-version
  returns HTTP 404 upstream. The maintainer confirmed both public Go cache volumes
  exist on the VM, including `raziel_razield-proof-go-mod-cache` from earlier runs.
  The M6 public cache now passes offline native/Go checks and the isolated build.
  No pin change is approved.
  A new host without a complete matching cache cannot bypass the upstream failure.
- Earlier SSH checks lacked a ControlMaster. The maintainer restored it on October
  15. Read-only inspection confirmed the applied swap mismatch in both sandboxes.
  Disposable annotated-container probes passed and were removed. No remote source,
  production scope property, daemon reload, restart, or proof submission ran.
- On resume, check the master first. If unavailable, pause and ask the maintainer
  to restore the interactive connection. Do not recreate MFA sessions.
- VM updates still require maintainer pulls through the server's PAT flow.

## Work Completed

The October 15 guest repair pins the recovered public interpreter at SHA-256
817b8689719d2079756160e56fe779b96f8e8194b77f68b5cca1c82fcb664a41.
Runtime preparation verifies its checksum and derives both approved key identifiers
before installation. The child repeats key validation before private IPC. The
post-proof journal/key checks remain. The public-only CLI accepts the approved
guest and rejects the installed mismatched guest without private mounts or proving.

The grant form uses a six-name, typed operator-only projection from the exact
approved package. The owner socket preserves the metadata and rejects old payloads.
Direct, team, organization, referenced caveat, and ANY/ALL requirements are covered.
The form resets edits on policy changes and preserves exact uncertain retries.
Grant materialization, complete payload defaults, and all agent responses stay fixed.
The desktop fixture checks five policy shapes without issuing a grant or proof.

The permanent M6 zero-swap repair adds the explicit systemd property to exactly
the proof and verifier sandbox services. Each health check retains its launcher
socket mode/owner guard and checks its actual RAM cap plus memory.swap.max=0.
Existing worker/API healthy dependencies remain in place. Native Mac cgroupfs
positive and negative health cases pass. Two fresh disposable Compose containers
on the VM independently report systemd MemorySwapMax=0 and cgroup swap=0.
No host-wide daemon reload was run against the active demo. The runbook explains
safe two-sandbox recreation and confirms no proof, image, or binary rebuild.

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

The follow-up provides one ordered proof-preparation command and an explicit legacy
public Go cache recovery option. The source cache stays read-only. Recovery checks
the exact SP1 module manifest, pinned source checksums, complete offline downloads,
and module integrity, then copies without overwriting destination entries. It
verifies the destination before the networkless proof build. No private volumes,
keys, credentials, Docker socket, or new dependency versions enter that role.
The console now has a bounded internal HTTP readiness check required by ngrok.
Local real-Go probes accept an intact read-only cache and reject changed or missing
modules. The recovery command preserves its source and rejects a conflicting
destination. These probes use a small controlled cache, not the VM's full SP1 cache.

The October 15 auth-file repair adds a private 1 MiB tmpfs owned by UID/GID 1000,
mode 0700, at the image's fixed auth directory. An explicit base-config environment
setting avoids the default config path hidden by that mount. The unmodified pinned
image entrypoint generates and validates its synthetic auth file. The HTTP CLI
starts its local health API with no external network and no public tunnel.
Standalone/integrated checks retain the image, user, read-only root, network, route,
and credential-scope constraints. No SDK, agent source, real token, or VM changed.

The requested GET `/api/health` route returns exactly status=ok without reading
private configuration, setting a cookie, or querying any downstream service.
The response is non-cacheable. The ingress permits only that exact GET path as
an unauthenticated exception. ADR-0022 records the explicit maintainer approval
and its liveness-only security scope. Existing operator-auth regression tests pass.

## Validation Plan

For this repair, validate the pinned artifact, actual key-pair derivation, negative
drift cases, child guard ordering, exact-policy field derivation, strict owner
transport, uncertain retries, desktop field visibility, and deployment syntax.
Run no expensive proof or production restart as part of the isolated checkpoint.

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
| VM native-runner repair | PASS | Maintainer output confirms prefetch and offline native metadata now pass |
| Fresh local Gnark fetch | FAIL | Approved SP1 module pseudo-version returns HTTP 404 upstream; no version or fork substituted |
| Preparation focused checks | PASS | Ordered fetch/build stages, opt-in recovery, malformed arguments, fail-fast behavior, pinned-manifest/checksum rejection, and mount isolation |
| Actual Go read-only cache | PASS | Small real-module probe verifies offline with read-only source/cache mounts; modified and absent modules reject |
| Actual recovery command | PASS | Controlled cache copy with no network leaves source unchanged and keeps existing destination entries; conflicting destination rejects |
| Probe harness corrections | PASS | Source go.sum holds only the xid go.mod checksum; probe adds its cached zip checksum. Conflict probe creates the conflict before recovery finalizes read-only files |
| Console HTTP probe | PASS | Real pinned Node probe accepts HTTP 200 and rejects HTTP 503 or a closed port; actual Next.js startup not claimed |
| Preparation/readiness Compose checks | PASS | Live M6, policy preview, integrated ngrok, and existing proof sandbox checks pass; resource and network limits unchanged |
| Preparation script format/lint | PASS | Existing pinned Linux Biome checks the new focused script; no package installed. Host Darwin Biome binary is absent |
| Shell/runbook/diff checks | PASS | Edited shell scripts and runbook commands parse, relative document links resolve, diff whitespace is clean, and scoped credential/private-artifact scan passes |
| Sibling worktrees | PASS | SDK and agents remain clean and unchanged in this backend-only repair |
| VM legacy cache recovery/full stack | SKIP | Maintainer must pull, run recovery, and report health; no remote action or proof ran |
| Ngrok original HTTP startup | PASS | Pinned image reproduces the reported read-only auth-file error with a synthetic token and no network |
| Ngrok auth RAM mount | PASS | Original entrypoint creates and validates auth config; parent mode 0700, UID/GID 1000, tmpfs backing, and read-only root verified |
| Ngrok actual HTTP CLI | PASS | Local loopback health API responds with no external network and no public tunnel |
| Ngrok repair Compose/policy checks | PASS | Standalone and M6 layouts pass; existing nine allowed and twelve denied route vectors still pass |
| Ngrok auth guard negatives | PASS | Missing auth mount, missing base-config setting, and world-readable directory mode all reject |
| Ngrok repair handoff checks | PASS | Shell/runbook syntax, relative links, diff whitespace, and scoped credential/private-artifact scan |
| Ngrok live VM retry | SKIP | Maintainer must pull and recreate console/tunnel, then test public health, unsigned API routing, and signed Save connection |
| Public health and operator-auth tests | PASS | Eight focused tests in the pinned networkless toolchain; status-only response, no cookie/cache, existing auth guards unchanged |
| Public route policy | PASS | Ten allowed and sixteen denied vectors, including health method/subpath rejections |
| Console health-route typecheck | PASS | Pinned toolchain tsc --noEmit, no dependency change |
| Health route formatting/lint | PASS | Existing pinned Biome checks route, test, and policy vectors |
| Public health VM acceptance | SKIP | No live tunnel, private token, or VM action in this implementation |
| VM failure diagnosis, October 15 | PASS | Retained job failed in 35 ms; PROVING precedes the executor; sandbox OOM counters zero; both actual swap limits max despite equal Docker requested limits |
| Native VM annotation | PASS | Disposable networkless container registers systemd MemorySwapMax=0 and actual cgroup swap=0; container removed |
| Actual VM Compose recreation | PASS | Two fresh containers from the rendered annotation each register systemd=0 and cgroup=0; isolated project removed after each run |
| Local actual health commands | PASS | Proof/verifier commands each accept zero swap and reject unlimited swap with a fixed diagnostic; four cases, no private mounts |
| Permanent repair Compose checks | PASS | M6 live, M6 policy preview, ngrok-integrated M6, baseline proof and baseline verifier render checks; annotation scope exactly two M6 sandboxes |
| Initial ngrok render attempts | FAIL, resolved | Missing synthetic render password/URL; rerun with explicit dummy settings passes, no tunnel or real token used |
| Repair syntax/whitespace | PASS | Shell syntax and git diff --check |
| Runbook and focused review | PASS | 27 documented shell blocks parse, relative links resolve; independent annotation/type/inheritance/cgroupfs review has no actionable findings |
| Production permanent repair/proof | SKIP | Maintainer must pull and deploy after active proofs finish; no production sandbox recreation, daemon reload, or new proof |
| Guest/public-key regression | PASS | Isolated x86_64 VM: 16 one-shot tests, one helper entry ignored by design; exact approved ID/hash plus both mismatch cases |
| Owner-service regressions | PASS | Isolated VM: 125 passed, four existing live external-service tests ignored; includes request approval, catalogue, and owner wire version-2 tests |
| Actual public-only preparation | PASS | Offline runtime preparation into container tmpfs, approved key check, installed checksum, no private volumes; corrupted checksum rejects before installation |
| Installed mismatched guest | PASS | New public-only CLI rejects current 925b guest against approved c809 pair, no IPC, snapshot, network, or proof |
| Grant console regressions | PASS | Pinned networkless toolchain: 220 tests in four files, tsc --noEmit, scoped Biome checks |
| Desktop grant controls | PASS | Playwright at 1440x1000: direct/team/organization/caveated/combined fields and selection reset; synthetic API fixtures, no service writes |
| Mixed-version review finding | PASS | Internal catalogue payload version 2 rejects pre-repair metadata-free response; framed regression added, browser JSON stays version 1 |
| Initial isolated validation setup | FAIL, resolved | Source archive ownership metadata rejected by capability-dropped container; portable extraction fixed. Owner-mutation test needed CHOWN in disposable test container only |
| Local prover validation | NOT AVAILABLE | Mac's offline cache lacks locked anstream 1.0.0; no download or dependency change. Complete VM cache used instead |
| Final preparation/Compose/docs | PASS | Artifact/preparation checks, base proof, M6 preview/live, integrated ngrok, shell syntax, 28 runbook shell blocks, and diff whitespace |
| Repaired-stack real proof | SKIP | Maintainer deployment and fresh agent request required. No production runtime changed and no proof generated in this checkpoint |

## Risks And Private-Data Notes

Keys, API tokens, SoC credentials, issuance attempts, capabilities, snapshots, and
private witnesses remain outside Git. Ignored runtime/cache/macOS files are local
artifacts, not pending source changes. No proof/public-value contract changed.
Real-data acceptance and a second scenario remain deferred and unclaimed.

## Exact Next Action

Have the maintainer pull the backend checkpoint and follow
[Guest Identity And Grant Form Repair](../runbooks/M6-LIVE-VM.md#guest-identity-and-grant-form-repair)
only after admitted proofs finish or reach terminal cancellation. Rebuild the
owner/prover binaries offline, recreate the stopped backend roles, and expect
approved_program_key=pass before a fresh agent request. Confirm policy-aware inputs
in the browser, then continue the real proof and protected-query gate.
Do not interrupt a working proof, rotate the trusted key, or reset volumes.
Keep existing settings, identities, private data, and sibling checkouts.
Use the public health and unsigned API probes only for any remaining ingress issue.
Check Docker and the existing SSH ControlMaster before further runtime work.
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
- [Public Go cache and console readiness repair](history/2026-10-14-m6-go-cache-and-console-readiness.md)
- [Ngrok auth-file RAM-mount repair](history/2026-10-15-ngrok-auth-tmpfs-repair.md)
- [Unauthenticated ingress health route](history/2026-10-15-public-ingress-health-route.md)
- [Permanent M6 zero-swap enforcement](history/2026-10-15-m6-permanent-zero-swap.md)
- [Approved guest and policy-aware grant repair](history/2026-10-15-m7-approved-guest-and-grant-controls.md)
