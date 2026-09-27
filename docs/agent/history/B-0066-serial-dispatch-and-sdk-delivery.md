# B-0066: Serial Dispatch And SDK Delivery

## Final Checkpoint

Implementation checkpoint complete on 2026-09-27. Live M6 VM proof validation and
public npm publication remain pending. The maintainer deferred npm publication
after confirming the packaged candidate suffices for provisional VM testing.
The maintainer requested Git commits/pushes, a progress report, then a pause.

Delivered: serial BullMQ dispatch, durable capacity reservation, exact agent signing,
proof-service handoff with uncertain-submit recovery, retained terminal results,
SDK resume/sign/submit/poll/cancel, and dual-signature protected-query reuse.
The packaged consumer passes cold/cache queries and revoked-token rejection with
a controlled proof verifier. No production mock flag or proof semantics changed.

The new `docker-compose.m6-live.yml` connects production owner, dispatcher, proof,
and independent verifier roles. ADR-0021 records the approved dedicated PostgreSQL
17.10 datastore. The new project imports no M5 or preview state. The proof keeps
eight CPUs, 56 GiB, and no swap. The verifier keeps its 2 GiB networkless child.
Only console port 3000 is exposed, on loopback by default. See
[M6 Live VM](../../runbooks/M6-LIVE-VM.md) for preparation and startup.

Final validation:

| Check | Result | Scope |
| --- | --- | --- |
| Rust dispatch/ownership | PASS | 60 library, 11 service integration, 8 admission tests; real Redis/BullMQ, controlled proof delivery |
| Rust compatibility | PASS | Scoped all-target Clippy with warnings denied; default capability plus dispatcher check |
| SDK | PASS | 249 tests, source/test types, ESM/declaration build, lint and format |
| Packaged query consumer | PASS | 35 focused console checks, including cold/cache signatures, distinct nonces/receipts, and revocation |
| Console regression | PASS | 457 tests, 18 opt-in checks skipped |
| Gateway regression | PASS | 27 tests |
| Compatibility wrapper | PASS | 6 tests and build |
| Static checks | PASS | RPC generation, Buf lint, console types, workspace format, changed-file lint, shell syntax, whitespace |
| Compose | PASS | Existing preview and new live stack, all profiles, isolated volumes and private ports |
| PostgreSQL persistence | PASS | Pinned migration; synthetic permission before and after database and SpiceDB restart |
| Candidate installation | PASS | Frozen local tarball with registry security metadata checks, no third-party pin changes |
| New VM release build and real proof | SKIP | VM runbook supplied, not executed in this session |
| Public npm release | SKIP | Deferred by maintainer |

SDK source commit: `e7f1cd6b354f236f0323d9d19aa4a431c5dfa470`.
Candidate: `packages/sdk-ts/vendor/gitaalekhyapaul-raziel-sdk-ts-0.1.0-b0066.tgz`.
SHA-256: `556abe87978429a1542bfbbe8d81edd48a8be92ceeb07b1731eb18e8233ce572`.
The 40-file package contains ESM, declarations, public DTOs, README, metadata, and
license. No credentials, private state, or tests are packaged. Public npm release
metadata is ready but authentication and execution are deferred.

All original B-0066 acceptance items pass at the implementation/component level.
The public-release item was explicitly deferred. Live connected proof acceptance
is not replaced by controlled tests and remains required before M6 completion.

Final corrections: one Rust import needed formatting. pnpm still requested registry
security metadata with offline archive installation. Its networkless invocation was
stopped and the approved metadata check passed. No package policy was disabled.
The new synthetic-only persistence containers and database volume were removed with
approval. No existing preview, M5, or unrelated project services were stopped.

Privacy review: agent outputs contain no PolicyIR, tuples, Merkle openings, or stored
caveat context. The verifier has no issuer endpoint or private snapshot mount.
New live issuer-offline verification is not claimed. No VM command or npm publication
ran. No agent-demo repository source changed. SDK `.DS_Store` remains untracked.

## Dated Implementation Log

The following notes retain the earlier work-in-progress evidence. The final
checkpoint above supersedes their pending-work and uncommitted-state descriptions.

- Baton ID: B-0066
- State: ACTIVE
- Milestone: M6, serial agent capability dispatch and SDK delivery
- Owner: Codex coordinator
- Started UTC: 2026-09-27T07:56:28Z
- Last updated UTC: 2026-09-27T09:59:00Z
- Branch: `main`
- Worktree: B-0064/B-0065 checkpoints pushed. B-0066 backend and SDK changes are uncommitted.
- Execution: Resumed by the maintainer on 2026-09-27. Docker is available.
- Stop instruction: finish B-0066, report progress, commit and push each changed
  repository to its own remote, then pause the goal until explicit resume.

