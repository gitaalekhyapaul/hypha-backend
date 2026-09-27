# B-0065: Access Inbox And Immutable Policy Packages

- Baton ID: B-0065
- State: Archived, completed locally
- Milestone: M6, access inbox and immutable policy packages
- Owner: Codex
- Started UTC: 2026-09-26T20:29:05Z
- Completed UTC: 2026-09-27T07:24:00Z
- Branch: `main`
- Worktree: B-0064 and B-0065 changes remain uncommitted. No push, npm release, or VM deployment.

## Objective

Implement agent-owned access intents and operator review, with immutable policy
packages and request-triggered preparation under approved ADR-0020.

## Acceptance Criteria

- [x] Registered agents can create and inspect only their own access requests.
- [x] Operator can inspect, approve, or reject a request through the dashboard.
- [x] Approvals pin exact agent, task, purpose, finalized dataset, and policy version.
- [x] Publishing another policy version does not invalidate a pinned earlier version.
- [x] Immutable owner packages and composed SpiceDB schema keep private data server-side.
- [x] Approved request preparation produces a version-pinned snapshot proposal.
- [x] SDK exposes typed access-intent methods through the external API.
- [x] Focused tests and docs match these user flows.

## Completion Boundary

The connected portal approves a real SDK-created request against an installed policy
and finalized dataset. Private owner preparation provisions the selected grant and
produces an accepted signed snapshot at the exact SpiceDB revision. Exact retries
reuse that snapshot. Two policy versions remain installed. The public request stays
`APPROVED_WAITING`, without a capability or private recipe.

B-0066 owns serial capacity reservation, automatic preparation, final agent signatures,
proof dispatch, and protected-query integration. Those operations did not start here.
The maintainer requested a pause after B-0065, so the next baton is READY and unclaimed.
No proof generation or VM operation ran in this closing slice. All workers stopped.

## Architecture Constraints

- ADR-0019 and ADR-0020 are approved. No new proof semantics or capability format.
- Keep the one-proof-at-a-time limit. B-0066 owns durable dispatch and late signatures.
- Policy publication must not wait for the duration of an active proof.
- Snapshot formation follows an approved agent request, not policy publication alone.
- Agent authentication does not authorize protected data without a verified capability.
- No PolicyIR, relationship tuples, salts, openings, or grant recipe reaches an agent.

## File Scope

- Console agent/operator access routes, lifecycle store, request inbox and policy builder
- Owner policy-version storage, compiler naming/composed schema and registry publication
- Request preparation and snapshot-owner integration
- Protobuf contracts, generated clients, sibling SDK methods and packaged candidate
- Focused affected tests, deployment configuration and current documentation

Read active sources before choosing exact file ownership. Claim one bounded slice.
No remote edits, push, npm release, or unapproved dependency installation.

Current implementation split: coordinator owns Protobuf, HTTP facade, owner-client
integration, runtime wiring, and this baton. Disjoint workers may own the Rust access
lifecycle module, operator inbox component/client, and namespaced policy rendering.
Those paths are connected and their local acceptance criteria passed.

Final bounded slice: request-triggered private preparation. Coordinator owned the
approved-input accessor, preparation composition, integration, docs, and this baton.
Disjoint workers owned namespaced grant construction and durable SpiceDB handoff,
and exact-revision aggregate checks in the snapshot authority. The UI approval
slice passed its real HTTP/browser gate. No automatic dispatcher or proof starts.
No guest or commitment semantics change.

Previous bounded slice completed: installed-policy catalogue and immutable dataset
binding primitives. Coordinator implemented the binding store, gateway enforcement,
HTTP integration, deployment wiring, and this baton. Disjoint workers completed the
owner catalogue/RPC and connected catalogue UI, then closed. This earlier slice
preceded persisted request approval and aggregate-aware snapshot preparation. No commitment
or guest semantics changed, and no proof ran in this slice.

The composed browser gate found allocation-store initialization attempted a UID
probe on the publisher's read-only mount. This slice adds a read-only store open
path and preserves the mount boundary. Allocation writes remain in the console.

## Out Of Scope

- B-0066 serial dispatch, final issuance exchange, protected-query facade and npm release
- B-0067 agent dashboards, LLM tools, ledgers and message channel
- B-0068 NUS VM demonstration and evidence
- New proof format, parallel proving, private client-side policy evaluation

## Decisions

