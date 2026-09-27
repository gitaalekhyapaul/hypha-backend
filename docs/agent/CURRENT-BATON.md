# Current Engineering Baton

- Baton ID: B-0066
- State: READY
- Milestone: M6, serial agent capability dispatch and SDK delivery
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-09-27T07:47:00Z
- Branch: `main`
- Worktree: B-0064/B-0065 backend and SDK source checkpoints committed and pushed.
- Execution: Paused by the maintainer after B-0065. Do not start without resume.

## Objective

Connect approved access requests to one-at-a-time proof dispatch, final agent
signatures, and reusable protected queries through the external SDK.

## Acceptance Criteria

- [ ] Two approved requests wait durably and dispatch one at a time.
- [ ] Reserve workflow capacity before invoking B-0065 private snapshot preparation.
- [ ] Publish the exact issuance proposal and accept the registered agent's signature.
- [ ] Retries, cancellation, deadlines, and restart recovery preserve request ownership.
- [ ] Polling and terminal results reach the SDK without private witness material.
- [ ] The external SDK completes a protected query and repeats it through the cache.
- [ ] Package consumer checks pass and public release prerequisites are resolved.
- [ ] The provisional VM deployment runbook matches the executable stack.

## Architecture Constraints

- Inherit approved ADR-0019 and ADR-0020. Do not change proof or commitment semantics.
- BullMQ remains the proof queue. Only one proof runs at a time.
- Final issuance signatures occur after capacity reservation and snapshot preparation.
- Independent policy publication does not wait through a proof.
- Agents receive no PolicyIR, tuples, recipes, salts, openings, or witness traces.
- Authentication and inbox approval alone never authorize protected data.
- The verifier never calls the issuer. Cold and cached paths check exact dataset bindings.

## File Scope

On resume, select one bounded slice within owner dispatch, issuance HTTP/RPC,
SDK polling/signing/protected queries, required Compose wiring, and focused tests.
Sibling writes remain subject to filesystem permissions. Do not edit VM source.

## Out Of Scope

- B-0067 agent dashboards, LLM adapters, and the shared message channel
- B-0068 three-dashboard VM demonstration and final evidence
- Parallel proving, new proof semantics, or implicit M5 state migration

## Decisions

- The maintainer requested completing B-0065, then pausing.
- Direct use of the existing Redis 1.5.0 pin and pinned SpiceDB download was approved.
- M6 remains opt-in and isolated from the default M5 runtime.
- The backend consumes the local B-0065 SDK candidate, not a public npm release.
- Maintainer authorized committing and pushing the current backend and SDK progress.
  This is a checkpoint operation, not a resume of B-0066. Npm publication and VM
  deployment remain outside this authorization.

## Open Questions And Blockers

- No unresolved decision blocks the completed B-0065 slice.
- Public npm release still needs authenticated publication and confirmed metadata.
- The local M6 preview has memory-backed SpiceDB. Durable deployment or explicit
  safe recovery is required before a VM demonstration.

## Work Completed

B-0064 delivered registration, tokens, signed transport, and the external SDK base.
B-0065 delivered agent-owned intents, operator approve/reject/cancel, independently
published immutable policy versions, exact dataset bindings, and request-triggered
signed snapshot preparation. Real SpiceDB preparation and exact retry reuse passed.

The browser stops at `APPROVED_WAITING`. Private preparation is callable and tested,
but no automatic dispatcher invokes it yet. No new proof ran in B-0065. M0-M5 remain
complete. M6 and M7 are not complete. All B-0065 workers finished and were closed.

See [B-0065 evidence](history/B-0065-access-inbox-and-policy-packages.md) and the
[access inbox runbook](../runbooks/AGENT-ACCESS-INBOX.md).

## Validation Plan