## Objective

Connect approved access requests to one-at-a-time proof dispatch, final agent
signatures, and reusable protected queries through the external SDK.

## Acceptance Criteria

- [ ] Two approved requests wait durably and dispatch one at a time.
- [x] Reserve workflow capacity before invoking B-0065 private snapshot preparation.
- [x] Publish the exact issuance proposal and accept the registered agent's signature.
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

Current slice: SDK polling and protected-query delivery pass local checks. Package
the candidate and validate its backend consumer, then wire deployment.
Coordinator owns these paths:

- `services/razield/src/access_request/`, `access_dispatch/`, `request_preparation.rs`,
  queue/module feature gates, the dispatcher binary, and relevant Cargo targets.
- `proto/raziel/v1/access_service.proto`, `agent_api.proto`, and the admin access RPC client.
- Console access-request/resume routes, `lib/agent-query/`, the fixed protected-query
  route, public reconciliation response projection, and focused route tests.
- Architecture, technical/API specification checkpoints, inbox runbook, and this baton.
- README and M6-M7 plan status: remove stale paused-state claims after this resume.
- Sibling `raziel-sdk-ts`: access client, signer-based issuance helper, focused tests,
  public DTO export, generated agent API contract, README, and compatibility metadata.
  Sibling writes require filesystem approval. No npm publication is authorized here.
- `packages/sdk-ts` compatibility package and candidate archive, workspace lockfile,
  and focused packaged-consumer tests. No new third-party dependency pins.
- Opt-in live M6 Compose overlay, build/start helpers, durable datastore wiring,
  deployment checks, and the provisional VM runbook.

Both delegated tasks hit the workspace spending cap before doing work and were
closed. The coordinator owns all edits. VM source remains untouched.

## Out Of Scope

- B-0067 agent dashboards, LLM adapters, and the shared message channel
- B-0068 three-dashboard VM demonstration and final evidence
- Parallel proving, new proof semantics, or implicit M5 state migration

## Decisions

- The maintainer requested completing B-0065, then pausing.
- The maintainer explicitly resumed B-0066. The backend was clean at `abff0c8`.
- Security review scope: ADR-0020 already approves serial dispatch and the dual-signature
  query facade. Preserve exact request, key, target, and dataset binding checks.
- Signature acceptance reuses Rust capability admission and canonical commitments.
  Persist the immutable accepted attempt before public submission state. Recover that
  write before signature timeout or cancellation handling. Do not mint replacement nonces.
- The handoff slice stores bounded public terminal ProofJob bytes in a separate Redis
  hash, atomically with request termination and capacity release. Private comparison
  input remains in owner storage. It adds no queue implementation or proof evaluator.
- The dispatcher records submission start before the RPC. A missing job after an
  uncertain start cannot confirm cancellation. Only exact signed bytes can be resent.
- Direct use of the existing Redis 1.5.0 pin and pinned SpiceDB download was approved.
- M6 remains opt-in and isolated from the default M5 runtime.
- The maintainer approved dedicated PostgreSQL 17.10 for M6 SpiceDB on 2026-09-27,
  pinned at `postgres@sha256:9b18b78397054fce88a9552e9d5a3ad5bb7fd258c5b3cc1c5028e46373d6ea8f`.
  Use a new project-local volume and internal network. Do not migrate the preview.
- The backend consumes the local B-0066 SDK candidate, not a public npm release.
- The earlier maintainer GitHub checkpoint covered B-0064/B-0065 source. The later
  explicit B-0066 resume supersedes its paused execution state. No new commit, push,
  npm publication, or VM deployment occurred in this implementation session.

## Open Questions And Blockers

- No unresolved decision blocks the completed B-0065 slice.
- The maintainer will change the primary project folder after B-0066. Continue
  using scoped write approvals for sibling edits until then.
- Public npm release still needs authenticated publication and confirmed metadata.
- The maintainer has attached all three repositories to the app project and requests
  project-scoped persistent write access. This task's supplied writable roots still
  list only the backend. SDK edits used approved per-command escalation. No tool for
  a persistent directory grant is exposed. Recheck effective roots on the next turn,
  do not assume the project screenshot changes this running sandbox or request full access.
- The local M6 preview has memory-backed SpiceDB. Durable deployment or explicit
  safe recovery is required before a VM demonstration.

## Work Completed

B-0064 delivered registration, tokens, signed transport, and the external SDK base.
B-0065 delivered agent-owned intents, operator approve/reject/cancel, independently
published immutable policy versions, exact dataset bindings, and request-triggered
signed snapshot preparation. Real SpiceDB preparation and exact retry reuse passed.