- Maintainer requested completing B-0065, then pausing and reporting the result.
  Do not start B-0066 after this baton closes.
- User approved ADR-0020 on 2026-09-25.
- B-0064 registered-key authentication and packaged SDK are locally implemented.
- SDK source owns signing and transport. Backend `packages/sdk-ts` only re-exports.
- The B-0065 SDK patch was applied to the sibling after scoped filesystem approval.
  Backend consumers now select its tested local tarball, not an npm publication.
- Console uses its Node HTTP entry point for raw target/header checks.
- Keep UI feature completion ahead of unrelated test or infrastructure expansion.
- Maintainer approved the pinned SpiceDB 1.54.0 image download. Its isolated local
  publication gate passed. The test container was stopped and removed afterward.

## Open Questions And Blockers

- Maintainer approved direct use of the already locked Rust `redis =1.5.0` pin.
  No dependency approval blocks this slice.
- Npm authentication and LLM deployment credentials are later release inputs.
- Sibling SDK writes were approved for B-0064. Respect filesystem gates for later edits.

## Work Completed

Implemented the generated access-intent contract, owner-only RPC, bounded Rust CLI,
signed HTTP create/list/get/cancel facade, and operator list/detail/reject/cancel.
The Rust access role owns Redis generation checks, admission limits, private intent
comparison, terminal retention, and cancellation before dispatch. Lookup-only exact
retries do not require dataset service availability. No proof runs in this role.

The connected operator inbox supports current-page pending/all views, agent filter,
pagination, safe details, rejection, cancellation, and conflict refresh. Browser
sign-in and actual stored terminal records passed at the local preview.

Typed namespaced rendering is implemented for all supported rule families. The
owner-only version-preparation API now calls the Rust allocator and pinned compiler.
Opaque family/version IDs and numeric versions are durable, not caller-selected.
Exact retries reuse the same allocation. Separate private allocation storage survives
candidate cleanup. Failed drafts do not recycle version numbers.

The actual Rust CLI plus pinned Go compiler prepared two versions in one family,
preserved the first source, and passed retry checks. Operator HTTP routes enforce
session/CSRF checks and return identity plus the safe candidate projection. The
visual builder now selects the versioned flow in the opt-in M6 runtime. Legacy activation rejects the reserved `family:`
namespace. Preparation is not approval. New versions have a separate publish endpoint.

Added `tools/raziel-admin/src/policy_package/` with immutable owner-only retention,
exact source/config/PolicyIR validation, namespace ownership, registry approval
resolution, version coexistence, and explicit reapproval handling. The composed owner
publisher now calls it after authority approval and registry publication. Historical
retained reads support schema composition without treating expired or revoked records
as authorization. Dataset binding primitives, their request-approval writer, and
aggregate-aware snapshot provenance are implemented and locally checked.

Added the composed owner publication role and authenticated operator publish route.
The owner locks the short publication operation, validates exact retained artifacts,
checks live schema drift, obtains approval, retains the package, and installs the
aggregate. Durable pending state reconciles a lost schema-write response against the
recorded old or proposed schema. Unexpected drift fails without overwriting it.
The route returns only safe identity and approval projections, with exact-field checks.
The existing M5 role remains the default. The separate `docker-compose.m6.yml`
runtime removes legacy schema, snapshot, proof, and verifier services entirely.
Server mode guards reject legacy mutations in M6 and version publication in M5.
Project-scoped volumes, `.raziel/m6`, and fresh authority keys separate runtime state.
The console has a loopback host port. SpiceDB, Redis, and owner RPCs stay private.

The visual builder reuses all five supported rule families. It compiles server-assigned
versions, publishes exact candidates, retains exact retry requests, and starts a next
version in the same family. It never switches into legacy graph activation. The real
browser published direct version 1 and alternative-paths version 2 in one family.
The owner retained two installed packages with no pending write. The publisher's
read-only allocation mount exposed a constructor write probe, fixed by an explicit
read-only store open. Unit coverage rejects allocation through that reader.

Applied SDK create/list/get/cancel methods to sibling source after scoped approval.
The backend consumes `gitaalekhyapaul-raziel-sdk-ts-0.1.0-b0065.tgz`, SHA-256
`0ea89506eb120abd09befe9b45dd3b882374adc3926a00d43c95e2848f9f495e`.
The packaged client passed the real owner-service gate. Its HTTP transport is
in-process for this gate. Dataset lookup and operator admission are fixtures.

