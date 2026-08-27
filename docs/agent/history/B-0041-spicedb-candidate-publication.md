# Engineering Baton Archive

- Baton ID: B-0041
- Final state: REVIEW
- Milestone: M5, SpiceDB candidate publication and revision handoff
- Owner: Codex
- Started UTC: 2026-08-26T20:51:38Z
- Completed UTC: 2026-08-26T21:09:38Z
- Branch: `main`

## Objective

Implement ADR-0018 `write-spicedb-candidate` so one registered candidate writes its
exact compiled schema and bounded relationship batch to SpiceDB, then stores the
returned revision in a fixed owner-only handoff file for snapshot publication.

## Acceptance Criteria

- [x] The command derives all private paths from fixed owner configuration and one
      validated candidate ID.
- [x] The exact schema digest matches both the candidate manifest and registered
      PolicyIR before either SpiceDB mutation.
- [x] A strict typed JSON relationship batch rejects unknown fields, unsupported
      policy shapes, duplicate identities, lossy integers, and oversized batches.
- [x] The command writes schema before one transactional relationship batch and
      requires a non-empty ZedToken from both mutations.
- [x] The final revision handoff uses mode `0600` and is absent from public results,
      errors, command arguments, and debug output.
- [x] Retry reuses an exact matching handoff without mutating SpiceDB again.
- [x] Schema or relationship failure records no false handoff success.
- [x] Focused tests, Rustfmt, Clippy, and `git diff --check` pass.

## Architecture Constraints

- Relationship administration remains owner-only and write-only.
- The command cannot approve policies, mutate the registry, publish snapshots, or
  issue capabilities.
- Public proof values, PolicyIR semantics, canonical commitments, and guest behavior
  did not change.

## Files Changed

- `Cargo.lock`
- `tools/raziel-admin/Cargo.toml`
- `tools/raziel-admin/src/lib.rs`
- `tools/raziel-admin/src/bin/raziel-spicedb-candidate.rs`
- `tools/raziel-admin/src/spicedb_candidate/`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0041-spicedb-candidate-publication.md`

## Decisions

- Reused the approved pinned `spicedb-rs-proto` client directly. Importing
  `services/razield` would create a Cargo dependency cycle.
- Used `TOUCH` mutations so an exact retry can recover after a write succeeded but
  local handoff persistence failed.
- Validated the complete private batch through the existing `SnapshotTree` policy
  boundary. This supports direct, team, delegated, and caveated records.
- Kept only candidate ID as caller input. Paths, endpoint, trust domain, registry,
  and credential location come from fixed owner configuration.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `cargo test -p raziel-admin --no-fail-fast` | PASS, 23 tests | Local Rust | 2026-08-26T21:09:38Z |
| `cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Local Rust | 2026-08-26T21:09:38Z |
| `./scripts/dev cargo test -p raziel-admin --no-fail-fast` | PASS, 23 tests | Pinned Docker | 2026-08-26T21:09:38Z |
| `./scripts/dev cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Pinned Docker | 2026-08-26T21:09:38Z |
| `./scripts/dev cargo fmt --all -- --check` | PASS | Pinned Docker | 2026-08-26T21:09:38Z |
| `git diff --check` | PASS | Worktree | 2026-08-26T21:09:38Z |

## Risks And Private-Data Notes

- B-0041 does not expose a graph read API.
- The returned ZedToken remains inside the private handoff message.
- The next service slice must resolve this candidate-specific handoff without
  accepting a caller-selected path or exposing the token to Next.js.

## Exact Next Action

Add a candidate-specific symbolic revision resolver to `razield` and bind the loaded
handoff to the requested policy record and policy hash before `BuildSnapshot` reads
SpiceDB.
