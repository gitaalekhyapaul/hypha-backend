# Engineering Baton Archive

- Baton ID: B-0042
- Final state: REVIEW
- Milestone: M5, private candidate revision to snapshot transition
- Owner: Codex
- Started UTC: 2026-08-26T21:09:38Z
- Completed UTC: 2026-08-26T21:22:27Z
- Branch: `main`

## Objective

Resolve one candidate-specific private SpiceDB revision handoff through the existing
`BuildSnapshot` symbolic secret reference, verify its candidate, policy-record, and
policy-hash binding, then preserve the fixed M4 secret mode for compatibility.

## Acceptance Criteria

- [x] `BuildSnapshot` can select fixed legacy secret mode or one configured private
      candidate root. It never accepts a filesystem path.
- [x] Candidate mode derives exactly one handoff path from a validated symbolic
      candidate ID.
- [x] The handoff is read through a checked owner-only file handle and strictly
      decoded without logging or returning its ZedToken.
- [x] Candidate ID, policy record ID, and policy hash must match the validated
      `BuildSnapshot` request before SpiceDB access.
- [x] Missing, altered, malformed, non-private, or mismatched handoffs fail before
      snapshot publication.
- [x] Existing M4 fixed-reference behavior and tests remain valid.
- [x] Focused tests, Rustfmt, Clippy, and `git diff --check` pass.

## Architecture Constraints

- The browser and Next.js receive only a symbolic candidate identifier, never a token
  or local path.
- `verifierd` remains independent and never calls `razield`.
- Snapshot claims, public proof values, PolicyIR semantics, and canonical encodings
  did not change.
- Failed snapshot calls leave the B-0041 handoff intact for exact retry.

## Files Changed

- `tools/raziel-admin/src/spicedb_candidate/`
- `services/razield/src/service/snapshot_config.rs`
- `services/razield/src/service/snapshot_runtime.rs`
- `services/razield/src/service/tests.rs`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0042-private-candidate-revision-snapshot.md`

## Decisions

- Preserved the fixed M4 revision secret as one explicit configuration mode.
- Added one mutually exclusive candidate-root mode. The request supplies only a
  validated candidate identifier.
- Bound the private handoff to the candidate ID, policy record ID, and policy hash
  before opening a SpiceDB connection.
- Reused the owner-only checked-file reader and returned only the revision string to
  the internal snapshot runtime.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `cargo test -p raziel-admin --no-fail-fast` | PASS, 23 tests | Local Rust | 2026-08-26T21:22:27Z |
| `cargo test -p razield --no-fail-fast` | PASS, 107 run and 13 live tests ignored | Local Rust | 2026-08-26T21:22:27Z |
| `cargo clippy -p raziel-admin -p razield --all-targets -- -D warnings` | PASS | Local Rust | 2026-08-26T21:22:27Z |
| `./scripts/dev cargo test -p raziel-admin -p razield --lib --no-fail-fast` | PASS, 121 run and 4 ignored | Pinned Docker | 2026-08-26T21:22:27Z |
| `./scripts/dev cargo clippy -p raziel-admin -p razield --all-targets -- -D warnings` | PASS | Pinned Docker | 2026-08-26T21:22:27Z |
| `./scripts/dev cargo fmt --all -- --check` | PASS | Pinned Docker | 2026-08-26T21:22:27Z |
| Private-output scan | PASS, no revision logging | Candidate and snapshot runtime | 2026-08-26T21:22:27Z |
| `git diff --check` | PASS | Worktree | 2026-08-26T21:22:27Z |

## Risks And Private-Data Notes

- The loaded ZedToken exists only inside the snapshot service process long enough to
  perform the exact SpiceDB read and create the signed snapshot descriptor.
- Candidate cleanup after a successful snapshot remains part of the owner lifecycle
  workflow.

## Exact Next Action

Connect the typed M5 policy draft to a server-side candidate preparation boundary
that creates the private compiler inputs and candidate manifest.