The installed-approval catalogue now passes through the existing owner socket and an
operator-authenticated, uncached HTTP route. It validates retained artifacts and signed
registry history, excludes inactive approvals, and refuses pending publication state.
The console receives only identifiers, hashes, entrypoint, epoch, and validity. The
Policy panel lists installed approvals, refreshes after publication, and selects a
family without manual ID copying. The real browser recovered both versions after a
reload and selected their family. The console has no package/registry volume mounts.

Added an owner-only immutable dataset-binding store. Exact retries keep a record,
but rebinding its policy/type/scope to a different dataset/revision/checksum fails.
The M6 gateway requires the exact owner mapping on verifier and cache paths, then
checks revision and recomputed dataset checksum within the same dataset read. The
M6 binding volume is read-only at the gateway. The access owner mounts it for approval writes.
M5 retains its legacy routing in the separate default runtime. Explicit legacy
binding migration and connected M6 dispatch remain unclaimed.

Operator approval now persists the exact private command before Redis publication,
pins trusted finalized metadata and the installed policy, and writes the immutable
dataset binding. Exact retries use retained input, never fresh mutable selection.
The real browser approved an SDK-created request against installed v1 after v2 had
been published. The SDK observed APPROVED_WAITING generation 2 with no proposal or
capability, and its exact create retry returned the same request.

Request-triggered preparation now materializes namespaced grants under the short
publication lock, retaining the exact accepted composition and SpiceDB revision.
Snapshot extraction compares the complete schema at that revision, then reads only
the selected package's definitions. Its owner-private receipt precedes registry
publication. Recovery reuses the original tree and descriptor. Generation and
cancellation checks fence preparation before and after network operations.
The connected real-service gate and exact retry passed. B-0066 owns dispatch and
final signatures. No B-0065 acceptance work remains.

Snapshot registry confirmation now accepts a newer unrelated validated generation
with the exact committed descriptor/root and still-approved bound policy. Stale
writer generations retry at most three times without rebuilding the private snapshot.
Revocation confirmation preserves the same exact-record check.

The offline Go composer now recompiles each exact source/config against expected
binary PolicyIR, validates namespace ownership, and checks aggregate compiler
structures. It supports at most 32 namespaced versions plus one legacy package.
Only an empty `agent` definition can be shared. Output schema and private compiler
reflection are bounded and written as `0600` files. It uses the maintained parser's
definition spans, with no new dependency. It does not check approval signatures or
install the schema itself. The owner coordinator performs those operations. The new
`compare-schema` command compares compiled definitions without depending on reflected
formatting or comments. Both contracts are documented in the compiler README.

The allocator, snapshot, composer, retained-read, compiled-comparison, publication,
runtime, and UI workers completed and were closed. The final grant, snapshot, and
approval workers completed and were closed. Coordinator recorded the closing evidence.

B-0064 is archived in
[agent authentication and SDK](B-0064-agent-authentication-and-sdk.md).
Delivered registration/token UI, strict Rust helper, Redis nonce/revocation admission,
signed `/me`, SDK extraction and client, Compose wiring, and user runbook.

No backend/SDK commit or push has been made for B-0064. The VM is unchanged.

## Validation Plan

