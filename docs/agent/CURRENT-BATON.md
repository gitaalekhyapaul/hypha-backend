# Current Engineering Baton

- Baton ID: B-0067
- State: READY
- Milestone: M7, two-agent reconciliation dashboards
- Owner: unassigned
- Started UTC: 2026-10-01T06:38:26Z
- Last updated UTC: 2026-10-08T01:58:58Z
- Branch: `main`
- Worktree: release changes are checkpointed by the backend commit containing this
  file. SDK and agent commits below are already pushed and remote-verified.
- Execution: the authorized three-repository release checkpoint is complete.
  VM deployment and the broader M6/M7 demo stay paused until maintainer resumption.

## Objective

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
- Use the existing Mac ngrok client with an SSH-forwarded VM console for HTTPS API
  ingress. Keep dashboard ports SSH-forwarded. No paid subscription is required.
- Ask for Docker or SSH ControlMaster if unavailable. Never modify VM source directly.

## Open Questions And Blockers

- No feature or dependency decision is pending.
- Live VM release build, API reachability, and real M6/M7 proof validation are unrun.
- The last VM readiness check on October 1 found a refused SSH ControlMaster.
  Its current availability was not tested by this local release checkpoint.
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

## Validation Plan

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

## Risks And Private-Data Notes

Keys, API tokens, SoC credentials, issuance attempts, capabilities, snapshots, and
private witnesses remain outside Git. Ignored runtime/cache/macOS files are local
artifacts, not pending source changes. No proof/public-value contract changed.
Real-data acceptance and a second scenario remain deferred and unclaimed.

## Exact Next Action

Wait for maintainer resumption. Check Docker and SSH ControlMaster, then have the
maintainer pull the pushed backend and agent commits on the VM. Follow the M6/M7
runbooks for HTTPS ingress, startup, Agent A registration, inbox approval, exact
signing, real proof, and protected query. Continue to Agent B and reconciliation.
B-0068 retains the two-proof recording, negative controls, screenshots, and evidence.

## Dated Evidence

- [B-0066 archive](history/B-0066-serial-dispatch-and-sdk-delivery.md)
- [SDK publication and October 8 follow-up](history/2026-10-01-sdk-publication-attempt.md)
- [Agent migration and October 8 follow-up](history/2026-10-01-agent-public-sdk-consumption.md)
- [Preserved pre-release state](history/2026-10-08-b0067-pre-release-state.md)
