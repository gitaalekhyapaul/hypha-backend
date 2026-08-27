# Engineering Baton Archive

- Baton ID: B-0049
- State: COMPLETE
- Milestone: M5, access graph and relationship snapshot
- Owner: Codex
- Started UTC: 2026-08-26T23:24:13Z
- Completed UTC: 2026-08-27T00:03:12Z
- Branch: `main`
- Worktree: Approved M5 docs and B-0034 through B-0049 features are uncommitted.

## Objective

Add a bounded desktop Access Graph workspace that publishes the selected policy's
scenario relationships to SpiceDB and creates the signed private relationship
snapshot needed for capability issuance.

## Acceptance Criteria

- [x] The Access Graph screen starts from one active policy and exposes only the
      relationship choices supported by the selected visual rule.
- [x] The operator can review actor, agent, team, task, dataset, and caveat values
      before one explicit publish action.
- [x] The mutation authenticates Origin, session, and CSRF before reading input.
- [x] The owner adapter writes the exact candidate schema and bounded relationship
      batch through the existing write-only SpiceDB boundary.
- [x] The snapshot-only `razield` process builds and registers one signed snapshot
      from the private candidate revision and active policy record.
- [x] Retry returns the same snapshot for the same graph request.
- [x] The browser receives only the safe public snapshot projection.
- [x] Relationships, caveat values, ZedToken, salts, Merkle openings, PolicyIR, local
      paths, and authority signatures stay out of browser responses and logs.
- [x] Focused frontend and Rust checks plus one live desktop publication pass.

## Architecture Constraints

- The UI cannot read the SpiceDB graph or ZedToken after publication.
- The agent never receives relationships, stored caveat values, or Merkle openings.
- Every snapshot binds the exact approved policy record and hash.
- The snapshot authority key stays outside the console and capability API process.
- Scenario mapping cannot change PolicyIR or guest semantics.

## Decisions

- The Access Graph screen is a bounded scenario form, not a free-form tuple editor.
- The console sends graph data through an owner-only Unix socket. It does not mount
  SpiceDB credentials, registries, relationship storage, or snapshot keys.
- The snapshot service builds without the proof and queue features. This keeps the
  owner snapshot process small without changing the normal `razield` build.
- Active policy selection remains browser-session state for this PoC slice. A refresh
  requires activation again, and the activation service returns the same record.
- The stale M5 plan reference to browser-visible ZedToken was removed. The public
  screen uses the signed snapshot ID and root.

## Work Completed

- Added the Access Graph form, visual relationship preview, authenticated route,
  publication states, and accepted public snapshot panel.
- Added a reusable SpiceDB candidate publication API with deterministic relationship
  batches, exact retry, and conflict detection.
- Added an owner-only Access Graph service and client over a private Unix socket.
- Added the snapshot-only `razield` service and isolated Compose mounts.
- Added the focused frontend relationship mapping check.
- Updated the local portal runbook and M5 public-output contract.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/compose --profile apps exec -T demo-console pnpm --filter @raziel/demo-console test` | PASS, 47 tests | Console behavior and Access Graph mapping | 2026-08-27T00:01Z |
| `./scripts/compose --profile apps exec -T demo-console pnpm --filter @raziel/demo-console lint` | PASS, 71 files | Console source | 2026-08-27T00:01Z |
| `./scripts/compose --profile apps exec -T demo-console pnpm --filter @raziel/demo-console typecheck` | PASS | Console types | 2026-08-27T00:01Z |
| `cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Owner publication service | 2026-08-27T00:02Z |
| `cargo clippy -p razield --all-targets -- -D warnings` | PASS | Normal service build | 2026-08-27T00:02Z |
| `cargo clippy -p razield --no-default-features --bin razield-snapshot -- -D warnings` | PASS | Snapshot-only service build | 2026-08-27T00:02Z |
| Live desktop policy activation and Access Graph publication | PASS, one relationship and registry generation 5 | Connected frontend and owner services | 2026-08-26T23:57Z |
| Exact publication retry through `raziel-access-graph-publish` | PASS, same snapshot ID, root, epoch, and generation | Idempotency | 2026-08-27T00:02Z |

## Live Public Result

Post-archive correction, 2026-08-27: this result used the display agent ID as the
SpiceDB subject and a 3,600-second snapshot lifetime. It remains valid evidence for
graph publication and exact retry. It is not capability-admission evidence. B-0050
replaced the subject with the lowercase Ed25519 key thumbprint and raised the default
snapshot lifetime to 7,200 seconds.

- Policy record: `policy-6c22c7d88149e685ac7e453a010a4ab2`
- Policy hash: `0bb81b18a3de63db816659b3b444fb4b5ce4e4afb684cc040d759990b8f71411`
- Snapshot: `snapshot-v1:e60eff91314f265c260213be060271876dbd95c0e4b0fa60d98efd5e0a3b355f`
- Relationship root: `e60eff91314f265c260213be060271876dbd95c0e4b0fa60d98efd5e0a3b355f`
- Snapshot epoch: `1`
- Accepted relationship count: `1`
- Registry generation: `5`
- Status: `ACCEPTED`

## Risks And Private-Data Notes

- The form contains relationship and caveat values that become private witness data.
  They enter the authenticated owner request but do not return in public output.
- Active policy selection does not survive a browser refresh in this slice.
- This synthetic workflow does not satisfy the M6 real-data acceptance gate.

## Exact Next Action

Open B-0050 for the Agent and Capability workspace. Connect the existing TypeScript
agent signing SDK to `razield` through a server-side session, expose queued and proving
status in the desktop portal, and keep local recorded mode unable to unlock data.