Add focused access-owner isolation, idempotency, approval generation, immutable
policy-version coexistence, and request preparation tests. Reuse existing compiler,
registry, M5 UI, and snapshot tests. No new proof benchmark is required for this slice.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Real request preparation, final rebuilt binary | PASS | Real SpiceDB, selected installed v1 with v2 present, signed snapshot, exact retry reuses descriptor, public state unchanged, no proof | 2026-09-27T07:22:45Z |
| Admin library | PASS | 117 tests, 4 explicit integration gates ignored in this invocation | 2026-09-27T07:16:00Z |
| Snapshot and service library | PASS | 28 tests, 1 opt-in live recovery gate ignored in parent invocation | 2026-09-27T07:20:00Z |
| SpiceDB reader library | PASS | 9 tests, 1 opt-in historical revision gate ignored in parent invocation | 2026-09-27T07:22:00Z |
| Access approval and preparation with Redis | PASS | 11 tests, includes current generation and cancellation fencing; explicit test URL | 2026-09-27T07:21:20Z |
| Snapshot worker live checks | PASS | 13 snapshot tests including interrupted-append recovery; 9 reader tests and live historical-schema comparison; worker report | 2026-09-27T07:22:00Z |
| Access-only build and Clippy | PASS | No SP1 feature, preparation integration target included, warnings denied | 2026-09-27T07:21:00Z |
| Console approval/client checks | PASS | 138 tests and console types rerun; earlier connected browser and SDK gate below | 2026-09-27T07:12:00Z |
| M6 Compose and whitespace | PASS | Apps/all profiles, ports 3000/3011, final diff check | 2026-09-27T07:22:23Z |
| Approval HTTP and operator client | PASS | 138 focused tests, console types, Buf generation/lint, scoped Biome | 2026-09-27T06:50:00Z |
| Approval owner lifecycle | PASS | 10 approval/configuration checks and 11 inbox checks including live Redis; access-only build and Clippy (worker result) | 2026-09-27T06:56:00Z |
| Installed approval validation | PASS | 13 approval checks and 6 catalogue checks; Clippy and formatting (worker result) | 2026-09-27T06:50:00Z |
| Connected browser and packaged SDK approval | PASS | Real finalized dataset, installed v1, HTTP/RPC/Redis, generation 2, exact retry, no public private fields, no proof | 2026-09-27T06:59:52Z |
| Initial connected poll | FAIL | Intentional access-role replacement interrupted polling with 503; fresh synthetic run passed after deployment | 2026-09-27T06:58:00Z |
| B-0064 Console suite | PASS | 127 tests, including real SDK/Rust/Redis HTTP gate | 2026-09-26T19:09:08Z |
| B-0064 SDK candidate | PASS | 133 tests, package/build/type/vector checks | 2026-09-26 |
| B-0064 Rust and compatibility gates | PASS | Exact commands/results summarized in archive | 2026-09-26 |
| B-0064 final integrated browser recheck | NOT AVAILABLE | In-app browser connection error, host HTTP passed | 2026-09-26 |
| Rust access service | PASS | 11 tests, 10 live Redis/Tonic checks; Clippy and format (worker result) | 2026-09-26 |
| Linux access-only build | PASS | `razield-access` with `access-service`, no SP1 feature | 2026-09-26 |
| Owner CLI tests | PASS | 2 focused tests | 2026-09-26 |
| HTTP facade and operator client | PASS | 81 focused tests | 2026-09-26T20:50:00Z |
| Live request integration | PASS | Real auth helper, Redis, CLI/RPC; dataset and operator admission doubles | 2026-09-26T20:50:54Z |
| Proto generation, package build and Buf lint | PASS | Final RPC names follow standard request/response naming | 2026-09-26 |
| Namespaced policy render | PASS | 123 tests including 15 pinned compiler cases (worker result) | 2026-09-26 |
| Console production build | PASS | Includes the new agent and operator routes | 2026-09-26T20:52:00Z |
| Connected desktop inbox | PASS | Actual owner-service records and details, in-app browser | 2026-09-26T20:55:00Z |
| Console suite | PASS | 296 tests; two explicit live gates SKIP in this invocation | 2026-09-26T21:00:12Z |
| Admin library | PASS | 49 tests including 12 immutable-package cases (worker result) | 2026-09-26 |
| SDK staging and package checks | PASS | 220 tests, build/types, public exports, package consumer (worker result) | 2026-09-26T21:02:50Z |
| Packaged SDK/owner integration | PASS | Real signing/Rust/Redis/RPC with in-process HTTP transport | 2026-09-27T04:52:18Z |
| Backend SDK compatibility | PASS | 6 existing consumer tests against the B-0065 tarball | 2026-09-27T04:52:41Z |
| Final console types and diff checks | PASS | Updated packaged imports; backend and sibling tracked diffs | 2026-09-27T04:54:00Z |
| Offline dependency refresh | FAIL | Cached pnpm registry metadata missing; no dependency pin changed | 2026-09-27T04:50:00Z |
| Lockfile refresh then frozen offline install | PASS | Local SDK candidate and existing protobuf 2.12.1 runtime | 2026-09-27T04:52:00Z |
| VM, proof generation, npm release | SKIP | Not part of this local inbox validation | 2026-09-26 |
| `cargo check --offline -p razield --no-default-features --features access-service` | PASS | Pinned Docker; approved Redis 1.5.0 direct dependency | 2026-09-27T05:06:52Z |
| Policy candidate and HTTP route tests | PASS | 17 checks including allocation facade, legacy compatibility, and authenticated new routes | 2026-09-27T05:10:27Z |
| Real allocator/compiler integration and render cases | PASS | 90 checks including 15 pinned compiler families and one actual two-version flow | 2026-09-27T05:08:42Z |
| `cargo test --offline --locked -p raziel-admin --lib` | PASS | 64 tests, including 14 allocation checks and legacy namespace rejection | 2026-09-27T05:10:54Z |
| Snapshot runtime and handler tests | PASS | 10 mutation tests and 3 handler tests; Clippy passed (worker result) | 2026-09-27T05:08:00Z |
| Console types and production build | PASS | New version routes compile, no new UI activation claim | 2026-09-27T05:11:05Z |
| Scoped Biome, Compose config, shell syntax, diff checks | PASS | Edited integration and deployment preparation files | 2026-09-27T05:11:17Z |
| `go test -mod=readonly ./internal/composition ./internal/cli -count=1` | PASS | Pinned Docker with `GOPROXY=off`, `GOSUMDB=off`; includes 19 new top-level tests | 2026-09-27T05:22:28Z |
| Retained package history | PASS | 19 package tests; Clippy and formatting (worker result) | 2026-09-27 |
| Compiled schema comparison | PASS | 8 new top-level Go tests plus focused composition/CLI suite, offline (worker result) | 2026-09-27 |
| Publication HTTP and legacy activation | PASS | 11 focused tests; safe projections, exact identity checks, separate socket, authentication | 2026-09-27T05:36:55Z |
| Console types and production build | PASS | Includes version publish endpoint, not a connected builder action | 2026-09-27T05:42:15Z |
| Actual SpiceDB publication | PASS | Real compiler, authority, registry writer, pinned SpiceDB; two versions, old approval retained, exact retry | 2026-09-27T05:43:38Z |
| `cargo test --offline --locked -p raziel-admin --lib policy_publication::tests -- --ignored` | PASS | 3 controlled integration tests; drift and lost-response fault injection use a schema-service substitute | 2026-09-27T05:46:00Z |
| `cargo test --offline --locked -p raziel-admin --lib` | PASS | 71 passed, 3 explicit integration gates ignored here and passed separately | 2026-09-27T05:46:00Z |
| Admin check, Clippy, scoped formatting | PASS | Publication coordinator and activation adapter; warnings denied | 2026-09-27 |
| M6 Compose configuration | PASS | Apps/all profiles, ports 3000/3011, exclusive writer and private owner services | 2026-09-27T06:11:32Z |
| Fresh isolated M6 runtime | PASS | Current pinned-toolchain debug Rust tools, real pinned compiler/SpiceDB, console on 3011 | 2026-09-27 |
| Connected version builder | PASS | Direct v1 then alternative-paths v2 in same family; both approved; two retained packages, no pending write | 2026-09-27T06:09:38Z |
| Catalogue and binding admin library | PASS | 88 tests, 3 existing opt-in gates skipped; Clippy and client/server builds passed | 2026-09-27T06:34:21Z |
| Gateway suite | PASS | 27 tests, including shared Rust/TS binding record, cold/cache enforcement, exact revision and checksum checks | 2026-09-27T06:29:12Z |
| Console suite | PASS | 388 tests, 18 explicit live/compiler cases skipped | 2026-09-27T06:31:27Z |
| Console and gateway types; console production build | PASS | Includes authenticated approved-version route | 2026-09-27T06:34:22Z |
| Connected catalogue browser | PASS | Real owner RPC returned installed v1/v2 after reload; family selection populated draft; desktop screenshot inspected | 2026-09-27T06:33:50Z |
| M6 Compose bindings | PASS | Required mapping mode, gateway read-only volume, private initializer, no legacy fallback in M6 | 2026-09-27T06:31:27Z |
| Final hygiene and local runtime | PASS | Rust format, scoped Biome and diff checks; updated binding initializer exited successfully and gateway is healthy | 2026-09-27T06:36:47Z |