The existing preview still stops at `APPROVED_WAITING`. Its deployment has not changed.
No new proof ran in B-0065 or this B-0066 slice. M0-M5 remain complete. M6 and M7 are
not complete. All B-0065 workers finished and were closed.

B-0066 progress:

- Added `razield-access-dispatch`, using the existing BullMQ dependency. Queue
  references contain only agent ID, request ID, and generation. Concurrency is one.
- Added durable reservation before private preparation, exact public proposals,
  120-second signer timeout, explicit resume, and preparation cancellation fencing.
- Restart reconciliation retains an existing proposal. A prepared public descriptor
  survives transient publication retries within the process. No private witness is cached.
- Added the internal resume RPC/CLI and agent-authenticated HTTP route.
- Added the fixed protected-query facade. Outer API authentication and inner key
  identity must agree. Exact envelope bytes pass to the existing gateway. Tokens and
  caller headers do not. No raw dataset route is exposed.
- Public receipt projection now accepts the exact M6 namespaced resource type.
  Existing M5 callers retain the `dataset` default.
- Added `AccessIssuanceSubmission`, owner-only submit RPC, Rust admin transport,
  and the authenticated issuance HTTP route. The registered key must match both
  the original intent and signed capability request. Rust reuses existing admission.
- Accepted attempts persist an immutable owner-only comparator before `SUBMITTING`.
  It retains proposal identity, acceptance time, nonce, signature, and deterministic
  job ID. All other fields reconstruct from the original intent and proposal.
  Private context is not duplicated or placed in Redis.
- Recovery reconciles the durable acceptance before signer timeout or cancellation.
  Exact retries return the retained result after the original signing window.
  Changed bodies cannot create a second attempt. Missing comparison state fails closed.
- Cancellation after signature acceptance records `CANCELLING`, not presumed proof
  termination. The dispatcher now calls the existing CapabilityService through Tonic.
- Submission start persists before create. Recovery reads the deterministic job first,
  and only resends the exact retained request. Cancellation blocks resends. Unknown
  publication cannot be treated as absent or release workflow capacity.
- Terminal ProofJob delivery checks exact owner, commitment, scope, policy, snapshot,
  expiry, metrics, closed states, and response bounds. Public results commit atomically
  with slot release and remain available after proof-queue retention ends.
- Detail reads and exact issuance retries return retained capability bytes. Lists omit
  those bytes. Seven-day request cleanup deletes the result and private comparator.
- The sibling SDK now exposes explicit resume, signIssuance, and submitIssuance.
  Proposal validation checks caller intent, registered key, scope, policy binding,
  snapshot root/ID, generation, and deadline. Signing reuses the existing canonical
  commitment and caller-owned signer, with bounded waits and AbortSignal support.
- Signing makes no HTTP request. The caller retains its returned attempt before an
  explicit submit. Retries preserve that body and refresh outer API authentication.
  The public generated DTO and compatibility source hash were refreshed. Existing
  backend vendored tarballs remain unchanged until packaged consumer validation.

Still incomplete: connected real-service dispatch, SDK workflow/query delivery,
publication prerequisites, and VM deployment plumbing.
The new dispatcher is not yet composed or started in the preview.

See [B-0065 evidence](history/B-0065-access-inbox-and-policy-packages.md) and the
[access inbox runbook](../runbooks/AGENT-ACCESS-INBOX.md).

## Validation Plan

