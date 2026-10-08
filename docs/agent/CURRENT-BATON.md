# Current Engineering Baton

- Baton ID: B-0067
- State: READY
- Milestone: M7, two-agent reconciliation dashboards
- Owner: unassigned
- Started UTC: 2026-10-01T06:38:26Z
- Last updated UTC: 2026-10-08T07:32:08Z
- Branch: `main`
- Worktree: release changes are checkpointed by the backend commit containing this
  file. SDK and agent commits below are already pushed and remote-verified.
- Execution: maintainer selected a Compose-managed ngrok tunnel with the account's
  fixed dev domain, portable between the Mac and VM. The bounded deployment follow-up
  is implemented and locally checked. No remote source or runtime edits.

## Objective

Next bounded objective: configure the private ngrok token and validate the signed
Agent A approval-to-data path on the VM. The fixed-domain Compose ingress and both
agent origin defaults are implemented. Preserve authentication, SDK TLS checks, and
private owner networks. One host serves the domain at a time. No SDK release or proof
change belongs to this deployment checkpoint.

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

## Open Questions And Blockers

- Private ngrok token configuration remains pending. The URL is known and the image
  is present locally. Keep the token in ignored backend settings, never chat or Git.
  Live cloud-policy acceptance and signed API validation require the private token.
- Live VM release build, API reachability, and real M6/M7 proof validation are unrun.
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
Agent archive cleanup and current head: `84f9eb135532cc52b89bbfbf20d77c1f85b12b7d`.
Both commits are pushed and match GitHub `origin/main`.
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

## Risks And Private-Data Notes

Keys, API tokens, SoC credentials, issuance attempts, capabilities, snapshots, and
private witnesses remain outside Git. Ignored runtime/cache/macOS files are local
artifacts, not pending source changes. No proof/public-value contract changed.
Real-data acceptance and a second scenario remain deferred and unclaimed.

## Exact Next Action

Have the maintainer pull both deployment checkpoints on the VM. Add the private
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