The first Rust check overlapped unfinished worker wire edits and failed to compile.
The complete library passed after integration. Gateway test typing and import ordering
were corrected. Starting the old runtime initializer lacked its new binding mount.
The current initializer ran successfully with the updated Compose configuration.
| Read-only allocation regression and admin suite | PASS | 72 tests; 3 explicit integration gates skipped in this invocation | 2026-09-27T06:07:32Z |
| Console suite | PASS | 341 tests; 18 opt-in integration cases skipped in this invocation | 2026-09-27T06:09:42Z |
| Console types, admin Clippy, diff check | PASS | Versioned UI, route guards, read-only store; no warnings | 2026-09-27T06:11:35Z |
| Console production build | PASS | Connected versioned builder and guarded routes | 2026-09-27T06:12:00Z |
| Final UI/client/mode checks | PASS | 47 tests after final copy/layout edits; scoped Biome and diff checks | 2026-09-27T06:15:09Z |

## Current Slice File Map

- `services/razield/src/request_preparation.rs`: approved input to pinned signed snapshot, without dispatch.
- `services/razield/src/access_request/preparation.rs`: exact generation and cancellation checks.
- `tools/raziel-admin/src/request_approval/`: private grant validation and materialization.
- `tools/raziel-admin/src/policy_publication/provisioning/`: durable revision/composition handoff.
- `services/razield/src/spicedb/composition.rs`: exact-revision aggregate validation.
- `services/razield/src/service/request_snapshot_receipt.rs`: private snapshot retry and recovery.
- `services/razield/tests/request_preparation.rs`: explicit real-service integration gate.
- `apps/demo-console/components/access-request-approval.tsx`: catalogue selection and private approval form.
- `tools/raziel-admin/src/policy_publication/catalogue.rs`: authenticated installed approvals.
- `tools/raziel-admin/src/policy_activation/`: bounded catalogue RPC and CLI projection.
- `apps/demo-console/app/api/policies/versions/approved/`: operator-only catalogue HTTP route.
- `apps/demo-console/components/policy-catalogue.tsx`: installed approvals and family selection.
- `tools/raziel-admin/src/dataset_binding/`: immutable owner routing record and approved-package constructor.
- `apps/demo-gateway/src/protected-request/dataset-binding.ts`: exact read-only resolution.
- `apps/demo-gateway/src/reconciliation/store.ts`: revision/checksum validation within the data read.
- `tests/vectors/dataset-binding-v1.json`: shared Rust/TypeScript storage-format fixture.
- `docker-compose.m6.yml`: required binding mode and isolated read-only gateway volume.