Reuse existing queue, issuance, signature, SDK, and snapshot checks. Current checks
cover two-request serial preparation, exact proposals, restart retention, timeout,
cancel fencing, resume, and the external query facade. Controlled snapshot preparation
does not count as proof execution. Run a real VM proof at the connected checkpoint.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0065 real preparation and retry | PASS | Real SpiceDB and signed snapshot, selected policy pinned, same descriptor reused, no proof | 2026-09-27T07:22:45Z |
| B-0065 focused checks | PASS | Admin, service, reader, Redis approval/preparation, console, types, Clippy, Compose; opt-in skips listed in archive | 2026-09-27 |
| B-0066 Rust slice | PASS | 54 library, 11 access-service integration, and 8 existing admission tests; real Redis/BullMQ and Ed25519, controlled preparation, no proof; two unrelated live SpiceDB checks filtered | 2026-09-27T09:06:26Z |
| B-0066 proof handoff | PASS | 60 library, 11 access-service integration, 8 admission tests; controlled proof-service delivery, real Redis/BullMQ; two live SpiceDB checks filtered; no SP1 proof | 2026-09-27T09:29:00Z |
| Handoff feature compatibility | PASS | Access-service and access-dispatch all-target Clippy with warnings denied; default capability-service plus dispatcher cargo check | 2026-09-27T09:31:00Z |
| SDK signing/submission | PASS | 234 tests, full source/test typecheck, ESM/declaration build; temporary source copy, existing pinned Docker dependencies, controlled HTTP, no install or proof | 2026-09-27T09:58:00Z |
| B-0066 HTTP slice | PASS | 34 console tests across signature submission, resume, protected facade, and existing reconciliation route | 2026-09-27T09:06:11Z |
| Gateway regression | PASS | 20 protected-query and dataset-binding checks, including cold/cache enforcement | 2026-09-27T08:47:27Z |
| B-0066 static checks | PASS | RPC generation, proto build, Buf lint, console typecheck, changed-file Biome check, access-dispatch all-target Clippy with warnings denied | 2026-09-27T09:08:21Z |
| Capability feature compatibility | PASS | `cargo check --offline --locked -p razield --features access-dispatch`, default capability feature retained | 2026-09-27T09:07:25Z |
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
- At that checkpoint B-0066 was READY and paused. The current ACTIVE state records
  the subsequent explicit maintainer resume.

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

Current-session corrections and limits:

- Initial access-only test compilation exposed capability-specific test targets
  without feature gates. Their required features now match the existing module gates.
- The default toolchain network could not resolve the owner Redis service. The
  local Redis tests now join the existing `raziel_owner-internal` network explicitly.
- Resume HTTP initially read stale built Protobuf exports. Rebuilding `proto-ts`
  resolved that failure. One Fetch body type mismatch was corrected without casts.
- An overly broad `--include-ignored` run invoked two SpiceDB-only checks without
  their explicit environment and failed. They are not claimed as rerun here.
- A subsequent worker check exposed repeated preparation after a transient public
  record lock failure. The processor now retains the prepared descriptor for retry.
  The final scoped library and access-service checks pass.
- A handoff regression initially let cancel return an operator-rejected request.
  Restored the existing conflict response for requests with no proof attempt. Late
  cancellation of proof-bound terminal requests returns their retained outcome.
  The full scoped rerun passed. Access-only feature warnings were corrected too.
- The first SDK test container could not create a cache mount under read-only source.
  Tests now use a temporary source copy. SDK writes remain confined to individually
  approved source edits. The final signing-wait edit still needs formatter application.

Reproducible scoped validation in `raziel-toolchain:m0`:

- Rust: `cargo test --offline --locked -p razield --no-default-features --features
  access-dispatch --lib --test access_request -- --include-ignored --test-threads=1
  --skip live_request_snapshot_recovers_after_private_receipt_before_registry_append
  --skip live_complete_schema_comparison_uses_the_historical_revision`. Use the
  existing owner network and explicit test Redis URL, never a production namespace.
- Rust lint: `cargo clippy --offline --locked -p razield --no-default-features
  --features access-dispatch --all-targets -- -D warnings`.
- Console: `pnpm --filter @raziel/demo-console exec vitest run
  test/agent-query.test.ts test/access-requests-http.test.ts test/reconciliation-route.test.ts`.
- Gateway: `pnpm --filter @raziel/demo-gateway exec vitest run
  test/protected-query.test.ts test/dataset-binding.test.ts`.
- Generated internal RPC files remain build outputs. The sibling SDK's public agent
  API file now matches backend SHA-256 `042104a2908797c20c5d8992a4029644110b9107ea7f683ec9cc006bbda87ea0`.
  Package tarballs and dependency versions did not change. Refresh the candidate only
  after polling/query implementation and packaged consumer validation.
- Existing admission vectors ran with `--test admission` in the Rust invocation.
- Focused acceptance checks cover all proposal fields, private context mismatch,
  invalid signatures, late submission, changed nonce retry, persisted-write recovery,
  owner-only file permissions, cancellation fencing, and missing comparison state.
- PASS: `cargo fmt --all`, changed TypeScript formatting/lint, and `git diff --check`.

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

Package the SDK candidate and run backend consumer checks, then complete deployment
wiring. SDK polling/query implementation passes 249 tests, source/test typechecking,
declaration build and Biome lint. The new malformed timestamp test fixture was fixed
without changing application semantics. The SDK formatter has been applied.
Keep connected proof execution and deployment as separate unclaimed gates until
validated. Do not start B-0067 or a real local proof. After B-0066 passes, commit and
push the changed repositories, report the outcome, and pause the goal. Public npm
publication remains a separate step.