On resume, start with the narrow dispatch slice. Reuse existing queue, issuance,
signature, SDK, and snapshot checks. Add two-request serial progress and exact-retry
coverage. Run a real VM proof only at the connected provisional checkpoint.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0065 real preparation and retry | PASS | Real SpiceDB and signed snapshot, selected policy pinned, same descriptor reused, no proof | 2026-09-27T07:22:45Z |
| B-0065 focused checks | PASS | Admin, service, reader, Redis approval/preparation, console, types, Clippy, Compose; opt-in skips listed in archive | 2026-09-27 |
| B-0066 implementation and tests | SKIP | Not started, paused by maintainer | 2026-09-27 |
| VM and npm publication | SKIP | Not part of this checkpoint | 2026-09-27 |

## Commit Checkpoint

The maintainer requested a GitHub checkpoint after the B-0065 pause. Preserve the
archived implementation evidence. Check source, tests, generated public SDK files,
and the exact vendored SDK candidate. Exclude credentials, runtime data, build
outputs, and macOS metadata. No agent-demo source changes exist for this checkpoint.
GitHub checkpoint:

- Backend source: `8f4fa681dbe3aa10e550a5bec0bd0380dc33aae0`, pushed to `origin/main`.
- SDK source: `1d2d2720d7babd7b6b674609320f36b522dc9eb7`, pushed to `origin/main`.
- Earlier backend planning commits and formatter-only `d7095fd` are included.
- `raziel-demo-agents` has no source changes. SDK `.DS_Store` stays local and untracked.
- This follow-up records synchronization only. B-0066 remains READY, unassigned,
  and paused. The goal's paused status was verified after both pushes.

Checkpoint validation on 2026-09-27:

- PASS: backend TypeScript suites, 492 tests. Eighteen opt-in console tests skipped
  in this invocation. Their earlier live evidence remains in the B-0065 archive.
- PASS: all backend TypeScript typechecks, Biome lint, full format check, Buf lint,
  deterministic generation, and M6 Compose configuration.
- PASS: Rust canonical, authority, and admin tests, including protocol vectors.
  Four admin integration gates were ignored here and retain separate archived evidence.
- PASS: all Go compiler packages with the existing offline module/build cache.
- PASS: SDK 220 tests, package build, source/test typecheck, declared lint and format
  scripts. Typechecking used existing backend dependencies through a temporary
  container-only path mapping, with no dependency installation or SDK source change.
- PASS: scoped Rust Clippy for canonical, authority, admin, and access-only razield,
  with warnings denied.
- PASS: staged private-file and credential-pattern checks. Only public SDK release
  candidates are included. The selected tarball matches its recorded SHA-256.
- The full SDK whitespace check reports four generator-produced blank EOF lines.
  Handwritten sources pass. Generated files are kept byte-exact with their hashes.
  An extra combined Biome check reports import-order suggestions beyond the SDK's
  declared lint/format scripts. No SDK behavior was changed for those suggestions.
- Ten existing backend import-order violations were fixed mechanically in separate
  commit `d7095fd`. Three Rust formatting differences were corrected.
- An initial Go invocation used an empty cache and failed offline. The configured
  existing cache passed without downloading dependencies.
- Initial SDK validation mounts hid transitive Vitest types and failed. The temporary
  path mapping resolved the existing complete dependency tree and typechecking passed.
- Full proof, VM deployment, and the complete CI build matrix were not rerun.

## Risks And Private-Data Notes

- Exact approved policy, agent, dataset revision, and request scope must survive dispatch.
- Snapshot receipt recovery reuses private state, never a fresh salt on an exact retry.
- Keep authority keys, grant recipes, tuples, and snapshots in owner-only runtime storage.
- The preview at `http://127.0.0.1:3011/` supports datasets, registration, policy
  versions, and request review. It is not a full M6 demonstration.
- Do not restart memory-backed SpiceDB or delete its volumes to bypass drift checks.
- Local prebuilt debug tools passed. A fresh release build is not claimed here.
- Installed approval recovery works. Unfinished browser draft recovery remains deferred.

## Exact Next Action

Remain paused. After explicit maintainer resume, inspect this baton and the dirty
worktree, then claim the bounded serial-dispatch slice. Do not start B-0067.
