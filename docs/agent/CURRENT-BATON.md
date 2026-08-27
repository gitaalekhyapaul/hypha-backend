# Current Engineering Baton

- Baton ID: B-0051
- State: REVIEW
- Milestone: M5, first protected reconciliation query
- Owner: unassigned
- Started UTC: 2026-08-27T00:50:21Z
- Last updated UTC: 2026-08-27T03:21:10Z
- Branch: `main`
- Worktree: M5 through UF-08 and the focused VM policy-tools build fix are committed;
  the VM rerun is pending.

## Objective

Add the first protected reconciliation query. The console signs one exact request,
the gateway checks the capability through the independent verifier, rejects stale or
replayed envelopes, applies the approved dataset scope, returns bounded synthetic
rows, and records a public receipt.

## Acceptance Criteria

- [x] The Reconciliation screen starts only from a live succeeded capability and its
      finalized synthetic dataset.
- [x] The server-owned agent signs the exact method, target, body hash, fresh nonce,
      timestamp, and capability ID through the existing TypeScript SDK.
- [x] The gateway validates request shape, timestamp, agent key binding, signature,
      capability scope, and nonce before reading protected rows.
- [x] `verifierd` verifies the capability from its own M5 registry with no `razield`
      connection.
- [x] A valid first request returns only rows inside the capability scope.
- [x] Recorded mode and an unverifiable capability cannot unlock transaction data.
- [x] The gateway writes one bounded public receipt without proof bytes, private
      policy data, relationships, caveat values, or transaction rows.
- [x] Focused gateway, SDK, and console checks pass.

## Architecture Constraints

- `verifierd` never calls `razield`.
- The gateway never accepts a recorded or unproved success state.
- Every protected request checks exact scope, agent signature, freshness, and replay.
- The browser receives no raw capability proof, PolicyIR, relationships, witness, or
  detailed policy denial path.
- TypeScript uses the existing canonical SDK. It cannot duplicate signature rules.
- This slice uses labeled synthetic rows and does not satisfy M6.

## File Scope