## Risks And Private-Data Notes

- Shared schema publication and snapshot extraction need one owner-controlled writer.
- A request must not silently move to another policy version or dataset revision.
- Legacy M5 policy publication must not bypass new version ownership.
- The current SDK candidate is vendored locally, not a published npm dependency.
- The SDK's four exact Buf-generated files have generated trailing blank lines.
  Handwritten patch whitespace passed. Do not manually rewrite generated output.
- M5 remains the default. M6 is opt-in and has no legacy schema writer.
- The partial M6 preview uses memory-backed SpiceDB. Restart loses live schema but
  retains owner installation records, so publication fails closed on drift. Durable
  deployment or explicit safe recovery must precede the VM demonstration.
- Installed approvals survive UI reload through the owner catalogue. Recovery of
  unfinished UI drafts remains pending.
- The catalogue is a selection aid, not an authorization reservation. Request approval
  must freshly resolve its exact package and registry approval before persisting.
- Binding enforcement follows approved ADR-0020. No capability/public-value encoding
  changed. The binding store alone does not grant access or publish an approval.
- The local M6 runtime uses prebuilt tools. Its documented release preparation is
  not a claim that a fresh release build passed in this turn. Source binds are
  development convenience, not filesystem isolation from the M5 source tree.
- Snapshot extraction now validates selected-package provenance within the exact
  accepted aggregate. Live preparation and recovery checks establish this boundary.
- The local preview at `http://127.0.0.1:3010/` covers authentication and the inbox.
  Its container is `raziel-access-ui-b0065`, connected to `raziel-access-b0065`.
  It is not the VM or full dataset/proof stack. The older B-0064 preview is stopped.
- The separate M6 preview at `http://127.0.0.1:3011/` runs as project `raziel-m6`.
  It covers dataset CRUD, registration, inbox, request approval, and versioned policy
  publication. Private preparation passed through an explicit integration invocation.
  The preview does not automatically dispatch preparation or proof jobs. Agent
  dashboards are not implemented yet.

## Exact Next Action

Remain paused at the maintainer's request. On explicit resume, claim B-0066 from
the new current baton. Preserve uncommitted B-0064/B-0065 work and seek approval
before push or VM deployment. This archive does not claim full M6 completion.