- `apps/demo-console/app/api/reconciliation/`
- `apps/demo-console/app/globals.css`
- `apps/demo-console/components/agent-capability-workspace.tsx`
- `apps/demo-console/components/reconciliation-workspace.tsx`
- `apps/demo-console/components/operator-portal.tsx`
- `apps/demo-console/lib/agent-capability/`
- `apps/demo-console/lib/portal-client/`
- `apps/demo-console/test/`
- `apps/demo-gateway/app/api/datasets/`
- `apps/demo-gateway/src/protected-request/`
- `apps/demo-gateway/src/reconciliation/`
- `apps/demo-gateway/test/`
- `packages/sdk-ts/`
- `tools/raziel-admin/`
- `scripts/prepare-m5-policy-tools`
- `docker-compose.yml`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0051-first-protected-query.md`

## Out Of Scope

- Verification cache reuse
- User-controlled denial and replay lab controls
- Receipt filtering and export screen
- Real transaction data
- Mobile layout polish
- New proof or capability semantics

## Decisions

- The console keeps the agent key and live capability package in process memory. It
  sends the protected envelope to the gateway over the private Compose network.
- Recorded mode can render public history but cannot call protected success routes.
- The gateway returns generic authorization failures to the browser.
- The M5 verifier uses a dedicated root-owned IPC volume and networkless two-GiB
  worker. `demo-verifierd` reads the approved M5 registry and remains private to the
  Compose network.
- The approved official Node Redis client stores one atomic nonce claim until the
  capability expires. ADR-0002 requires the gateway to claim the nonce before it
  calls `verifierd` and never release it after a service failure.

## Open Questions And Blockers

- The first VM startup failed in `demo-console-policy-tools`. Go attempted optional
  VCS stamping inside the bind-mounted checkout and returned `error obtaining VCS
  status: exit status 128`. The focused build fix disables unused VCS stamping.
- Review gate: push the focused build fix, pull it onto the NUS VM, then run one live
  UF-01 through UF-08 walkthrough. Local checks do not replace that live proof and
  verifier acceptance run.

## Work Completed

- B-0050 archived after the desktop capability workspace, SDK signing path, stable
  live API job ID, recorded playback, and VM profile passed their local gates.
- Added the approved `redis` 6.2.1 workspace dependency.
- Added a strict Rust verifier-client bridge. It inspects canonical capability IDs and
  forwards signed protected requests to `verifierd` without exposing proof or private
  authorization inputs to TypeScript.
- Added Redis nonce claims, independent verification, exact scope checks, bounded
  synthetic queries, and Protobuf-shaped public JSONL receipts to the gateway.
- Added the authenticated console signing route and desktop Reconciliation workspace.
  The view unlocks only for a live succeeded capability bound to the selected dataset.
- Added the private M5 verifier service shape, two-GiB networkless verifier sandbox,
  and verifier client to the pinned M5 policy-tool build.
- Updated the VM runbook with deployment checks, one live proof walkthrough, the
  protected query, receipt evidence, and the current demo boundary.
- Committed and pushed M5 through UF-08 as `8cb36c6`.
- Matched the existing workspace build policy by disabling unused Go VCS stamping in
  the M5 policy-tools builder. This avoids host/container Git ownership checks and
  does not change the compiler output or runtime policy semantics.

## Remaining M5 Work After B-0051

- VM review: commit and push this worktree, pull it onto the NUS VM, then run the first
  live UF-08 query through a fresh Plonk capability.
- UF-09: reuse one unexpired capability with a fresh nonce, add a bounded verification
  cache, and record a distinct cache-hit receipt.
- UF-10: add a controlled out-of-scope request and generic denial presentation.
- UF-11: add replay, expired, altered, and unsigned request demonstrations that return
  no protected rows.
- UF-12: add receipt history, bounded filters, public JSONL export, and benchmark
  summary export.
- UF-14: run and record the final desktop walkthrough through the SSH tunnel.

## Validation Plan

- Read the gateway store, SDK protected-request surface, verifier contract, and
  retained proof acceptance path.
- Add the smallest typed nonce, verifier, scope, query, and receipt boundaries.
- Connect one desktop first-query screen without adding denial-lab controls yet.
- Run focused checks and a retained-proof gateway check. Defer fresh Plonk to the
  final VM run.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `cargo test -p raziel-admin verifier_client` | PASS, 10 tests | Strict verifier bridge | 2026-08-27T01:20:00Z |
| `cargo fmt --all -- --check` | PASS | Rust workspace format | 2026-08-27T01:22:00Z |
| `cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Verifier bridge static checks | 2026-08-27T01:23:00Z |
| Console and gateway `format:check`, `lint`, and `typecheck` | PASS | 93 console and 29 gateway files | 2026-08-27T01:27:00Z |
| Console and gateway tests | PASS, 67 tests | Signed query, gateway, replay, scope, receipt, portal | 2026-08-27T01:25:00Z |
| Console and gateway production builds | PASS | Next.js deployment build | 2026-08-27T01:27:00Z |
| `./scripts/compose --profile apps --profile m5-live config --quiet` | PASS | Full live VM service graph | 2026-08-27T01:27:00Z |
| `sh -n scripts/prepare-m5-policy-tools` | PASS | VM tool builder | 2026-08-27T01:28:04Z |
| `git diff --check` | PASS | Uncommitted M5 worktree | 2026-08-27T01:28:04Z |
| `pnpm format:check` in the pinned toolchain | PASS, 165 files plus Rust and Go | Full workspace | 2026-08-27T01:51:26Z |
| `pnpm lint` in the pinned toolchain | PASS, Biome, Clippy, Go vet, and Buf | Full workspace | 2026-08-27T01:51:26Z |
| `pnpm typecheck` in the pinned toolchain | PASS, TypeScript, Cargo check, and Go compile | Full workspace | 2026-08-27T01:51:26Z |
| `pnpm test` in the pinned toolchain | PASS, TypeScript, Rust, Go, and doc tests | Full non-guest workspace | 2026-08-27T01:51:26Z |
| First live VM Compose startup | FAIL, Go policy-tools build could not obtain VCS status | M5 policy-tools builder | 2026-08-27T03:18:50Z |
| `sh -n scripts/prepare-m5-policy-tools` | PASS | Focused policy-tools shell syntax | 2026-08-27T03:20:01Z |
| `./scripts/compose --profile apps --profile m5-live config --quiet` | PASS | Full M5 Compose graph | 2026-08-27T03:20:01Z |
| `git diff --check` | PASS | Focused VM build fix | 2026-08-27T03:20:01Z |
| Local `demo-console-policy-tools` container build | PASS, `m5_policy_tools=ready` | Exact one-shot policy-tools build | 2026-08-27T03:21:10Z |
| Live VM UF-08 walkthrough | NOT AVAILABLE, requires commit and VM pull | Fresh proof and connected services | 2026-08-27T01:28:04Z |

## Risks And Private-Data Notes

- The gateway sees public capability claims and selected transaction rows. Receipts
  must not retain rows, request bodies, raw proof bytes, or authorization context.
- A retained proof fixture can validate verifier transport but cannot authorize the
  browser's recorded M5 state.
- Privacy review: agent responses contain no PolicyIR, relationships, Merkle
  openings, caveat secrets, or witness material. The verifier reads its own registry
  and has no prover or `razield` connection.

## Exact Next Action

Pull the focused policy-tools build fix onto the NUS VM, then rerun the live UF-08
procedure in `docs/runbooks/M5-LOCAL-PORTAL.md`.
